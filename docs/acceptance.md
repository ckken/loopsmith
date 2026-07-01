# Loopsmith 全链路验收

本文定义 Loopsmith 最小 vibecoding 工作流的验收标准。自动测试不真实调用 `codex exec`，避免模型耗时、权限和网络状态影响 CI；真实模型 loop 保留为人工验收项。

## 本地质量门禁

每次发布前必须通过：

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

如果修改了发布脚本或版本号，还需要本地生成当前平台产物：

```bash
bash scripts/package-release.sh
```

## 最小工作流验收

验收链路必须覆盖：

1. `run` 能生成 `runs/<run-id>/manifest.json`、`summary.md`、`iteration_N/record.json` 和 `runs/index.json`。
2. `profile = "multi-review"` 能触发多个只读 review agent，并把 findings 合并给单 writer repair。
3. `hooks` 能在 `pre_run`、`post_iteration`、`pre_apply`、`post_apply`、`on_failure` 写入审计文件。
4. `inspect` 能读取最新 run 或指定 run，并展示状态、轮次、最终 record 和候选 artifact。
5. `diff` 能对比源文件和候选 workspace。
6. `apply --dry-run` 不写源文件。
7. `apply --verify` 显式写回源文件，并运行该 run 记录的验证命令。
8. 源文件在 run 开始后发生变化时，`apply` 默认拒绝覆盖；只有人工确认后才使用 `--force`。

## Fake-safe 端到端命令

下面命令不调用真实模型，只手工构造一个 run fixture，用来验证 CLI 的 inspect / diff / apply 链路：

```bash
repo=/Users/Bigo/Desktop/develop/ai/codex-loop
cargo build --manifest-path "$repo/Cargo.toml" --locked

tmp=$(mktemp -d)
mkdir -p "$tmp/source/runs/run-e2e/iteration_1/workspace"
printf 'old\n' > "$tmp/source/README.md"
printf 'new\n' > "$tmp/source/runs/run-e2e/iteration_1/workspace/README.md"

cat > "$tmp/source/runs/run-e2e/manifest.json" <<'JSON'
{
  "run_id": "run-e2e",
  "artifact": "README.md",
  "goal": "fake safe e2e",
  "verify": "grep new README.md",
  "max_iterations": 1,
  "started_at_unix": 1782777600,
  "finished_at_unix": 1782777601,
  "status": "passed",
  "iterations": 1,
  "final_record_path": "run-e2e/iteration_1/record.json",
  "final_artifact_path": "run-e2e/iteration_1/workspace/README.md",
  "summary_path": "run-e2e/summary.md",
  "source_artifact_hash": "force-e2e",
  "hooks": {
    "pre_apply": "printf pre > pre_apply.txt",
    "post_apply": "printf post > post_apply.txt"
  }
}
JSON

cat > "$tmp/source/runs/run-e2e/iteration_1/record.json" <<'JSON'
{
  "iteration": 1,
  "review": { "findings": [] },
  "repair": { "changes_made": [] },
  "validation": { "passed": true, "returncode": 0, "stdout": "", "stderr": "" },
  "remaining_delta": []
}
JSON

cat > "$tmp/source/runs/index.json" <<'JSON'
{ "runs": [{ "run_id": "run-e2e", "artifact": "README.md", "status": "passed", "started_at_unix": 1782777600, "finished_at_unix": 1782777601, "iterations": 1, "summary_path": "run-e2e/summary.md" }] }
JSON

(
  cd "$tmp/source"
  "$repo/target/debug/loopsmith" inspect run-e2e --runs-dir runs
  "$repo/target/debug/loopsmith" diff run-e2e --runs-dir runs
  "$repo/target/debug/loopsmith" apply run-e2e --runs-dir runs --dry-run --force
  "$repo/target/debug/loopsmith" apply run-e2e --runs-dir runs --force --verify
  grep new README.md
  grep pre pre_apply.txt
  grep post post_apply.txt
  test -f runs/run-e2e/hooks/apply/pre_apply/result.json
  test -f runs/run-e2e/hooks/apply/post_apply/result.json
)
```

## 发布验收

发布新 tag 后必须确认：

```bash
gh run list -R ckken/loopsmith --workflow CI --limit 1
gh run list -R ckken/loopsmith --workflow Release --limit 1
gh release view v0.3.1 -R ckken/loopsmith
```

二进制安装验证：

```bash
gh release download v0.3.1 -R ckken/loopsmith -p 'loopsmith-v0.3.1-aarch64-apple-darwin.tar.gz' -D /tmp/loopsmith-release
tar -xzf /tmp/loopsmith-release/loopsmith-v0.3.1-aarch64-apple-darwin.tar.gz -C /tmp/loopsmith-release
/tmp/loopsmith-release/loopsmith-v0.3.1-aarch64-apple-darwin/loopsmith doctor
```
