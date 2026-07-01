# Loopsmith Scorecard

本文用于界定 Loopsmith 当前阶段的评分口径。评分对象是“本地单 artifact AI 修复编排 CLI”，不是远程多租户平台、IDE UI 或通用 CI 系统。

## 当前评分

综合分：96 / 100

| 维度 | 分数 | 依据 |
| --- | ---: | --- |
| 产品定位 | 9.5 | 定位清晰：把 Codex 修复放进可审计、可验证、本地可控的 loop。 |
| MVP 闭环 | 9.5 | 已具备 `run -> inspect -> diff -> apply --dry-run -> apply --verify`。 |
| 工程质量 | 9.5 | Rust 单二进制、单元/集成测试、CI、Release、多平台产物完整。 |
| 安全边界 | 9.5 | 候选 workspace 隔离、显式 apply、source hash guard、pre/post apply hooks。 |
| 可验证性 | 9.5 | fake executor、fake-safe E2E、CI、release package verification 均覆盖。 |
| 编排能力 | 9.5 | 内置 workflow profile、可配置 hooks、多只读 review agent、单 writer repair。 |
| 可用性 | 9.0 | CLI 路径清楚，`profiles` 可发现；diff 展示仍可继续增强。 |
| 真实项目试点度 | 9.0 | 适合单文件/小范围修复试点；多 artifact 和 resume 仍是下一阶段重点。 |

## 95+ 标准

达到 95+ 需要同时满足：

- 有清晰目标：`goal` 描述任务边界和禁止范围。
- 有策略：`profile` 或 `review_agents` 定义 review 角色。
- 有门禁：`hooks` 把本地质量检查接入生命周期。
- 有隔离：`run` 只写候选 workspace。
- 有显式写回：`apply` 默认 hash guard，且可 dry-run。
- 有审计：review、repair、verify、hook、manifest、summary 均落盘。
- 有自动验收：本地测试、CI、release package verification 均可复现。

## 剩余扣分项

- 还没有 resume，失败或中断的长 run 不能从最近一轮继续。
- 还没有多 artifact 原生模型，跨文件修复需要拆成多个配置。
- diff 仍是依赖少、确定性强的基础文本 diff，不是完整 unified diff。
- hook 没有环境变量注入，例如 run id、iteration、artifact 等上下文变量。

这些是进入 98+ 的后续方向，不影响当前单 artifact 本地工作流达到 95+ 试点标准。
