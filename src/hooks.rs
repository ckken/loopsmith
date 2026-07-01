use crate::verify::{VerifyResult, run_verify};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HooksConfig {
    #[serde(default)]
    pub pre_run: Option<String>,
    #[serde(default)]
    pub post_iteration: Option<String>,
    #[serde(default)]
    pub pre_apply: Option<String>,
    #[serde(default)]
    pub post_apply: Option<String>,
    #[serde(default)]
    pub on_failure: Option<String>,
}

impl HooksConfig {
    pub fn commands(&self) -> [(&'static str, Option<&str>); 5] {
        [
            ("pre_run", self.pre_run.as_deref()),
            ("post_iteration", self.post_iteration.as_deref()),
            ("pre_apply", self.pre_apply.as_deref()),
            ("post_apply", self.post_apply.as_deref()),
            ("on_failure", self.on_failure.as_deref()),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookAudit {
    pub event: String,
    pub command: String,
    pub result: VerifyResult,
}

pub fn run_hook(event: &str, command: &str, cwd: &Path, output_dir: &Path) -> Result<VerifyResult> {
    fs::create_dir_all(output_dir)?;
    fs::write(output_dir.join("command.txt"), command)?;

    let result = run_verify(command, cwd)?;
    fs::write(output_dir.join("stdout.txt"), &result.stdout)?;
    fs::write(output_dir.join("stderr.txt"), &result.stderr)?;
    fs::write(
        output_dir.join("result.json"),
        serde_json::to_string_pretty(&HookAudit {
            event: event.to_string(),
            command: command.to_string(),
            result: result.clone(),
        })?,
    )?;
    Ok(result)
}

pub fn run_required_hook(
    event: &str,
    command: &str,
    cwd: &Path,
    output_dir: &Path,
) -> Result<VerifyResult> {
    let result = run_hook(event, command, cwd, output_dir)?;
    if !result.passed {
        bail!("{event} hook failed with status {}", result.returncode);
    }
    Ok(result)
}
