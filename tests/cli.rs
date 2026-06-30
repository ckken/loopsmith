use assert_cmd::Command;

#[test]
fn cli_help_lists_available_commands() {
    let assert = Command::cargo_bin("loopsmith")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("run"));
    assert!(stdout.contains("inspect"));
    assert!(stdout.contains("diff"));
    assert!(stdout.contains("apply"));
}

#[test]
fn cli_run_with_missing_config_fails_before_codex_exec() {
    let assert = Command::cargo_bin("loopsmith")
        .unwrap()
        .args(["run", "--config", "missing.json"])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();

    assert!(stderr.contains("failed to read config"));
}

#[test]
fn cli_inspect_with_missing_run_fails() {
    let dir = tempfile::tempdir().unwrap();
    let assert = Command::cargo_bin("loopsmith")
        .unwrap()
        .args([
            "inspect",
            "missing-run",
            "--runs-dir",
            dir.path().join("runs").to_str().unwrap(),
        ])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();

    assert!(stderr.contains("missing-run"));
}
