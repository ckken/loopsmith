use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyResult {
    pub passed: bool,
    pub returncode: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_verify(command: &str, cwd: &Path) -> Result<VerifyResult> {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .current_dir(cwd)
            .output()?
    } else {
        Command::new("sh")
            .args(["-c", command])
            .current_dir(cwd)
            .output()?
    };

    let returncode = output.status.code().unwrap_or(1);
    Ok(VerifyResult {
        passed: output.status.success(),
        returncode,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn verify_passes_for_zero_exit() {
        let dir = tempdir().unwrap();
        let result = run_verify("printf 123", dir.path()).unwrap();
        assert!(result.passed);
        assert_eq!(result.returncode, 0);
        assert!(result.stdout.contains("123"));
    }

    #[test]
    fn verify_fails_for_nonzero_exit() {
        let dir = tempdir().unwrap();
        let result = run_verify("exit 7", dir.path()).unwrap();
        assert!(!result.passed);
        assert_eq!(result.returncode, 7);
    }
}
