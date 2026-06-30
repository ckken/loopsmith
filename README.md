<p align="center">
  <img src="assets/logo.png" width="220" alt="Loopsmith logo">
</p>

<h1 align="center">Loopsmith</h1>

<p align="center">
  <em>面向 Codex 的自动化修复编排工具。</em>
</p>

**Loopsmith：用可审计的 review、repair、verify 循环，自动编排 Codex 模型调用、候选 workspace、验证命令、迭代状态和审计记录，把 AI 生成的候选修复推进到可验证结果。**

`loopsmith` 是一个面向 Codex CLI 的本地自动化修复编排工具，目标是把 `codex exec` 包装成可审计、可验证、可复用的修复闭环。

当前实现方向是 **Rust 单二进制 CLI**。仓库已具备最小可运行闭环：`doctor` 能检测本机 Codex CLI，`run` 能复制当前项目到候选工作区、调用 `codex exec` 执行 review/repair、运行机械验证并写入 `record.json`。

## 目标

第一版 CLI 聚焦一个最小闭环：

1. 读取目标文件和修复目标。
2. 调用 `codex exec` 进行结构化 review。
3. 在候选工作区中执行 bounded repair。
4. 执行机械验证命令，例如 `cargo test`、`npm test`、`pytest`。
5. 为每一轮写入 `record.json`、prompt、schema、answer、stdout/stderr 和候选文件。
6. 在验证通过、达到最大轮次、无有效增量或需要人工判断时停止。

## 设计原则

- Rust 负责 CLI、文件系统、进程编排、状态机和审计记录。
- Codex 只负责 review/repair，不负责最终判定。
- 成功与否优先由机械验证命令决定，不依赖主观判断。
- 默认 sandbox 为 `workspace-write`。
- 默认只修改 `runs/<run-id>/` 下的候选工作区，不直接覆盖源文件。
- 第一版不做 UI、不做 daemon、不做数据库、不做远程调度。

## 安装

从 GitHub Release 安装预编译二进制：

```bash
gh auth login
gh release download v0.1.0 -R ckken/loopsmith -p 'loopsmith-v0.1.0-aarch64-apple-darwin.tar.gz'
tar -xzf loopsmith-v0.1.0-aarch64-apple-darwin.tar.gz
sudo install -m 0755 loopsmith-v0.1.0-aarch64-apple-darwin/loopsmith /usr/local/bin/loopsmith
loopsmith doctor
```

从源码安装：

```bash
cargo install --git https://github.com/ckken/loopsmith --tag v0.1.0
loopsmith doctor
```

更多平台安装方法和发布流程见 [docs/release.md](docs/release.md)。

## 基本使用

```bash
loopsmith doctor
loopsmith run --config examples/plaintext-loop.json
```

开发阶段也可以直接通过 Cargo 运行：

```bash
cargo run -- doctor
cargo run -- run --config examples/plaintext-loop.json
```

如需安装到本机 PATH，可选执行：

```bash
cargo install --path . --force
loopsmith doctor
loopsmith run --config examples/plaintext-loop.json
```

`run` 默认会把候选项目复制到 `runs/<run-id>/iteration_N/workspace/`，源文件不会被直接覆盖。

## 配置示例

当前示例配置：

```json
{
  "artifact": "README.md",
  "goal": "Make the README clearer and remove stale setup guidance.",
  "verify": "cargo test --quiet",
  "max_iterations": 3,
  "model": "gpt-5.5",
  "review_model": "gpt-5.4",
  "repair_model": "gpt-5.4-mini",
  "model_reasoning_effort": "low",
  "sandbox": "workspace-write",
  "approval_policy": "never"
}
```

字段说明：

- `artifact`：需要修复的目标文件路径。
- `goal`：本轮修复目标。
- `verify`：机械验证命令。
- `max_iterations`：最大迭代次数。
- `model`：默认传给 `codex exec --model` 的模型。
- `review_model`：可选，review 阶段模型；未配置时回退到 `model`。
- `repair_model`：可选，repair 阶段模型；未配置时回退到 `model`。
- `model_reasoning_effort`：传给 `codex exec --config model_reasoning_effort="..."` 的推理强度，当前支持 `low`、`medium`、`high`、`xhigh`。
- `sandbox`：传给 `codex exec --sandbox` 的权限边界。
- `approval_policy`：传给 Codex 顶层 `-a` 参数的审批策略。

当前建议模型：

- `gpt-5.5`：复杂修复、跨模块分析和高风险变更的默认稳妥选择。
- `gpt-5.4`：日常代码 review / repair，适合放在 `review_model`。
- `gpt-5.4-mini`：更快、更省的简单修复，适合低风险 `repair_model`。
- `gpt-5.3-codex-spark`：超快编码模型；如果当前账号/环境可用，可用于低风险 repair 或快速试跑。

模型名会原样透传给 Codex CLI。项目只校验空值，不限制必须来自上述清单，避免未来新增模型时需要立即改代码。

## 产物目录

每次运行写入：

```text
runs/<run-id>/
  iteration_1/
    workspace/
    review/
      prompt.txt
      schema.json
      answer.json
      stdout.txt
      stderr.txt
    repair/
      prompt.txt
      schema.json
      answer.json
      stdout.txt
      stderr.txt
    record.json
```

运行摘要会输出到终端；每轮的审计材料写入对应 `iteration_N/` 目录。

`runs/` 已被 `.gitignore` 忽略，不应提交到仓库。

## 实施计划

详细 Rust 实施计划见：

- [docs/superpowers/plans/2026-06-30-loopsmith-implementation-plan.md](docs/superpowers/plans/2026-06-30-loopsmith-implementation-plan.md)
- [docs/loopsmith-best-practices.md](docs/loopsmith-best-practices.md)
- [docs/release.md](docs/release.md)

当前核心模块：

- `config`：读取和校验配置。
- `schema`：生成 review/repair JSON schema。
- `codex_exec`：封装 `codex exec` 调用。
- `verify`：执行机械验证命令。
- `record`：写入每轮审计记录。
- `workspace`：创建候选工作区并复制目标文件。
- `runner`：编排迭代和停止条件。
- `main`：提供 CLI 入口。

## 当前状态

- Rust CLI 骨架已实现。
- 已用当前项目真实跑通一轮 `codex exec` review/repair loop。
- 当前版本仍是 P0：候选修复只留在 `runs/<run-id>/`，不会自动 apply 回源文件。

下一步建议：增加显式 `apply` 命令，把人工确认后的候选 patch 应用回源工作区。
