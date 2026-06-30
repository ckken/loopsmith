use crate::{
    codex_exec::run_codex_json,
    config::LoopConfig,
    record::{IterationRecord, write_record},
    schema::{repair_schema, review_schema},
    verify::run_verify,
    workspace::prepare_iteration_workspace,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopSummary {
    pub passed: bool,
    pub iterations: usize,
    pub final_record_path: Option<PathBuf>,
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
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("run-{seconds}")
}

pub fn run_loop(config: &LoopConfig, source_root: &Path, runs_dir: &Path) -> Result<LoopSummary> {
    run_loop_with_executor(config, source_root, runs_dir, &CodexPhaseExecutor)
}

pub trait PhaseExecutor {
    fn review(&self, config: &LoopConfig, artifact_text: &str, run_dir: &Path) -> Result<Value>;

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
    fn review(&self, config: &LoopConfig, artifact_text: &str, run_dir: &Path) -> Result<Value> {
        let phase_config = config.review_phase_config();
        run_codex_json(
            &review_prompt(config, artifact_text),
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

fn review_prompt(config: &LoopConfig, artifact_text: &str) -> String {
    format!(
        "You are reviewing an artifact before repair.\nArtifact: {}\nGoal: {}\nDo not edit files. Return concise structured findings.\n\n{}",
        config.artifact, config.goal, artifact_text
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
    let root = runs_dir.join(run_id());
    let mut workspace_source = source_root.to_path_buf();
    let mut remaining_delta = Vec::new();
    let mut final_record_path = None;

    for iteration in 1..=config.max_iterations {
        let iteration_dir = root.join(format!("iteration_{iteration}"));
        let copied =
            prepare_iteration_workspace(&workspace_source, &config.artifact, &iteration_dir)?;
        let workspace_root = iteration_dir.join("workspace");
        let artifact_text = fs::read_to_string(&copied)?;

        let review = executor.review(config, &artifact_text, &iteration_dir.join("review"))?;
        let repair = executor.repair(
            config,
            &copied,
            &review,
            &remaining_delta,
            iteration,
            &iteration_dir.join("repair"),
        )?;
        let validation = run_verify(&config.verify, &workspace_root)?;
        remaining_delta = if validation.passed {
            vec![]
        } else {
            vec![if validation.stderr.trim().is_empty() {
                validation.stdout.clone()
            } else {
                validation.stderr.clone()
            }]
        };

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
        workspace_source = workspace_root;

        if should_stop(
            iteration,
            config.max_iterations,
            record.validation.passed,
            &remaining_delta,
        ) {
            return Ok(LoopSummary {
                passed: record.validation.passed,
                iterations: iteration,
                final_record_path,
            });
        }
    }

    Ok(LoopSummary {
        passed: false,
        iterations: config.max_iterations,
        final_record_path,
    })
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
        let config = test_config("printf ok", 3);

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
    fn failed_verification_writes_remaining_delta() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let config = test_config("printf failure-details; exit 7", 1);

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
        let config = test_config("printf still-bad; exit 7", 2);

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
        let config = test_config("printf first-failure; exit 7", 2);
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
}
