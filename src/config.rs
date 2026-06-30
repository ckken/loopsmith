use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopConfig {
    pub artifact: String,
    pub goal: String,
    pub verify: String,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_sandbox")]
    pub sandbox: String,
    #[serde(default = "default_approval_policy")]
    pub approval_policy: String,
}

fn default_max_iterations() -> usize {
    3
}

fn default_model() -> String {
    "gpt-5.5".to_string()
}

fn default_sandbox() -> String {
    "workspace-write".to_string()
}

fn default_approval_policy() -> String {
    "never".to_string()
}

impl LoopConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let text = fs::read_to_string(path.as_ref())
            .with_context(|| format!("failed to read config {}", path.as_ref().display()))?;
        let config: Self =
            serde_json::from_str(&text).context("failed to parse loop config JSON")?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.artifact.trim().is_empty() {
            bail!("artifact is required");
        }
        if self.goal.trim().is_empty() {
            bail!("goal is required");
        }
        if self.verify.trim().is_empty() {
            bail!("verify is required");
        }
        if self.max_iterations == 0 {
            bail!("max_iterations must be at least 1");
        }
        if !["read-only", "workspace-write", "danger-full-access"].contains(&self.sandbox.as_str())
        {
            bail!("unsupported sandbox: {}", self.sandbox);
        }
        if !["untrusted", "on-request", "never"].contains(&self.approval_policy.as_str()) {
            bail!("unsupported approval_policy: {}", self.approval_policy);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_minimal_loop_config_with_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("loop.json");
        fs::write(
            &path,
            r#"{
              "artifact": "README.md",
              "goal": "remove stale setup guidance",
              "verify": "cargo test"
            }"#,
        )
        .unwrap();

        let config = LoopConfig::from_path(&path).unwrap();

        assert_eq!(config.artifact, "README.md");
        assert_eq!(config.goal, "remove stale setup guidance");
        assert_eq!(config.verify, "cargo test");
        assert_eq!(config.max_iterations, 3);
        assert_eq!(config.sandbox, "workspace-write");
        assert_eq!(config.approval_policy, "never");
    }

    #[test]
    fn rejects_missing_verify() {
        let config = LoopConfig {
            artifact: "README.md".to_string(),
            goal: "repair docs".to_string(),
            verify: "".to_string(),
            max_iterations: 3,
            model: "gpt-5.5".to_string(),
            sandbox: "workspace-write".to_string(),
            approval_policy: "never".to_string(),
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("verify"));
    }
}
