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
