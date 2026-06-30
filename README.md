# codex-loop

`codex-loop` 是一个面向 Codex CLI 的本地迭代修复工具规划仓库，目标是把 `codex exec` 包装成可审计、可验证、可复用的修复闭环。

当前实现方向是 **Rust 单二进制 CLI**。本仓库目前只包含项目说明和实施计划，运行时代码尚未开始实现。

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

## 计划命令

```bash
codex-loop doctor
codex-loop run --config examples/plaintext-loop.json
```

开发阶段可使用：

```bash
cargo run -- doctor
cargo run -- run --config examples/plaintext-loop.json
```

## 配置示例

计划中的最小配置文件如下：

```json
{
  "artifact": "README.md",
  "goal": "Make the README clearer and remove stale setup guidance.",
  "verify": "cargo test --quiet",
  "max_iterations": 3,
  "model": "gpt-5.4-mini",
  "sandbox": "workspace-write",
  "approval_policy": "never"
}
```

字段说明：

- `artifact`：需要修复的目标文件路径。
- `goal`：本轮修复目标。
- `verify`：机械验证命令。
- `max_iterations`：最大迭代次数。
- `model`：传给 `codex exec --model` 的模型。
- `sandbox`：传给 `codex exec --sandbox` 的权限边界。
- `approval_policy`：传给 `codex exec --ask-for-approval` 的审批策略。

## 产物目录

每次运行计划写入：

```text
runs/<run-id>/
  config.json
  summary.json
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

`runs/` 已被 `.gitignore` 忽略，不应提交到仓库。

## 实施计划

详细 Rust 实施计划见：

- [docs/superpowers/plans/2026-06-30-codex-loop-implementation-plan.md](docs/superpowers/plans/2026-06-30-codex-loop-implementation-plan.md)

计划中的 P0 模块：

- `config`：读取和校验配置。
- `schema`：生成 review/repair JSON schema。
- `codex_exec`：封装 `codex exec` 调用。
- `verify`：执行机械验证命令。
- `record`：写入每轮审计记录。
- `workspace`：创建候选工作区并复制目标文件。
- `runner`：编排迭代和停止条件。
- `main`：CLI 入口。

## 当前状态

- 私有仓库已初始化。
- Rust 技术方案已确定。
- README 和实施计划已完成。
- 运行时代码尚未实现。

下一步建议从实施计划的 `Task 1: Rust Project Skeleton And Config` 开始。
