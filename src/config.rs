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

    fn valid_config() -> LoopConfig {
        LoopConfig {
            artifact: "README.md".to_string(),
            goal: "repair docs".to_string(),
            verify: "cargo test".to_string(),
            max_iterations: 3,
            model: "gpt-5.5".to_string(),
            sandbox: "workspace-write".to_string(),
            approval_policy: "never".to_string(),
        }
    }

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
            verify: "".to_string(),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("verify"));
    }

    #[test]
    fn rejects_missing_artifact() {
        let config = LoopConfig {
            artifact: " ".to_string(),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("artifact"));
    }

    #[test]
    fn rejects_missing_goal() {
        let config = LoopConfig {
            goal: "\t".to_string(),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("goal"));
    }

    #[test]
    fn rejects_zero_max_iterations() {
        let config = LoopConfig {
            max_iterations: 0,
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("max_iterations"));
    }

    #[test]
    fn rejects_unsupported_sandbox() {
        let config = LoopConfig {
            sandbox: "full-auto".to_string(),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unsupported sandbox"));
    }

    #[test]
    fn rejects_unsupported_approval_policy() {
        let config = LoopConfig {
            approval_policy: "ask-for-approval".to_string(),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unsupported approval_policy"));
    }

    #[test]
    fn from_path_reports_missing_file() {
        let dir = tempdir().unwrap();

        let err = LoopConfig::from_path(dir.path().join("missing.json"))
            .unwrap_err()
            .to_string();

        assert!(err.contains("failed to read config"));
    }

    #[test]
    fn from_path_reports_invalid_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("loop.json");
        fs::write(&path, "{not-json").unwrap();

        let err = LoopConfig::from_path(&path).unwrap_err().to_string();

        assert!(err.contains("failed to parse loop config JSON"));
    }
}
