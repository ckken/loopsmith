<p align="center">
  <img src="assets/logo.png" width="220" alt="Loopsmith logo">
</p>

<h1 align="center">Loopsmith</h1>

<p align="center">
  <em>面向 Codex 的自动化修复编排工具。</em>
</p>

**Loopsmith：用可审计的 review、repair、verify 循环，自动编排 Codex 模型调用、候选 workspace、验证命令、迭代状态和审计记录，把 AI 生成的候选修复推进到可验证结果。**

`loopsmith` 是一个面向 Codex CLI 的本地自动化修复编排工具，目标是把 `codex exec` 包装成可审计、可验证、可复用的修复闭环。

当前实现方向是 **Rust 单二进制 CLI**。仓库已具备可试点的本地工作流闭环：`doctor` 能检测本机 Codex CLI，`run` 能复制当前项目到候选工作区、按 profile 触发一个或多个只读 review agent、调用单 writer repair、运行机械验证、执行 lifecycle hooks，并写入可审计记录。

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
gh release download v0.3.1 -R ckken/loopsmith -p 'loopsmith-v0.3.1-aarch64-apple-darwin.tar.gz'
tar -xzf loopsmith-v0.3.1-aarch64-apple-darwin.tar.gz
sudo install -m 0755 loopsmith-v0.3.1-aarch64-apple-darwin/loopsmith /usr/local/bin/loopsmith
loopsmith doctor
```

从源码安装：

```bash
cargo install --git https://github.com/ckken/loopsmith --tag v0.3.1
loopsmith doctor
```

更多平台安装方法和发布流程见 [docs/release.md](docs/release.md)。

## 基本使用

```bash
loopsmith doctor
loopsmith profiles
loopsmith run --config examples/plaintext-loop.json
```

## 最小 vibecoding 工作流

一次完整的最小工作流：

```bash
loopsmith run --config examples/plaintext-loop.json
loopsmith inspect
loopsmith diff
loopsmith apply --dry-run
loopsmith apply --verify
```

命令说明：

- `loopsmith run`：创建候选 workspace，执行 review / repair / verify 循环。
- `loopsmith profiles`：列出内置 workflow profile。
- `loopsmith inspect [RUN_ID]`：查看 run 状态、迭代记录、最终候选文件和 summary 路径；不传 `RUN_ID` 时读取最新 run。
- `loopsmith diff [RUN_ID] --iteration N`：对比源文件和指定轮次候选文件。
- `loopsmith apply [RUN_ID] --iteration N --dry-run`：只检查是否可以应用，不写源文件。
- `loopsmith apply [RUN_ID] --iteration N --verify`：把候选文件应用回源工作区，并运行该 run 的验证命令。
- `loopsmith apply --force`：源文件在 run 开始后发生变化时仍强制应用；默认会拒绝覆盖。

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
  "approval_policy": "never",
  "profile": "multi-review",
  "hooks": {
    "pre_run": "git diff --check",
    "post_iteration": "cargo test --quiet",
    "pre_apply": "cargo fmt --check",
    "post_apply": "cargo test --locked --all-targets"
  }
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
- `profile`：内置工作流策略；可选 `default`、`quick-fix`、`test-repair`、`docs-repair`、`multi-review`。
- `review_agents`：可选，显式配置多个只读 review agent；配置后覆盖 profile 的默认 agent。
- `hooks`：可选，配置 Loopsmith lifecycle hooks，包括 `pre_run`、`post_iteration`、`pre_apply`、`post_apply`、`on_failure`。

内置 profile：

```bash
loopsmith profiles
```

多 review agent 示例见 [examples/multi-review-loop.json](examples/multi-review-loop.json)。`multi-review` 会触发 `correctness`、`tests`、`docs` 三个只读 reviewer，并把 findings 合并后交给单 writer repair。

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
  manifest.json
  summary.md
  iteration_1/
    workspace/
    review/
      correctness/
        prompt.txt
        schema.json
        answer.json
        stdout.txt
        stderr.txt
      tests/
      docs/
    repair/
      prompt.txt
      schema.json
      answer.json
      stdout.txt
      stderr.txt
    record.json
    hooks/
      post_iteration/
        command.txt
        stdout.txt
        stderr.txt
        result.json
  hooks/
    pre_run/
    on_failure/
    apply/
      pre_apply/
      post_apply/
runs/index.json
```

运行摘要会输出到终端；每轮的审计材料写入对应 `iteration_N/` 目录。`manifest.json` 记录 run 级状态，`summary.md` 是人工验收摘要，`runs/index.json` 用于定位最新 run。

`runs/` 已被 `.gitignore` 忽略，不应提交到仓库。

## 实施计划

详细 Rust 实施计划见：

- [docs/superpowers/plans/2026-06-30-loopsmith-implementation-plan.md](docs/superpowers/plans/2026-06-30-loopsmith-implementation-plan.md)
- [docs/loopsmith-best-practices.md](docs/loopsmith-best-practices.md)
- [docs/acceptance.md](docs/acceptance.md)
- [docs/scorecard.md](docs/scorecard.md)
- [docs/release.md](docs/release.md)

当前核心模块：

- `config`：读取和校验配置。
- `schema`：生成 review/repair JSON schema。
- `codex_exec`：封装 `codex exec` 调用。
- `verify`：执行机械验证命令。
- `record`：写入每轮审计记录。
- `workspace`：创建候选工作区并复制目标文件。
- `runner`：编排迭代和停止条件。
- `hooks`：执行 Loopsmith lifecycle hooks 并写入审计文件。
- `run_state`：维护 run manifest、index、summary、inspect、diff、apply 和 apply hooks。
- `main`：提供 CLI 入口。

## 当前状态

- Rust CLI 骨架已实现。
- 已用当前项目真实跑通一轮 `codex exec` review/repair loop。
- 已具备可试点 vibecoding 工作流：`run` 生成候选修复，`inspect` 查看状态，`diff` 对比候选，`apply --dry-run` 做写回前检查，`apply --verify` 显式写回并重新验证。
- 已支持 workflow profile、多个只读 review agent、单 writer repair，以及 `pre_run` / `post_iteration` / `pre_apply` / `post_apply` / `on_failure` hooks。
- `apply` 默认校验源文件 hash，避免覆盖 run 开始后的人工修改。

下一步建议：增加 resume、多 artifact 支持、更完整的 unified diff 和可分享验收报告。
