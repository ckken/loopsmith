use loopsmith::hooks::{run_hook, run_required_hook};
use std::fs;
use tempfile::tempdir;

fn passing_command(message: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("echo {message}")
    } else {
        format!("printf {message}")
    }
}

fn failing_command(message: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("echo {message} && exit /B 9")
    } else {
        format!("printf {message}; exit 9")
    }
}

#[test]
fn hook_run_writes_audit_files() {
    let dir = tempdir().unwrap();
    let output_dir = dir.path().join("hooks/pre_run");

    let result = run_hook(
        "pre_run",
        &passing_command("hook-ok"),
        dir.path(),
        &output_dir,
    )
    .unwrap();

    assert!(result.passed);
    assert_eq!(
        fs::read_to_string(output_dir.join("command.txt")).unwrap(),
        passing_command("hook-ok")
    );
    assert!(
        fs::read_to_string(output_dir.join("stdout.txt"))
            .unwrap()
            .contains("hook-ok")
    );
    assert!(output_dir.join("stderr.txt").exists());
    let audit = fs::read_to_string(output_dir.join("result.json")).unwrap();
    assert!(audit.contains("pre_run"));
    assert!(audit.contains("hook-ok"));
}

#[test]
fn required_hook_fails_on_nonzero_exit() {
    let dir = tempdir().unwrap();
    let output_dir = dir.path().join("hooks/pre_apply");

    let err = run_required_hook(
        "pre_apply",
        &failing_command("blocked"),
        dir.path(),
        &output_dir,
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("pre_apply hook failed"));
    assert!(output_dir.join("result.json").exists());
}
