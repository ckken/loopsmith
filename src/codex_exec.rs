use crate::config::LoopConfig;
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

pub fn codex_args(schema_file: &Path, answer_file: &Path, config: &LoopConfig) -> Vec<String> {
    vec![
        "-a".to_string(),
        config.approval_policy.clone(),
        "exec".to_string(),
        "--model".to_string(),
        config.model.clone(),
        "--config".to_string(),
        format!(
            "model_reasoning_effort=\"{}\"",
            config.model_reasoning_effort
        ),
        "--sandbox".to_string(),
        config.sandbox.clone(),
        "--output-schema".to_string(),
        schema_file.display().to_string(),
        "--output-last-message".to_string(),
        answer_file.display().to_string(),
        "-".to_string(),
    ]
}

pub fn build_codex_command(schema_file: &Path, answer_file: &Path, config: &LoopConfig) -> Command {
    let mut command = Command::new("codex");
    command.args(codex_args(schema_file, answer_file, config));
    command
}

pub fn run_codex_json(
    prompt: &str,
    schema: &Value,
    run_dir: &Path,
    config: &LoopConfig,
) -> Result<Value> {
    fs::create_dir_all(run_dir)?;
    let prompt_file = run_dir.join("prompt.txt");
    let schema_file = run_dir.join("schema.json");
    let answer_file = run_dir.join("answer.json");

    fs::write(&prompt_file, prompt)?;
    fs::write(&schema_file, serde_json::to_string_pretty(schema)?)?;

    let mut child = build_codex_command(&schema_file, &answer_file, config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn codex exec")?;

    {
        let mut stdin = child.stdin.take().context("failed to open codex stdin")?;
        stdin.write_all(prompt.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    fs::write(run_dir.join("stdout.txt"), &output.stdout)?;
    fs::write(run_dir.join("stderr.txt"), &output.stderr)?;

    if !output.status.success() {
        anyhow::bail!("codex exec failed with status {}", output.status);
    }

    let answer = fs::read_to_string(&answer_file)
        .with_context(|| format!("missing codex answer {}", answer_file.display()))?;
    Ok(serde_json::from_str(&answer)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> LoopConfig {
        LoopConfig {
            artifact: "README.md".to_string(),
            goal: "repair docs".to_string(),
            verify: "cargo test".to_string(),
            max_iterations: 3,
            model: "gpt-5.5".to_string(),
            review_model: None,
            repair_model: None,
            model_reasoning_effort: "high".to_string(),
            sandbox: "workspace-write".to_string(),
            approval_policy: "never".to_string(),
            profile: None,
            review_agents: Vec::new(),
            hooks: crate::hooks::HooksConfig::default(),
        }
    }

    #[test]
    fn codex_args_include_schema_and_output_file() {
        let args = codex_args(
            &PathBuf::from("schema.json"),
            &PathBuf::from("answer.json"),
            &test_config(),
        );

        assert_eq!(args[0], "-a");
        assert_eq!(args[1], "never");
        assert_eq!(args[2], "exec");
        assert_eq!(&args[0..3], ["-a", "never", "exec"]);
        assert!(args.contains(&"model_reasoning_effort=\"high\"".to_string()));
        assert!(args.contains(&"--sandbox".to_string()));
        assert!(args.contains(&"workspace-write".to_string()));
        assert!(args.contains(&"--output-schema".to_string()));
        assert!(args.contains(&"--output-last-message".to_string()));
        assert!(!args.contains(&"--ask-for-approval".to_string()));
        assert_eq!(args.last().unwrap(), "-");
    }
}
