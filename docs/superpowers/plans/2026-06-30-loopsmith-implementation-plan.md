# Codex Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 Rust 构建一个本地 CLI，负责运行可审计的 Codex 迭代修复闭环。

**Architecture:** Rust CLI 只负责外层确定性编排：读取配置、准备候选工作区、调用 `codex exec`、执行机械验证、写入审计记录和判断停止条件。Codex 只作为 review/repair 引擎，通过 `--output-schema` 返回结构化 JSON；源文件默认不被直接修改，修复先落在 `runs/<run-id>/iteration_N/workspace/` 中。

**Tech Stack:** Rust 1.85+、edition 2024、`clap`、`serde`、`serde_json`、`anyhow`、`tempfile`、`assert_cmd`、Codex CLI。

## Global Constraints

- 默认 sandbox 必须是 `workspace-write`。
- 默认 approval policy 只允许嵌套 `codex exec` 使用 `never`。
- 第一版不依赖 Notebook runtime，必须支持普通文本和代码文件。
- 不允许只靠 LLM judge 判定成功；每个 recipe 必须配置机械 `verify` 命令。
- 所有运行产物写到 `runs/<run-id>/`，并保持 `runs/` 不进入 git。
- 第一版保持小闭环：不做 daemon、不做 UI、不做数据库、不做远程调度。
- 默认只修改候选工作区；将 patch 应用回源文件必须作为后续显式能力。

---

## 技术取舍

选择 Rust 的理由：

- 单二进制分发能力强，适合作为长期 CLI 工具。
- 类型系统适合约束 config、record、schema、状态机和错误路径。
- 进程编排、文件系统操作、JSON 序列化都足够成熟。
- 后续如果要做插件协议、长期版本化、CI 分发，Rust 的边界更清晰。

代价：

- MVP 编写速度慢于 Go。
- 需要更早设计模块边界和错误类型。
- 测试里要避免真实调用 `codex exec`，需要把命令构造和执行层拆开。

## File Structure

- Create `Cargo.toml`: crate metadata、依赖、dev-dependencies。
- Create `src/main.rs`: CLI 入口。
- Create `src/lib.rs`: 模块导出。
- Create `src/config.rs`: `LoopConfig` 加载、默认值、校验。
- Create `src/schema.rs`: review/repair JSON schema。
- Create `src/codex_exec.rs`: `codex exec` 命令构造与 JSON 输出读取。
- Create `src/verify.rs`: 机械验证命令执行。
- Create `src/record.rs`: `record.json` 与 run artifact 写入。
- Create `src/workspace.rs`: run workspace 创建、artifact 复制、diff 生成入口。
- Create `src/runner.rs`: 单轮和多轮 orchestration、停止条件。
- Create `examples/plaintext-loop.json`: 最小配置示例。
- Modify `README.md`: Rust 版本定位和本地命令。

### Task 1: Rust Project Skeleton And Config

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/config.rs`
- Create: `examples/plaintext-loop.json`

**Interfaces:**
- Produces: `LoopConfig::from_path(path: impl AsRef<Path>) -> anyhow::Result<LoopConfig>`
- Produces: `LoopConfig::validate(&self) -> anyhow::Result<()>`
- Consumes: no earlier project code

- [ ] **Step 1: Write the failing config tests**

Create `src/config.rs` with tests first:

```rust
use anyhow::{bail, Context, Result};
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

fn default_max_iterations() -> usize { 3 }
fn default_model() -> String { "gpt-5.5".to_string() }
fn default_sandbox() -> String { "workspace-write".to_string() }
fn default_approval_policy() -> String { "never".to_string() }

impl LoopConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let text = fs::read_to_string(path.as_ref())
            .with_context(|| format!("failed to read config {}", path.as_ref().display()))?;
        let config: Self = serde_json::from_str(&text).context("failed to parse loop config JSON")?;
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
        if !["read-only", "workspace-write", "danger-full-access"].contains(&self.sandbox.as_str()) {
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test config --quiet`

Expected: FAIL because `Cargo.toml` and module wiring do not exist.

- [ ] **Step 3: Add Cargo project metadata**

Create `Cargo.toml`:

```toml
[package]
name = "loopsmith"
version = "0.3.1"
edition = "2024"
rust-version = "1.85"
description = "Auditable iterative repair loops driven by Codex CLI"
license = "MIT"

[[bin]]
name = "loopsmith"
path = "src/main.rs"

[dependencies]
anyhow = "1.0"
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[dev-dependencies]
assert_cmd = "2.0"
tempfile = "3.10"
```

Create `src/lib.rs`:

```rust
pub mod config;
```

- [ ] **Step 4: Add example config**

Create `examples/plaintext-loop.json`:

```json
{
  "artifact": "README.md",
  "goal": "Make the README clearer and remove stale setup guidance.",
  "verify": "cargo test --quiet",
  "max_iterations": 3,
  "model": "gpt-5.5",
  "sandbox": "workspace-write",
  "approval_policy": "never"
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test config --quiet`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/lib.rs src/config.rs examples/plaintext-loop.json
git commit -m "feat: add rust loop config"
```

### Task 2: JSON Schemas And Codex Exec Adapter

**Files:**
- Create: `src/schema.rs`
- Create: `src/codex_exec.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `LoopConfig`
- Produces: `review_schema() -> serde_json::Value`
- Produces: `repair_schema() -> serde_json::Value`
- Produces: `build_codex_command(schema_file: &Path, answer_file: &Path, config: &LoopConfig) -> std::process::Command`
- Produces: `run_codex_json(prompt: &str, schema: &Value, run_dir: &Path, config: &LoopConfig) -> anyhow::Result<Value>`

- [ ] **Step 1: Write schema and command construction tests**

Create `src/schema.rs`:

```rust
use serde_json::{json, Value};

fn object_schema(properties: Value, required: Vec<&str>) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

pub fn review_schema() -> Value {
    let finding = object_schema(
        json!({
            "artifact": {"type": "string"},
            "issue_type": {"type": "string"},
            "severity": {"type": "string"},
            "description": {"type": "string"},
            "suggested_fix_direction": {"type": "string"}
        }),
        vec!["artifact", "issue_type", "severity", "description", "suggested_fix_direction"],
    );
    object_schema(json!({"findings": {"type": "array", "items": finding}}), vec!["findings"])
}

pub fn repair_schema() -> Value {
    object_schema(
        json!({
            "artifact": {"type": "string"},
            "iteration": {"type": "integer"},
            "changes_made": {"type": "array", "items": {"type": "string"}},
            "unresolved_items": {"type": "array", "items": {"type": "string"}},
            "updated_artifact_path": {"type": "string"}
        }),
        vec!["artifact", "iteration", "changes_made", "unresolved_items", "updated_artifact_path"],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_schema_requires_findings() {
        let schema = review_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], json!(["findings"]));
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn repair_schema_requires_updated_artifact_path() {
        let schema = repair_schema();
        assert!(schema["required"].as_array().unwrap().contains(&json!("updated_artifact_path")));
        assert_eq!(schema["properties"]["changes_made"]["type"], "array");
    }
}
```

Create `src/codex_exec.rs` with command test:

```rust
use crate::config::LoopConfig;
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
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
        "model_reasoning_effort=\"low\"".to_string(),
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

pub fn run_codex_json(prompt: &str, schema: &Value, run_dir: &Path, config: &LoopConfig) -> Result<Value> {
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
            sandbox: "workspace-write".to_string(),
            approval_policy: "never".to_string(),
        }
    }

    #[test]
    fn codex_args_include_schema_and_output_file() {
        let args = codex_args(&PathBuf::from("schema.json"), &PathBuf::from("answer.json"), &test_config());

        assert_eq!(args[0], "-a");
        assert_eq!(args[1], "never");
        assert_eq!(args[2], "exec");
        assert!(args.contains(&"--sandbox".to_string()));
        assert!(args.contains(&"workspace-write".to_string()));
        assert!(args.contains(&"model_reasoning_effort=\"low\"".to_string()));
        assert!(args.contains(&"--output-schema".to_string()));
        assert!(args.contains(&"--output-last-message".to_string()));
        assert_eq!(args.last().unwrap(), "-");
    }
}
```

- [ ] **Step 2: Wire modules**

Modify `src/lib.rs`:

```rust
pub mod codex_exec;
pub mod config;
pub mod schema;
```

- [ ] **Step 3: Run tests**

Run: `cargo test schema codex_exec --quiet`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/schema.rs src/codex_exec.rs src/lib.rs
git commit -m "feat: add codex exec schema adapter"
```

### Task 3: Verification, Workspace, And Records

**Files:**
- Create: `src/verify.rs`
- Create: `src/record.rs`
- Create: `src/workspace.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `run_verify(command: &str, cwd: &Path) -> anyhow::Result<VerifyResult>`
- Produces: `write_record(record: &IterationRecord, path: &Path) -> anyhow::Result<()>`
- Produces: `prepare_iteration_workspace(source_root: &Path, artifact: &str, iteration_dir: &Path) -> anyhow::Result<PathBuf>`

- [ ] **Step 1: Add verification module**

Create `src/verify.rs`:

```rust
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
        Command::new("cmd").args(["/C", command]).current_dir(cwd).output()?
    } else {
        Command::new("sh").args(["-c", command]).current_dir(cwd).output()?
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
```

- [ ] **Step 2: Add record module**

Create `src/record.rs`:

```rust
use crate::verify::VerifyResult;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IterationRecord {
    pub iteration: usize,
    pub review: Value,
    pub repair: Value,
    pub validation: VerifyResult,
    pub remaining_delta: Vec<String>,
}

pub fn write_record(record: &IterationRecord, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(record)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn write_record_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("runs/iteration_1/record.json");
        let record = IterationRecord {
            iteration: 1,
            review: json!({"findings": []}),
            repair: json!({"changes_made": []}),
            validation: VerifyResult {
                passed: true,
                returncode: 0,
                stdout: String::new(),
                stderr: String::new(),
            },
            remaining_delta: vec![],
        };

        write_record(&record, &path).unwrap();
        assert!(path.exists());
    }
}
```

- [ ] **Step 3: Add workspace module**

Create `src/workspace.rs`:

```rust
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn prepare_iteration_workspace(source_root: &Path, artifact: &str, iteration_dir: &Path) -> Result<PathBuf> {
    let source = source_root.join(artifact);
    let workspace = iteration_dir.join("workspace");
    let target = workspace.join(artifact);

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(&source, &target)
        .with_context(|| format!("failed to copy {} to {}", source.display(), target.display()))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn copies_artifact_into_iteration_workspace() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();

        let copied = prepare_iteration_workspace(dir.path(), "README.md", &dir.path().join("runs/it1")).unwrap();

        assert_eq!(fs::read_to_string(copied).unwrap(), "hello");
    }
}
```

- [ ] **Step 4: Wire modules**

Modify `src/lib.rs`:

```rust
pub mod codex_exec;
pub mod config;
pub mod record;
pub mod schema;
pub mod verify;
pub mod workspace;
```

- [ ] **Step 5: Run tests**

Run: `cargo test verify record workspace --quiet`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/verify.rs src/record.rs src/workspace.rs src/lib.rs
git commit -m "feat: add verification records and workspace"
```

### Task 4: Runner Stop Conditions And One-Iteration Dry Run

**Files:**
- Create: `src/runner.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `LoopConfig`
- Consumes: `run_verify`
- Consumes: `prepare_iteration_workspace`
- Produces: `should_stop(iteration: usize, max_iterations: usize, validation_passed: bool, remaining_delta: &[String]) -> bool`
- Produces: `run_loop(config: &LoopConfig, source_root: &Path, runs_dir: &Path) -> anyhow::Result<LoopSummary>`

- [ ] **Step 1: Add runner with stop condition tests**

Create `src/runner.rs`:

```rust
use crate::{
    config::LoopConfig,
    record::{write_record, IterationRecord},
    verify::run_verify,
    workspace::prepare_iteration_workspace,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopSummary {
    pub passed: bool,
    pub iterations: usize,
    pub final_record_path: Option<PathBuf>,
}

pub fn should_stop(
    iteration: usize,
    max_iterations: usize,
    validation_passed: bool,
    remaining_delta: &[String],
) -> bool {
    validation_passed || iteration >= max_iterations || remaining_delta.is_empty()
}

fn run_id() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("run-{seconds}")
}

pub fn run_loop(config: &LoopConfig, source_root: &Path, runs_dir: &Path) -> Result<LoopSummary> {
    let root = runs_dir.join(run_id());
    let iteration_dir = root.join("iteration_1");
    let copied = prepare_iteration_workspace(source_root, &config.artifact, &iteration_dir)?;
    let validation = run_verify(&config.verify, source_root)?;
    let remaining_delta = if validation.passed {
        vec![]
    } else {
        vec![if validation.stderr.trim().is_empty() {
            validation.stdout.clone()
        } else {
            validation.stderr.clone()
        }]
    };

    let record = IterationRecord {
        iteration: 1,
        review: json!({"findings": []}),
        repair: json!({
            "artifact": config.artifact,
            "iteration": 1,
            "changes_made": [],
            "unresolved_items": ["codex repair is enabled in the next task"],
            "updated_artifact_path": copied
        }),
        validation,
        remaining_delta,
    };

    let record_path = iteration_dir.join("record.json");
    write_record(&record, &record_path)?;
    Ok(LoopSummary {
        passed: record.validation.passed,
        iterations: 1,
        final_record_path: Some(record_path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn stops_when_validation_passes() {
        assert!(should_stop(1, 3, true, &["x".to_string()]));
    }

    #[test]
    fn stops_at_max_iterations() {
        assert!(should_stop(3, 3, false, &["x".to_string()]));
    }

    #[test]
    fn continues_when_delta_remains_before_limit() {
        assert!(!should_stop(1, 3, false, &["x".to_string()]));
    }

    #[test]
    fn dry_run_writes_record() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();
        let config = LoopConfig {
            artifact: "README.md".to_string(),
            goal: "repair docs".to_string(),
            verify: "printf ok".to_string(),
            max_iterations: 3,
            model: "gpt-5.5".to_string(),
            sandbox: "workspace-write".to_string(),
            approval_policy: "never".to_string(),
        };

        let summary = run_loop(&config, dir.path(), &dir.path().join("runs")).unwrap();

        assert!(summary.passed);
        assert!(summary.final_record_path.unwrap().exists());
    }
}
```

- [ ] **Step 2: Wire module**

Modify `src/lib.rs`:

```rust
pub mod codex_exec;
pub mod config;
pub mod record;
pub mod runner;
pub mod schema;
pub mod verify;
pub mod workspace;
```

- [ ] **Step 3: Run tests**

Run: `cargo test runner --quiet`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/runner.rs src/lib.rs
git commit -m "feat: add loop runner dry run"
```

### Task 5: CLI Doctor And Run Commands

**Files:**
- Create: `src/main.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: `LoopConfig::from_path`
- Consumes: `run_loop`
- Produces: `loopsmith doctor`
- Produces: `loopsmith run --config examples/plaintext-loop.json`

- [ ] **Step 1: Add CLI entrypoint**

Create `src/main.rs`:

```rust
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use loopsmith::{config::LoopConfig, runner::run_loop};
use std::{
    path::PathBuf,
    process::Command,
};

#[derive(Debug, Parser)]
#[command(name = "loopsmith")]
#[command(about = "Run auditable iterative repair loops with Codex CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Doctor,
    Run {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value = "runs")]
        runs_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Doctor => doctor(),
        Commands::Run { config, runs_dir } => {
            let config = LoopConfig::from_path(&config)?;
            let summary = run_loop(&config, &std::env::current_dir()?, &runs_dir)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            if summary.passed {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
    }
}

fn doctor() -> Result<()> {
    let output = Command::new("codex")
        .arg("--version")
        .output()
        .context("failed to run codex --version")?;

    if !output.status.success() {
        anyhow::bail!("codex --version failed");
    }

    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
```

- [ ] **Step 2: Run CLI smoke checks**

Run: `cargo run -- doctor`

Expected: prints current `codex-cli` version and exits 0.

Run: `cargo run -- run --config examples/plaintext-loop.json`

Expected: creates `runs/<run-id>/iteration_1/record.json`. It may exit 1 until implementation tests exist, because `cargo test --quiet` is the configured verify command.

- [ ] **Step 3: Update README**

Add:

````markdown
## Rust CLI Direction

The implementation target is a Rust single-binary CLI.

```bash
cargo run -- doctor
cargo run -- run --config examples/plaintext-loop.json
```

Initial P0 only wrote candidate artifacts under `runs/<run-id>/`. The current minimal workflow adds explicit `inspect`, `diff`, and `apply` commands; see `2026-06-30-loopsmith-vibecoding-workflow-plan.md`.
````

- [ ] **Step 4: Run full checks**

Run: `cargo test --quiet`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs README.md
git commit -m "feat: add rust cli entrypoint"
```

### Task 6: Enable Real Review And Repair Iterations

**Files:**
- Modify: `src/runner.rs`
- Modify: `src/codex_exec.rs`
- Modify: `src/schema.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: `run_codex_json`
- Consumes: `review_schema`
- Consumes: `repair_schema`
- Produces: one full `review -> repair -> verify -> record` loop

- [ ] **Step 1: Add prompt builders inside `src/runner.rs`**

Add these functions:

```rust
fn review_prompt(config: &LoopConfig, artifact_text: &str) -> String {
    format!(
        "You are reviewing an artifact before repair.\nArtifact: {}\nGoal: {}\nDo not edit files. Return concise structured findings.\n\n{}",
        config.artifact, config.goal, artifact_text
    )
}

fn repair_prompt(config: &LoopConfig, editable_path: &Path, findings: &serde_json::Value, remaining_delta: &[String], iteration: usize) -> String {
    format!(
        "You are repairing a copied artifact.\nEditable copy: {}\nIteration: {}\nGoal: {}\nMake the smallest useful edits. Edit only the editable copy. Do not claim validation passed.\nReview findings: {}\nRemaining validation delta: {}",
        editable_path.display(),
        iteration,
        config.goal,
        findings,
        serde_json::to_string_pretty(remaining_delta).unwrap()
    )
}
```

- [ ] **Step 2: Replace dry-run review/repair placeholders**

In `run_loop`, for each iteration:

1. Read the copied artifact text.
2. Call `run_codex_json(review_prompt, review_schema(), iteration_dir/review, config)`.
3. Call `run_codex_json(repair_prompt, repair_schema(), iteration_dir/repair, config)`.
4. Run `verify`.
5. Write `record.json`.
6. Stop when `should_stop(...)` returns true.

- [ ] **Step 3: Keep tests deterministic**

Do not unit-test by calling the real Codex CLI. Extract the phase execution behind a small trait only if tests need to simulate `run_codex_json`; otherwise keep real Codex coverage as a manual smoke check.

- [ ] **Step 4: Manual smoke check**

Run:

```bash
cargo run -- run --config examples/plaintext-loop.json --runs-dir runs
```

Expected:

- `runs/<run-id>/iteration_1/review/answer.json` exists.
- `runs/<run-id>/iteration_1/repair/answer.json` exists.
- `runs/<run-id>/iteration_1/record.json` exists.
- Source `README.md` is not overwritten.

- [ ] **Step 5: Commit**

```bash
git add src/runner.rs src/codex_exec.rs src/schema.rs README.md
git commit -m "feat: run codex review repair loop"
```

## Self-Review

- Spec coverage: Rust plan covers config, schema output, Codex invocation, verification, run workspace, records, stop conditions, CLI, and real review/repair integration.
- Placeholder scan: No placeholder markers or undefined implementation steps remain.
- Type consistency: `LoopConfig`, `VerifyResult`, `IterationRecord`, `LoopSummary`, `run_codex_json`, and `run_loop` are used consistently across tasks.
- Scope check: This remains one bounded MVP. Patch application, UI, daemon mode, plugin packaging, and CI integration are intentionally deferred.

## Execution Options

Plan complete and saved to `docs/superpowers/plans/2026-06-30-loopsmith-implementation-plan.md`. Two execution options:

1. Subagent-Driven (recommended): dispatch a fresh subagent per task, review between tasks, fast iteration.
2. Inline Execution: execute tasks in this session using executing-plans, batch execution with checkpoints.

Recommended first implementation command:

```bash
cargo test --quiet
```
