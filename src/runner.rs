use crate::{
    codex_exec::run_codex_json,
    config::{LoopConfig, ReviewAgentConfig},
    hooks::{run_hook, run_required_hook},
    record::{IterationRecord, write_record},
    run_state::{
        RunManifest, RunStatus, artifact_hash, write_manifest_and_index, write_run_summary,
    },
    schema::{repair_schema, review_schema},
    verify::run_verify,
    workspace::prepare_iteration_workspace,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopSummary {
    pub run_id: String,
    pub run_dir: PathBuf,
    pub passed: bool,
    pub iterations: usize,
    pub final_record_path: Option<PathBuf>,
    pub final_artifact_path: Option<PathBuf>,
    pub summary_path: Option<PathBuf>,
}

pub fn should_stop(
    iteration: usize,
    max_iterations: usize,
    validation_passed: bool,
    remaining_delta: &[String],
) -> bool {
    validation_passed || iteration >= max_iterations || remaining_delta.is_empty()
}

fn run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("run-{millis}")
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn run_loop(config: &LoopConfig, source_root: &Path, runs_dir: &Path) -> Result<LoopSummary> {
    run_loop_with_executor(config, source_root, runs_dir, &CodexPhaseExecutor)
}

pub trait PhaseExecutor {
    fn review(
        &self,
        config: &LoopConfig,
        agent: &ReviewAgentConfig,
        artifact_text: &str,
        run_dir: &Path,
    ) -> Result<Value>;

    fn repair(
        &self,
        config: &LoopConfig,
        editable_path: &Path,
        findings: &Value,
        remaining_delta: &[String],
        iteration: usize,
        run_dir: &Path,
    ) -> Result<Value>;
}

pub struct CodexPhaseExecutor;

impl PhaseExecutor for CodexPhaseExecutor {
    fn review(
        &self,
        config: &LoopConfig,
        agent: &ReviewAgentConfig,
        artifact_text: &str,
        run_dir: &Path,
    ) -> Result<Value> {
        let phase_config = config.review_phase_config_for(agent);
        run_codex_json(
            &review_prompt(config, agent, artifact_text),
            &review_schema(),
            run_dir,
            &phase_config,
        )
    }

    fn repair(
        &self,
        config: &LoopConfig,
        editable_path: &Path,
        findings: &Value,
        remaining_delta: &[String],
        iteration: usize,
        run_dir: &Path,
    ) -> Result<Value> {
        let phase_config = config.repair_phase_config();
        run_codex_json(
            &repair_prompt(config, editable_path, findings, remaining_delta, iteration),
            &repair_schema(),
            run_dir,
            &phase_config,
        )
    }
}

fn review_prompt(config: &LoopConfig, agent: &ReviewAgentConfig, artifact_text: &str) -> String {
    let focus = agent
        .focus
        .as_deref()
        .unwrap_or("review correctness, tests, docs, and safety");
    format!(
        "You are a read-only review agent.\nAgent: {}\nFocus: {}\nArtifact: {}\nGoal: {}\nDo not edit files. Return concise structured findings.\n\n{}",
        agent.id, focus, config.artifact, config.goal, artifact_text
    )
}

fn repair_prompt(
    config: &LoopConfig,
    editable_path: &Path,
    findings: &Value,
    remaining_delta: &[String],
    iteration: usize,
) -> String {
    format!(
        "You are repairing a copied artifact.\nEditable copy: {}\nIteration: {}\nGoal: {}\nMake the smallest useful edits. Edit only the editable copy. Do not claim validation passed.\nReview findings: {}\nRemaining validation delta: {}",
        editable_path.display(),
        iteration,
        config.goal,
        findings,
        serde_json::to_string_pretty(remaining_delta).unwrap()
    )
}

pub fn run_loop_with_executor(
    config: &LoopConfig,
    source_root: &Path,
    runs_dir: &Path,
    executor: &dyn PhaseExecutor,
) -> Result<LoopSummary> {
    let run_id = run_id();
    let root = runs_dir.join(&run_id);
    let source_artifact = source_root.join(&config.artifact);
    let source_artifact_hash = artifact_hash(&source_artifact)?;
    let mut manifest = RunManifest {
        run_id: run_id.clone(),
        artifact: config.artifact.clone(),
        goal: config.goal.clone(),
        verify: config.verify.clone(),
        max_iterations: config.max_iterations,
        source_artifact_hash,
        started_at_unix: unix_seconds(),
        finished_at_unix: None,
        status: RunStatus::Running,
        iterations: 0,
        final_record_path: None,
        final_artifact_path: None,
        summary_path: Some(format!("{run_id}/summary.md")),
        hooks: config.hooks.clone(),
    };
    write_manifest_and_index(&manifest, runs_dir)?;

    if let Some(command) = &config.hooks.pre_run {
        run_required_hook("pre_run", command, source_root, &root.join("hooks/pre_run"))?;
    }

    let mut workspace_source = source_root.to_path_buf();
    let mut remaining_delta = Vec::new();
    let mut final_record_path = None;
    let mut final_artifact_path = None;
    let mut summary_path = None;

    for iteration in 1..=config.max_iterations {
        let iteration_dir = root.join(format!("iteration_{iteration}"));
        let copied =
            prepare_iteration_workspace(&workspace_source, &config.artifact, &iteration_dir)?;
        let workspace_root = iteration_dir.join("workspace");
        let artifact_text = fs::read_to_string(&copied)?;

        let review = run_review_agents(config, &artifact_text, &iteration_dir, executor)?;
        let repair = executor.repair(
            config,
            &copied,
            &review,
            &remaining_delta,
            iteration,
            &iteration_dir.join("repair"),
        )?;
        let mut validation = run_verify(&config.verify, &workspace_root)?;
        remaining_delta = if validation.passed {
            vec![]
        } else {
            vec![if validation.stderr.trim().is_empty() {
                validation.stdout.clone()
            } else {
                validation.stderr.clone()
            }]
        };
        if let Some(command) = &config.hooks.post_iteration {
            let hook_result = run_hook(
                "post_iteration",
                command,
                &workspace_root,
                &iteration_dir.join("hooks/post_iteration"),
            )?;
            if !hook_result.passed {
                let delta = hook_delta("post_iteration", &hook_result);
                validation.passed = false;
                validation.returncode = hook_result.returncode;
                if validation.stderr.trim().is_empty() {
                    validation.stderr = delta.clone();
                } else {
                    validation.stderr.push('\n');
                    validation.stderr.push_str(&delta);
                }
                remaining_delta.push(delta);
            }
        }

        let record = IterationRecord {
            iteration,
            review,
            repair,
            validation,
            remaining_delta: remaining_delta.clone(),
        };

        let record_path = iteration_dir.join("record.json");
        write_record(&record, &record_path)?;
        final_record_path = Some(record_path);
        final_artifact_path = Some(workspace_root.join(&config.artifact));
        workspace_source = workspace_root;

        let should_stop = should_stop(
            iteration,
            config.max_iterations,
            record.validation.passed,
            &remaining_delta,
        );
        manifest.iterations = iteration;
        manifest.final_record_path = Some(format!("{run_id}/iteration_{iteration}/record.json"));
        manifest.final_artifact_path = Some(format!(
            "{run_id}/iteration_{iteration}/workspace/{}",
            config.artifact
        ));
        manifest.status = if record.validation.passed {
            RunStatus::Passed
        } else if should_stop {
            RunStatus::Failed
        } else {
            RunStatus::Running
        };
        if manifest.status != RunStatus::Running {
            manifest.finished_at_unix = Some(unix_seconds());
        }
        if manifest.status == RunStatus::Failed {
            if let Some(command) = &config.hooks.on_failure {
                let _ = run_hook(
                    "on_failure",
                    command,
                    source_root,
                    &root.join("hooks/on_failure"),
                );
            }
        }
        write_manifest_and_index(&manifest, runs_dir)?;
        summary_path = Some(write_run_summary(runs_dir, &manifest)?);

        if should_stop {
            return Ok(LoopSummary {
                run_id,
                run_dir: root,
                passed: record.validation.passed,
                iterations: iteration,
                final_record_path,
                final_artifact_path,
                summary_path,
            });
        }
    }

    Ok(LoopSummary {
        run_id,
        run_dir: root,
        passed: false,
        iterations: config.max_iterations,
        final_record_path,
        final_artifact_path,
        summary_path,
    })
}

fn run_review_agents(
    config: &LoopConfig,
    artifact_text: &str,
    iteration_dir: &Path,
    executor: &dyn PhaseExecutor,
) -> Result<Value> {
    let mut agents = Vec::new();
    let mut findings = Vec::new();
    for agent in config.effective_review_agents() {
        let run_dir = iteration_dir.join("review").join(&agent.id);
        let answer = executor.review(config, &agent, artifact_text, &run_dir)?;
        fs::create_dir_all(&run_dir)?;
        fs::write(
            run_dir.join("answer.json"),
            serde_json::to_string_pretty(&answer)?,
        )?;

        if let Some(items) = answer.get("findings").and_then(Value::as_array) {
            for item in items {
                findings.push(json!({
                    "agent": agent.id,
                    "finding": item
                }));
            }
        }
        agents.push(json!({
            "id": agent.id,
            "model": agent.model,
            "focus": agent.focus,
            "answer": answer
        }));
    }

    Ok(json!({
        "agents": agents,
        "findings": findings
    }))
}

fn hook_delta(event: &str, result: &crate::verify::VerifyResult) -> String {
    let output = if result.stderr.trim().is_empty() {
        result.stdout.trim()
    } else {
        result.stderr.trim()
    };
    if output.is_empty() {
        format!("{event} hook failed with status {}", result.returncode)
    } else {
        format!(
            "{event} hook failed with status {}: {output}",
            result.returncode
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    struct FakePhaseExecutor;

    impl PhaseExecutor for FakePhaseExecutor {
        fn review(
            &self,
            _config: &LoopConfig,
            _agent: &ReviewAgentConfig,
            _artifact_text: &str,
            _run_dir: &Path,
        ) -> Result<Value> {
            Ok(json!({"findings": []}))
        }

        fn repair(
            &self,
            config: &LoopConfig,
            editable_path: &Path,
            _findings: &Value,
            _remaining_delta: &[String],
            iteration: usize,
            _run_dir: &Path,
        ) -> Result<Value> {
            Ok(json!({
                "artifact": config.artifact,
                "iteration": iteration,
                "changes_made": [],
                "unresolved_items": [],
                "updated_artifact_path": editable_path
            }))
        }
    }

    struct RecordingPhaseExecutor {
        seen_deltas: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl PhaseExecutor for RecordingPhaseExecutor {
        fn review(
            &self,
            _config: &LoopConfig,
            _agent: &ReviewAgentConfig,
            _artifact_text: &str,
            _run_dir: &Path,
        ) -> Result<Value> {
            Ok(json!({"findings": []}))
        }

        fn repair(
            &self,
            config: &LoopConfig,
            editable_path: &Path,
            _findings: &Value,
            remaining_delta: &[String],
            iteration: usize,
            _run_dir: &Path,
        ) -> Result<Value> {
            self.seen_deltas
                .lock()
                .unwrap()
                .push(remaining_delta.to_vec());
            Ok(json!({
                "artifact": config.artifact,
                "iteration": iteration,
                "changes_made": [],
                "unresolved_items": [],
                "updated_artifact_path": editable_path
            }))
        }
    }

    fn test_config(verify: &str, max_iterations: usize) -> LoopConfig {
        LoopConfig {
            artifact: "README.md".to_string(),
            goal: "repair docs".to_string(),
            verify: verify.to_string(),
            max_iterations,
            model: "gpt-5.5".to_string(),
            review_model: None,
            repair_model: None,
            model_reasoning_effort: "low".to_string(),
            sandbox: "workspace-write".to_string(),
            approval_policy: "never".to_string(),
            profile: None,
            review_agents: Vec::new(),
            hooks: crate::hooks::HooksConfig::default(),
        }
    }

    fn passing_verify(message: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("echo {message}")
        } else {
            format!("printf {message}")
        }
    }

    fn failing_verify(message: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("echo {message} && exit /B 7")
        } else {
            format!("printf {message}; exit 7")
        }
    }

    #[test]
    fn stops_when_validation_passes() {
        assert!(should_stop(1, 3, true, &["x".to_string()]));
    }

    #[test]
    fn stops_at_max_iterations() {
        assert!(should_stop(3, 3, false, &["x".to_string()]));
    }

    #[test]
    fn continues_when_delta_remains_before_limit() {
        assert!(!should_stop(1, 3, false, &["x".to_string()]));
    }

    #[test]
    fn dry_run_writes_record() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let config = test_config(&passing_verify("ok"), 3);

        let summary = run_loop_with_executor(
            &config,
            dir.path(),
            &dir.path().join("runs"),
            &FakePhaseExecutor,
        )
        .unwrap();

        assert!(summary.passed);
        assert!(summary.final_record_path.unwrap().exists());
    }

    #[test]
    fn run_loop_writes_manifest_index_and_summary() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let runs_dir = dir.path().join("runs");
        let config = test_config(&passing_verify("ok"), 3);

        let summary =
            run_loop_with_executor(&config, dir.path(), &runs_dir, &FakePhaseExecutor).unwrap();

        assert!(summary.run_dir.join("manifest.json").exists());
        assert!(summary.summary_path.unwrap().exists());
        assert!(runs_dir.join("index.json").exists());

        let manifest_text = fs::read_to_string(summary.run_dir.join("manifest.json")).unwrap();
        let manifest: crate::run_state::RunManifest = serde_json::from_str(&manifest_text).unwrap();

        assert_eq!(manifest.run_id, summary.run_id);
        assert_eq!(manifest.artifact, "README.md");
        assert_eq!(manifest.iterations, 1);
        assert_eq!(manifest.status, crate::run_state::RunStatus::Passed);
        assert!(manifest.final_artifact_path.unwrap().contains("README.md"));
    }

    #[test]
    fn failed_verification_writes_remaining_delta() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let config = test_config(&failing_verify("failure-details"), 1);

        let summary = run_loop_with_executor(
            &config,
            dir.path(),
            &dir.path().join("runs"),
            &FakePhaseExecutor,
        )
        .unwrap();
        let record_path = summary.final_record_path.unwrap();
        let record: IterationRecord =
            serde_json::from_str(&fs::read_to_string(record_path).unwrap()).unwrap();

        assert!(!summary.passed);
        assert_eq!(summary.iterations, 1);
        assert!(!record.validation.passed);
        assert_eq!(record.validation.returncode, 7);
        assert!(record.remaining_delta[0].contains("failure-details"));
    }

    #[test]
    fn failed_verification_runs_until_max_iterations() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let config = test_config(&failing_verify("still-bad"), 2);

        let summary = run_loop_with_executor(
            &config,
            dir.path(),
            &dir.path().join("runs"),
            &FakePhaseExecutor,
        )
        .unwrap();
        let final_record = summary.final_record_path.unwrap();
        let run_root = final_record
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        assert!(!summary.passed);
        assert_eq!(summary.iterations, 2);
        assert!(run_root.join("iteration_1/record.json").exists());
        assert!(run_root.join("iteration_2/record.json").exists());
    }

    #[test]
    fn second_iteration_receives_previous_delta() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let config = test_config(&failing_verify("first-failure"), 2);
        let seen_deltas = Arc::new(Mutex::new(Vec::new()));
        let executor = RecordingPhaseExecutor {
            seen_deltas: Arc::clone(&seen_deltas),
        };

        let summary =
            run_loop_with_executor(&config, dir.path(), &dir.path().join("runs"), &executor)
                .unwrap();
        let seen_deltas = seen_deltas.lock().unwrap();

        assert!(!summary.passed);
        assert_eq!(summary.iterations, 2);
        assert_eq!(seen_deltas.len(), 2);
        assert!(seen_deltas[0].is_empty());
        assert!(seen_deltas[1][0].contains("first-failure"));
    }

    struct RecordingReviewExecutor {
        seen_agents: Arc<Mutex<Vec<String>>>,
        repair_reviews: Arc<Mutex<Vec<Value>>>,
    }

    impl PhaseExecutor for RecordingReviewExecutor {
        fn review(
            &self,
            _config: &LoopConfig,
            agent: &ReviewAgentConfig,
            _artifact_text: &str,
            _run_dir: &Path,
        ) -> Result<Value> {
            self.seen_agents.lock().unwrap().push(agent.id.clone());
            Ok(json!({
                "findings": [
                    {
                        "severity": "medium",
                        "message": format!("{} finding", agent.id)
                    }
                ]
            }))
        }

        fn repair(
            &self,
            config: &LoopConfig,
            editable_path: &Path,
            findings: &Value,
            _remaining_delta: &[String],
            iteration: usize,
            _run_dir: &Path,
        ) -> Result<Value> {
            self.repair_reviews.lock().unwrap().push(findings.clone());
            Ok(json!({
                "artifact": config.artifact,
                "iteration": iteration,
                "changes_made": [],
                "unresolved_items": [],
                "updated_artifact_path": editable_path
            }))
        }
    }

    #[test]
    fn multi_review_profile_runs_default_agents_and_merges_findings() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let mut config = test_config(&passing_verify("ok"), 1);
        config.profile = Some("multi-review".to_string());
        let seen_agents = Arc::new(Mutex::new(Vec::new()));
        let repair_reviews = Arc::new(Mutex::new(Vec::new()));
        let executor = RecordingReviewExecutor {
            seen_agents: Arc::clone(&seen_agents),
            repair_reviews: Arc::clone(&repair_reviews),
        };

        let summary =
            run_loop_with_executor(&config, dir.path(), &dir.path().join("runs"), &executor)
                .unwrap();

        assert!(summary.passed);
        assert_eq!(
            *seen_agents.lock().unwrap(),
            vec![
                "correctness".to_string(),
                "tests".to_string(),
                "docs".to_string()
            ]
        );
        let merged = repair_reviews.lock().unwrap()[0].clone();
        assert_eq!(merged["agents"].as_array().unwrap().len(), 3);
        assert_eq!(merged["findings"].as_array().unwrap().len(), 3);
        assert!(
            summary
                .run_dir
                .join("iteration_1/review/correctness/answer.json")
                .exists()
        );
        assert!(
            summary
                .run_dir
                .join("iteration_1/review/tests/answer.json")
                .exists()
        );
        assert!(
            summary
                .run_dir
                .join("iteration_1/review/docs/answer.json")
                .exists()
        );
    }

    #[test]
    fn pre_run_hook_writes_audit_before_review() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let mut config = test_config(&passing_verify("ok"), 1);
        config.hooks.pre_run = Some(passing_verify("pre-run"));

        let summary = run_loop_with_executor(
            &config,
            dir.path(),
            &dir.path().join("runs"),
            &FakePhaseExecutor,
        )
        .unwrap();

        assert!(summary.run_dir.join("hooks/pre_run/command.txt").exists());
        assert!(summary.run_dir.join("hooks/pre_run/result.json").exists());
    }

    #[test]
    fn failed_post_iteration_hook_keeps_run_failed_and_records_delta() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let mut config = test_config(&passing_verify("ok"), 1);
        config.hooks.post_iteration = Some(failing_verify("hook-failed"));

        let summary = run_loop_with_executor(
            &config,
            dir.path(),
            &dir.path().join("runs"),
            &FakePhaseExecutor,
        )
        .unwrap();
        let record: IterationRecord =
            serde_json::from_str(&fs::read_to_string(summary.final_record_path.unwrap()).unwrap())
                .unwrap();

        assert!(!summary.passed);
        assert!(!record.validation.passed);
        assert!(record.remaining_delta[0].contains("post_iteration hook failed"));
        assert!(
            summary
                .run_dir
                .join("iteration_1/hooks/post_iteration/result.json")
                .exists()
        );
    }
}
