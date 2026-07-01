use crate::hooks::HooksConfig;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};

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
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub review_agents: Vec<ReviewAgentConfig>,
    #[serde(default)]
    pub hooks: HooksConfig,
}

pub const RECOMMENDED_CODEX_MODELS: &[&str] =
    &["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex-spark"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowProfile {
    Default,
    QuickFix,
    TestRepair,
    DocsRepair,
    MultiReview,
}

impl WorkflowProfile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::QuickFix => "quick-fix",
            Self::TestRepair => "test-repair",
            Self::DocsRepair => "docs-repair",
            Self::MultiReview => "multi-review",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Default => "single reviewer, single writer, balanced defaults",
            Self::QuickFix => "single fast reviewer for low-risk local repairs",
            Self::TestRepair => "single reviewer focused on failing tests and coverage gaps",
            Self::DocsRepair => "single reviewer focused on docs clarity and stale guidance",
            Self::MultiReview => "three read-only reviewers: correctness, tests, docs",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "quick-fix" => Some(Self::QuickFix),
            "test-repair" => Some(Self::TestRepair),
            "docs-repair" => Some(Self::DocsRepair),
            "multi-review" => Some(Self::MultiReview),
            _ => None,
        }
    }
}

pub const BUILTIN_PROFILES: &[WorkflowProfile] = &[
    WorkflowProfile::Default,
    WorkflowProfile::QuickFix,
    WorkflowProfile::TestRepair,
    WorkflowProfile::DocsRepair,
    WorkflowProfile::MultiReview,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewAgentConfig {
    pub id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub model_reasoning_effort: Option<String>,
}

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
        let agent = self.effective_review_agents().remove(0);
        self.review_phase_config_for(&agent)
    }

    pub fn review_phase_config_for(&self, agent: &ReviewAgentConfig) -> Self {
        let mut config = self.clone();
        if let Some(model) = &agent.model {
            config.model = model.clone();
        } else if let Some(model) = &self.review_model {
            config.model = model.clone();
        }
        if let Some(reasoning) = &agent.model_reasoning_effort {
            config.model_reasoning_effort = reasoning.clone();
        }
        config.review_model = None;
        config.repair_model = None;
        config.review_agents = Vec::new();
        config
    }

    pub fn repair_phase_config(&self) -> Self {
        let mut config = self.clone();
        if let Some(model) = &self.repair_model {
            config.model = model.clone();
        }
        config.review_model = None;
        config.repair_model = None;
        config.review_agents = Vec::new();
        config
    }

    pub fn effective_profile(&self) -> WorkflowProfile {
        self.profile
            .as_deref()
            .and_then(WorkflowProfile::parse)
            .unwrap_or(WorkflowProfile::Default)
    }

    pub fn effective_review_agents(&self) -> Vec<ReviewAgentConfig> {
        if !self.review_agents.is_empty() {
            return self.review_agents.clone();
        }

        match self.effective_profile() {
            WorkflowProfile::MultiReview => vec![
                ReviewAgentConfig {
                    id: "correctness".to_string(),
                    model: Some("gpt-5.4".to_string()),
                    focus: Some("find behavior bugs, edge cases, and regression risk".to_string()),
                    model_reasoning_effort: None,
                },
                ReviewAgentConfig {
                    id: "tests".to_string(),
                    model: Some("gpt-5.4-mini".to_string()),
                    focus: Some("find missing tests and weak validation".to_string()),
                    model_reasoning_effort: None,
                },
                ReviewAgentConfig {
                    id: "docs".to_string(),
                    model: Some("gpt-5.4-mini".to_string()),
                    focus: Some("find docs, usage, and acceptance gaps".to_string()),
                    model_reasoning_effort: None,
                },
            ],
            WorkflowProfile::QuickFix => vec![default_agent(
                "quick",
                "find the smallest safe repair for a low-risk change",
            )],
            WorkflowProfile::TestRepair => vec![default_agent(
                "tests",
                "focus on failing tests, test gaps, and validation output",
            )],
            WorkflowProfile::DocsRepair => vec![default_agent(
                "docs",
                "focus on documentation clarity, stale guidance, and examples",
            )],
            WorkflowProfile::Default => vec![default_agent(
                "default",
                "review correctness, tests, docs, and safety at a balanced depth",
            )],
        }
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
        if let Some(profile) = &self.profile {
            if WorkflowProfile::parse(profile).is_none() {
                bail!("unsupported profile: {profile}");
            }
        }
        let mut ids = HashSet::new();
        for (index, agent) in self.review_agents.iter().enumerate() {
            validate_agent_id(index, &agent.id)?;
            if !ids.insert(agent.id.as_str()) {
                bail!("duplicate review agent id: {}", agent.id);
            }
            if let Some(model) = &agent.model {
                validate_model_value(&format!("review_agents[{index}].model"), model)?;
            }
            if let Some(reasoning) = &agent.model_reasoning_effort {
                if !["low", "medium", "high", "xhigh"].contains(&reasoning.as_str()) {
                    bail!("unsupported review_agents[{index}].model_reasoning_effort: {reasoning}");
                }
            }
        }
        for (name, command) in self.hooks.commands() {
            if command.is_some_and(|value| value.trim().is_empty()) {
                bail!("hooks.{name} must not be empty");
            }
        }
        Ok(())
    }
}

fn default_agent(id: &str, focus: &str) -> ReviewAgentConfig {
    ReviewAgentConfig {
        id: id.to_string(),
        model: None,
        focus: Some(focus.to_string()),
        model_reasoning_effort: None,
    }
}

fn validate_model_value(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{name} is required");
    }
    Ok(())
}

fn validate_agent_id(index: usize, value: &str) -> Result<()> {
    if value.trim().is_empty()
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("review_agents[{index}].id must contain only ASCII letters, numbers, '-' or '_'");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::HooksConfig;
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
            profile: None,
            review_agents: Vec::new(),
            hooks: HooksConfig::default(),
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
        assert_eq!(config.effective_profile(), WorkflowProfile::Default);
        assert_eq!(config.effective_review_agents().len(), 1);
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
    fn loads_profile_review_agents_and_hooks() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("loop.json");
        fs::write(
            &path,
            r#"{
              "artifact": "README.md",
              "goal": "repair docs",
              "verify": "cargo test",
              "profile": "multi-review",
              "review_agents": [
                { "id": "correctness", "model": "gpt-5.4", "focus": "find bugs" },
                { "id": "tests", "model": "gpt-5.4-mini", "focus": "find missing tests" }
              ],
              "hooks": {
                "pre_run": "git diff --check",
                "post_iteration": "cargo test --quiet",
                "pre_apply": "cargo fmt --check",
                "post_apply": "cargo test --locked --all-targets",
                "on_failure": "printf failed"
              }
            }"#,
        )
        .unwrap();

        let config = LoopConfig::from_path(&path).unwrap();

        assert_eq!(config.effective_profile(), WorkflowProfile::MultiReview);
        assert_eq!(config.review_agents.len(), 2);
        assert_eq!(config.review_agents[0].id, "correctness");
        assert_eq!(config.hooks.pre_run.as_deref(), Some("git diff --check"));
        assert_eq!(
            config.hooks.post_apply.as_deref(),
            Some("cargo test --locked --all-targets")
        );
    }

    #[test]
    fn multi_review_profile_expands_default_agents() {
        let config = LoopConfig {
            profile: Some("multi-review".to_string()),
            ..valid_config()
        };

        let agents = config.effective_review_agents();

        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].id, "correctness");
        assert_eq!(agents[1].id, "tests");
        assert_eq!(agents[2].id, "docs");
    }

    #[test]
    fn review_phase_config_for_agent_prefers_agent_model() {
        let config = LoopConfig {
            review_model: Some("gpt-5.4".to_string()),
            ..valid_config()
        };
        let agent = ReviewAgentConfig {
            id: "tests".to_string(),
            model: Some("gpt-5.4-mini".to_string()),
            focus: Some("find missing tests".to_string()),
            model_reasoning_effort: Some("medium".to_string()),
        };

        let phase = config.review_phase_config_for(&agent);

        assert_eq!(phase.model, "gpt-5.4-mini");
        assert_eq!(phase.model_reasoning_effort, "medium");
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
    fn rejects_unsupported_profile() {
        let config = LoopConfig {
            profile: Some("wide-open".to_string()),
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unsupported profile"));
    }

    #[test]
    fn rejects_unsafe_review_agent_id() {
        let config = LoopConfig {
            review_agents: vec![ReviewAgentConfig {
                id: "../escape".to_string(),
                model: None,
                focus: None,
                model_reasoning_effort: None,
            }],
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("review_agents[0].id"));
    }

    #[test]
    fn rejects_empty_hook_command() {
        let config = LoopConfig {
            hooks: HooksConfig {
                pre_run: Some(" ".to_string()),
                ..HooksConfig::default()
            },
            ..valid_config()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("hooks.pre_run"));
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
