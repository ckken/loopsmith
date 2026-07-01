use loopsmith::{
    hooks::HooksConfig,
    record::{IterationRecord, write_record},
    run_state::{
        RunManifest, RunStatus, apply_run, artifact_hash, diff_run, inspect_run, latest_run_id,
        write_manifest_and_index,
    },
    verify::VerifyResult,
};
use serde_json::json;
use std::{fs, path::Path};
use tempfile::tempdir;

fn pass_command() -> String {
    if cfg!(target_os = "windows") {
        "echo ok".to_string()
    } else {
        "printf ok".to_string()
    }
}

fn verify_readme_contains(text: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("findstr {text} README.md")
    } else {
        format!("grep {text} README.md")
    }
}

fn write_fake_record(path: &Path, iteration: usize, passed: bool) {
    let record = IterationRecord {
        iteration,
        review: json!({"findings": []}),
        repair: json!({
            "changes_made": ["updated artifact"],
            "updated_artifact_path": "README.md"
        }),
        validation: VerifyResult {
            passed,
            returncode: if passed { 0 } else { 1 },
            stdout: String::new(),
            stderr: String::new(),
        },
        remaining_delta: vec![],
    };
    write_record(&record, path).unwrap();
}

fn create_fake_run(
    source_root: &Path,
    runs_dir: &Path,
    run_id: &str,
    candidate_text: &str,
    verify: &str,
    status: RunStatus,
    started_at_unix: u64,
) -> RunManifest {
    fs::write(source_root.join("README.md"), "old\n").unwrap();

    let run_dir = runs_dir.join(run_id);
    let workspace = run_dir.join("iteration_1/workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("README.md"), candidate_text).unwrap();
    write_fake_record(
        &run_dir.join("iteration_1/record.json"),
        1,
        status == RunStatus::Passed,
    );

    let manifest = RunManifest {
        run_id: run_id.to_string(),
        artifact: "README.md".to_string(),
        goal: "update README".to_string(),
        verify: verify.to_string(),
        max_iterations: 3,
        source_artifact_hash: artifact_hash(&source_root.join("README.md")).unwrap(),
        started_at_unix,
        finished_at_unix: Some(started_at_unix + 1),
        status,
        iterations: 1,
        final_record_path: Some(format!("{run_id}/iteration_1/record.json")),
        final_artifact_path: Some(format!("{run_id}/iteration_1/workspace/README.md")),
        summary_path: Some(format!("{run_id}/summary.md")),
        hooks: HooksConfig::default(),
    };
    write_manifest_and_index(&manifest, runs_dir).unwrap();
    manifest
}

#[test]
fn workflow_latest_run_id_and_inspect_read_manifest_and_records() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();

    create_fake_run(
        &source,
        &runs,
        "run-1",
        "new one\n",
        &pass_command(),
        RunStatus::Failed,
        1,
    );
    create_fake_run(
        &source,
        &runs,
        "run-2",
        "new two\n",
        &pass_command(),
        RunStatus::Passed,
        2,
    );

    assert_eq!(latest_run_id(&runs).unwrap(), "run-2");

    let inspection = inspect_run(&runs, Some("run-1")).unwrap();
    assert_eq!(inspection.manifest.run_id, "run-1");
    assert_eq!(inspection.records.len(), 1);
    assert_eq!(inspection.records[0].iteration, 1);
}

#[test]
fn workflow_diff_run_shows_source_and_candidate_lines() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();
    create_fake_run(
        &source,
        &runs,
        "run-1",
        "new\n",
        &pass_command(),
        RunStatus::Passed,
        1,
    );

    let diff = diff_run(&source, &runs, Some("run-1"), Some(1)).unwrap();

    assert!(diff.changed);
    assert!(diff.diff.contains("-old"));
    assert!(diff.diff.contains("+new"));
}

#[test]
fn workflow_apply_run_dry_run_keeps_source_unchanged() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();
    create_fake_run(
        &source,
        &runs,
        "run-1",
        "new\n",
        &pass_command(),
        RunStatus::Passed,
        1,
    );

    let outcome = apply_run(&source, &runs, Some("run-1"), Some(1), true, false, false).unwrap();

    assert!(outcome.dry_run);
    assert!(!outcome.applied);
    assert_eq!(
        fs::read_to_string(source.join("README.md")).unwrap(),
        "old\n"
    );
}

#[test]
fn workflow_apply_run_replaces_source_when_hash_matches() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();
    create_fake_run(
        &source,
        &runs,
        "run-1",
        "new\n",
        &pass_command(),
        RunStatus::Passed,
        1,
    );

    let outcome = apply_run(&source, &runs, Some("run-1"), Some(1), false, false, false).unwrap();

    assert!(outcome.applied);
    assert_eq!(
        fs::read_to_string(source.join("README.md")).unwrap(),
        "new\n"
    );
}

#[test]
fn workflow_apply_run_blocks_when_source_changed_since_run_started() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();
    create_fake_run(
        &source,
        &runs,
        "run-1",
        "new\n",
        &pass_command(),
        RunStatus::Passed,
        1,
    );
    fs::write(source.join("README.md"), "human change\n").unwrap();

    let err = apply_run(&source, &runs, Some("run-1"), Some(1), false, false, false)
        .unwrap_err()
        .to_string();

    assert!(err.contains("source artifact changed"));
}

#[test]
fn workflow_apply_run_force_allows_source_hash_mismatch() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();
    create_fake_run(
        &source,
        &runs,
        "run-1",
        "new\n",
        &pass_command(),
        RunStatus::Passed,
        1,
    );
    fs::write(source.join("README.md"), "human change\n").unwrap();

    let outcome = apply_run(&source, &runs, Some("run-1"), Some(1), false, true, false).unwrap();

    assert!(outcome.applied);
    assert_eq!(
        fs::read_to_string(source.join("README.md")).unwrap(),
        "new\n"
    );
}

#[test]
fn workflow_apply_run_verify_after_apply_runs_manifest_verify_command() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();
    create_fake_run(
        &source,
        &runs,
        "run-1",
        "new\n",
        &verify_readme_contains("new"),
        RunStatus::Passed,
        1,
    );

    let outcome = apply_run(&source, &runs, Some("run-1"), Some(1), false, false, true).unwrap();

    assert!(outcome.applied);
    assert!(outcome.verification.unwrap().passed);
}

fn write_marker_command(name: &str, text: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("echo {text}> {name}")
    } else {
        format!("printf {text} > {name}")
    }
}

#[test]
fn workflow_apply_run_executes_pre_and_post_apply_hooks() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source");
    let runs = dir.path().join("runs");
    fs::create_dir_all(&source).unwrap();
    let mut manifest = create_fake_run(
        &source,
        &runs,
        "run-1",
        "new\n",
        &verify_readme_contains("new"),
        RunStatus::Passed,
        1,
    );
    manifest.hooks.pre_apply = Some(write_marker_command("pre_apply.txt", "pre"));
    manifest.hooks.post_apply = Some(write_marker_command("post_apply.txt", "post"));
    write_manifest_and_index(&manifest, &runs).unwrap();

    let outcome = apply_run(&source, &runs, Some("run-1"), Some(1), false, false, true).unwrap();

    assert!(outcome.applied);
    assert_eq!(
        fs::read_to_string(source.join("pre_apply.txt")).unwrap(),
        "pre"
    );
    assert_eq!(
        fs::read_to_string(source.join("post_apply.txt")).unwrap(),
        "post"
    );
    assert!(
        runs.join("run-1/hooks/apply/pre_apply/result.json")
            .exists()
    );
    assert!(
        runs.join("run-1/hooks/apply/post_apply/result.json")
            .exists()
    );
}
