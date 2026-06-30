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
    #[serde(default)]
    pub review_model: Option<String>,
    #[serde(default)]
    pub repair_model: Option<String>,
    #[serde(default = "default_model_reasoning_effort")]
    pub model_reasoning_effort: String,
    #[serde(default = "default_sandbox")]
    pub sandbox: String,
    #[serde(default = "default_approval_policy")]
    pub approval_policy: String,
}

pub const RECOMMENDED_CODEX_MODELS: &[&str] =
    &["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex-spark"];

fn default_max_iterations() -> usize {
    3
}

fn default_model() -> String {
    "gpt-5.5".to_string()
}

fn default_model_reasoning_effort() -> String {
    "low".to_string()
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

    pub fn review_phase_config(&self) -> Self {
        let mut config = self.clone();
        if let Some(model) = &self.review_model {
            config.model = model.clone();
        }
        config.review_model = None;
        config.repair_model = None;
        config
    }

    pub fn repair_phase_config(&self) -> Self {
        let mut config = self.clone();
        if let Some(model) = &self.repair_model {
            config.model = model.clone();
        }
        config.review_model = None;
        config.repair_model = None;
        config
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
        validate_model_value("model", &self.model)?;
        if let Some(model) = &self.review_model {
            validate_model_value("review_model", model)?;
        }
        if let Some(model) = &self.repair_model {
            validate_model_value("repair_model", model)?;
        }
        if !["low", "medium", "high", "xhigh"].contains(&self.model_reasoning_effort.as_str()) {
            bail!(
                "unsupported model_reasoning_effort: {}",
                self.model_reasoning_effort
            );
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

fn validate_model_value(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{name} is required");
    }
    Ok(())
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
            review_model: None,
            repair_model: None,
            model_reasoning_effort: "low".to_string(),
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
        assert_eq!(config.model, "gpt-5.5");
        assert_eq!(config.review_model, None);
        assert_eq!(config.repair_model, None);
        assert_eq!(config.model_reasoning_effort, "low");
        assert_eq!(config.sandbox, "workspace-write");
        assert_eq!(config.approval_policy, "never");
    }

    #[test]
    fn loads_phase_models_and_reasoning_effort() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("loop.json");
        fs::write(
            &path,
            r#"{
              "artifact": "README.md",
              "goal": "repair docs",
              "verify": "cargo test",
              "model": "gpt-5.5",
              "review_model": "gpt-5.4",
              "repair_model": "gpt-5.4-mini",
              "model_reasoning_effort": "medium"
            }"#,
        )
        .unwrap();

        let config = LoopConfig::from_path(&path).unwrap();

        assert_eq!(config.model, "gpt-5.5");
        assert_eq!(config.review_phase_config().model, "gpt-5.4");
        assert_eq!(config.repair_phase_config().model, "gpt-5.4-mini");
        assert_eq!(config.model_reasoning_effort, "medium");
    }

    #[test]
    fn recommended_model_list_includes_current_codex_models() {
        assert!(RECOMMENDED_CODEX_MODELS.contains(&"gpt-5.5"));
        assert!(RECOMMENDED_CODEX_MODELS.contains(&"gpt-5.4"));
        assert!(RECOMMENDED_CODEX_MODELS.contains(&"gpt-5.4-mini"));
        assert!(RECOMMENDED_CODEX_MODELS.contains(&"gpt-5.3-codex-spark"));
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
    fn rejects_missing_model() {
        let config = LoopConfig {
            model: " ".to_string(),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("model"));
    }

    #[test]
    fn rejects_missing_phase_model() {
        let config = LoopConfig {
            review_model: Some("".to_string()),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("review_model"));
    }

    #[test]
    fn rejects_unsupported_reasoning_effort() {
        let config = LoopConfig {
            model_reasoning_effort: "ultra".to_string(),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unsupported model_reasoning_effort"));
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
