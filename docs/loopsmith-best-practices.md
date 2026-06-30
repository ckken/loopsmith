# Loopsmith 最佳实践

**Loopsmith：用可审计的 review、repair、verify 循环，自动编排 Codex 模型调用、候选 workspace、验证命令、迭代状态和审计记录，把 AI 生成的候选修复推进到可验证结果。**

本文面向 `loopsmith` 的真实落地使用，目标是让 Codex 自动化修复编排流程可控、可审计、可复现，而不是把模型输出直接当作最终结果。

## 核心原则

`loopsmith` 应该被当作“候选修复生成器”，而不是自动合并器。它负责在隔离工作区里让 Codex 执行 review / repair，并通过机械验证命令判断候选结果是否可进入人工验收。

推荐坚持以下原则：

- 先小范围运行，再扩大到多文件或跨模块任务。
- 先让验证命令定义成功，再让模型尝试修复。
- 所有模型输出都写入审计记录，不以口头结论替代 `record.json`。
- 默认只修改 `runs/<run-id>/` 下的候选工作区，不直接覆盖源文件。
- 自动测试只使用 fake executor，不在 CI 或单元测试中真实调用 `codex exec`。

## 适合的任务

优先用于边界清晰、可以机械验证的任务：

- README、配置示例、迁移说明、接口文档修复。
- 单文件或少量文件的 lint / format / test 修复。
- 有明确失败日志的测试修复。
- 规则明确的代码改造，例如移除废弃 API、补齐字段、修复 schema。

暂时不建议用于以下场景：

- 需要大量产品判断或设计判断的任务。
- 没有验证命令、只能靠主观判断的重构。
- 需要直接操作线上环境、密钥、数据库或外部服务的任务。
- 多个 agent 同时改同一批文件的任务。

## 推荐配置

基础配置建议从低风险模式开始：

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

字段使用建议：

- `artifact`：尽量指向单个高价值文件。第一版不要用目录作为目标。
- `goal`：写清楚“要改什么”和“不要改什么”，避免模型扩大范围。
- `verify`：必须是可重复执行的本地命令，且失败时能输出明确错误。
- `max_iterations`：默认 3 足够。超过 3 轮仍失败，通常应该人工介入。
- `sandbox`：默认使用 `workspace-write`，只有明确需要时才放宽。
- `approval_policy`：自动 loop 建议使用 `never`，避免执行中途等待人工确认。

## 模型选择

当前推荐按阶段拆模型，而不是所有阶段都使用同一个模型：

| 阶段 | 推荐模型 | 适用场景 |
| --- | --- | --- |
| 默认兜底 | `gpt-5.5` | 复杂任务、跨模块判断、高风险修复 |
| Review | `gpt-5.4` | 日常代码审查、文档审查、测试失败定位 |
| Repair | `gpt-5.4-mini` | 简单修复、格式调整、低风险文档更新 |
| 快速试跑 | `gpt-5.3-codex-spark` | 低风险 repair、快速生成候选补丁 |

实践建议：

- 高风险任务：`model = gpt-5.5`，`review_model = gpt-5.5`，`repair_model = gpt-5.4`。
- 常规任务：`review_model = gpt-5.4`，`repair_model = gpt-5.4-mini`。
- 快速试错：`repair_model = gpt-5.3-codex-spark`，但要保留严格验证命令。
- 不要把模型名写死成白名单。未来新增模型时，应允许配置原样透传给 Codex CLI。

`model_reasoning_effort` 建议从 `low` 开始。只有 review 质量不足、定位不稳定或任务本身复杂时，再提高到 `medium` / `high` / `xhigh`。

## 验证命令

验证命令是整个 loop 的最终裁判。没有可靠验证命令时，`loopsmith` 只能生成候选结果，不能判断是否完成。

好的验证命令应该满足：

- 本地可运行，不依赖临时人工输入。
- 失败输出具体，能被下一轮 repair 消费。
- 运行时间可控，避免每轮都触发长时间集成测试。
- 不修改源工作区，只在候选 workspace 内执行。

示例：

```bash
cargo test --quiet
npm test -- --runInBand
pnpm lint && pnpm test
pytest -q tests/test_config.py
```

不建议：

```bash
make deploy
pnpm test --watch
terraform apply
```

如果项目测试很慢，建议先为 loop 准备更窄的验证命令，例如只跑目标模块、目标包或目标测试文件。

## 运行流程

推荐流程：

1. 确认当前工作区状态干净，或至少知道哪些改动是人工保留的。
2. 编写小范围配置文件。
3. 执行 `loopsmith doctor` 检查 Codex CLI 可用性。
4. 执行 `loopsmith run --config <config>`。
5. 查看最后一轮 `record.json`、`review/answer.json`、`repair/answer.json` 和验证输出。
6. 人工检查 `runs/<run-id>/iteration_N/workspace/` 里的候选文件。
7. 确认后再手动把候选改动应用回源工作区。

当前版本不应该自动 apply。原因是 P0 阶段更重要的是把“模型做了什么、验证为什么通过或失败”记录清楚。

## 审计记录

每轮至少要保留以下信息：

- `review/prompt.txt`：review 阶段输入。
- `review/schema.json`：review 输出约束。
- `review/answer.json`：结构化 review 结果。
- `repair/prompt.txt`：repair 阶段输入。
- `repair/answer.json`：结构化 repair 结果。
- `stdout.txt` / `stderr.txt`：Codex CLI 原始输出。
- `record.json`：本轮汇总，包括 `iteration`、`validation`、`remaining_delta`。

重点看 `record.json`：

- `validation.passed = true` 只代表机械验证通过，不代表代码一定可合并。
- `remaining_delta` 是下一轮 repair 的关键输入。
- 如果连续多轮 `remaining_delta` 没有明显变化，应停止自动迭代，改为人工判断。

## 安全边界

默认安全策略：

- `sandbox = workspace-write`
- `approval_policy = never`
- 只复制候选 workspace。
- 不读取或写入 `.git`、`target`、`runs*` 等目录。
- 不在自动测试里真实调用 Codex 模型。

对于涉及密钥、生产配置、外部服务、数据库迁移的任务，应该把 loop 降级为“建议生成器”，由人工执行最终操作。

## Subagent 策略

当前阶段不建议让多个 subagent 同时修改文件。更稳的路线是先做阶段角色化：

- review 阶段可以未来扩展为多个只读 agent，例如 correctness、tests、docs。
- repair 阶段保持单 writer，避免多个 agent 写同一份候选 workspace。
- 每个 agent 都必须写入独立审计记录，包括模型、prompt、answer、耗时和失败原因。

推荐演进顺序：

1. 阶段模型配置。
2. timeout / retry / reasoning effort 配置。
3. record 中记录实际模型和参数。
4. 多 review agent，只读并合并 findings。
5. 单 repair agent 消费合并后的 findings。

## 测试策略

自动测试必须保持稳定、快速、可重复：

- 单元测试覆盖配置校验、workspace 复制边界、record round-trip、runner 状态机。
- CLI 集成测试只覆盖 help、配置读取失败等不触发真实模型的路径。
- runner 测试使用 fake executor。
- 不在 `cargo test` 中调用真实 `codex exec`。

真实模型 loop 属于手动验收，适合在本机执行并检查 `runs/` 产物。

## 常见失败处理

验证失败但有明确日志：

- 保留 `remaining_delta`。
- 允许进入下一轮。
- 如果 2 到 3 轮后仍失败，人工检查候选文件和验证日志。

验证失败但没有 stdout / stderr：

- 优先改验证命令，让它输出可消费的错误。
- 不要盲目增加最大迭代次数。

模型修改范围过大：

- 收窄 `artifact`。
- 在 `goal` 中写明禁止改动范围。
- 降低 `repair_model` 能力或提高 review 约束。

运行时间过长：

- 降低 `max_iterations`。
- 使用更窄的验证命令。
- repair 阶段使用 `gpt-5.4-mini` 或 `gpt-5.3-codex-spark`。

## 推荐落地路线

P0 已具备：

- Rust 单二进制 CLI。
- 配置读取和校验。
- review / repair / verify / record 闭环。
- 候选 workspace 隔离。
- fake executor 测试覆盖。

下一步建议：

1. 增加显式 `apply` 命令，但默认仍需人工确认。
2. 在 `record.json` 中记录实际使用模型、reasoning effort 和耗时。
3. 增加 per-phase timeout / retry。
4. 支持多 review agent，但 repair 保持单 writer。
5. 增加更完整的 diff 展示和人工验收报告。

判断是否可以进入真实项目试点的标准：

- 有稳定验证命令。
- 目标文件范围清晰。
- 候选 workspace 不污染源仓库。
- 失败时能通过 `record.json` 复盘。
- 人工能在 5 分钟内判断候选结果是否可采纳。
