# Loopsmith Vibecoding Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Loopsmith from a runnable repair loop into a minimal vibecoding workflow that can inspect, review, diff, safely apply, and summarize candidate fixes.

**Architecture:** Keep the existing `run` loop and add a small workflow state layer around it. Each run writes a `manifest.json`, updates `runs/index.json`, writes a human `summary.md`, and exposes read/apply operations through focused CLI subcommands.

**Tech Stack:** Rust 2024, `clap`, `serde`, `serde_json`, existing fake executor tests, no new dependencies.

## Global Constraints

- Preserve source workspace safety: `run` edits only candidate workspaces under `runs/<run-id>/`.
- `apply` must be explicit and must reject source files that changed since the run started unless `--force` is supplied.
- Tests must not call real `codex exec`; use fake run directories and existing fake executors.
- Keep diff implementation dependency-free and deterministic.
- Keep release and CI commands green: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`.

---

## File Structure

- Create `src/run_state.rs`
  - Owns `RunManifest`, `RunStatus`, `RunIndex`, inspection, summary, diff, apply, and stable artifact hashing.
  - Exposes library functions used by `runner` and `main`.
- Modify `src/runner.rs`
  - Create manifest before the first iteration.
  - Update manifest, index, and summary after each iteration and at final status.
  - Extend `LoopSummary` with `run_id`, `run_dir`, `final_artifact_path`, and `summary_path`.
- Modify `src/main.rs`
  - Add `inspect`, `diff`, and `apply` subcommands.
  - Keep `run` behavior compatible while printing the richer JSON summary.
- Modify `src/lib.rs`
  - Export `run_state`.
- Modify `tests/cli.rs`
  - Ensure help lists `inspect`, `diff`, and `apply`.
- Create `tests/workflow.rs`
  - End-to-end library tests for manifest/index, inspect, diff, dry-run apply, real apply, hash mismatch blocking, and verify-after-apply.
- Modify `README.md` and `docs/loopsmith-best-practices.md`
  - Document the minimal workflow commands and safety contract.

---

### Task 1: Run State Model

**Files:**
- Create: `src/run_state.rs`
- Modify: `src/lib.rs`
- Test: `tests/workflow.rs`

**Interfaces:**
- Produces:
  - `RunStatus`
  - `RunManifest`
  - `RunInspection`
  - `write_manifest_and_index(manifest: &RunManifest, runs_dir: &Path) -> Result<()>`
  - `latest_run_id(runs_dir: &Path) -> Result<String>`
  - `inspect_run(runs_dir: &Path, run_id: Option<&str>) -> Result<RunInspection>`
  - `stable_hash_bytes(bytes: &[u8]) -> String`
  - `artifact_hash(path: &Path) -> Result<String>`

- [x] Write failing tests in `tests/workflow.rs` that create two fake run manifests and assert `latest_run_id()` returns the newest run and `inspect_run()` returns manifest plus records.
- [x] Run `cargo test workflow --quiet` and confirm unresolved `run_state` failures.
- [x] Implement `src/run_state.rs` with manifest/index read/write and inspect.
- [x] Export `pub mod run_state;` from `src/lib.rs`.
- [x] Run `cargo test workflow --quiet` and confirm Task 1 tests pass.

### Task 2: Runner Manifest And Summary

**Files:**
- Modify: `src/runner.rs`
- Modify: `src/run_state.rs`
- Test: `src/runner.rs`

**Interfaces:**
- Consumes: `write_manifest_and_index`, `write_run_summary`.
- Produces:
  - `LoopSummary { run_id, run_dir, passed, iterations, final_record_path, final_artifact_path, summary_path }`
  - `write_run_summary(runs_dir: &Path, manifest: &RunManifest) -> Result<PathBuf>`

- [x] Add failing runner test asserting a fake run writes `manifest.json`, `summary.md`, and `runs/index.json`.
- [x] Run `cargo test runner --quiet` and confirm failure.
- [x] Update `run_loop_with_executor()` to create/update manifest and summary.
- [x] Run `cargo test runner --quiet` and confirm pass.

### Task 3: Diff And Apply Library Functions

**Files:**
- Modify: `src/run_state.rs`
- Test: `tests/workflow.rs`

**Interfaces:**
- Produces:
  - `DiffResult { run_id, iteration, artifact, source_path, candidate_path, changed, diff }`
  - `ApplyOutcome { run_id, iteration, artifact, source_path, candidate_path, applied, dry_run, verification }`
  - `diff_run(source_root: &Path, runs_dir: &Path, run_id: Option<&str>, iteration: Option<usize>) -> Result<DiffResult>`
  - `apply_run(source_root: &Path, runs_dir: &Path, run_id: Option<&str>, iteration: Option<usize>, dry_run: bool, force: bool, verify_after: bool) -> Result<ApplyOutcome>`

- [x] Add failing tests for `diff_run()` showing original and candidate lines.
- [x] Add failing tests for `apply_run(..., dry_run=true)` proving source is unchanged.
- [x] Add failing tests for real apply proving candidate content replaces source.
- [x] Add failing test proving source hash mismatch blocks apply.
- [x] Add failing test proving `force=true` allows apply after source hash mismatch.
- [x] Add failing test proving `verify_after=true` runs the manifest verify command.
- [x] Run `cargo test workflow --quiet` and confirm failures.
- [x] Implement `diff_run()` using a deterministic dependency-free text diff.
- [x] Implement `apply_run()` with hash guard, dry-run, force, and optional verify.
- [x] Run `cargo test workflow --quiet` and confirm pass.

### Task 4: CLI Commands

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/cli.rs`

**Interfaces:**
- Consumes: `inspect_run`, `diff_run`, `apply_run`.
- Produces CLI:
  - `loopsmith inspect [RUN_ID] --runs-dir runs --json`
  - `loopsmith diff [RUN_ID] --iteration N --runs-dir runs`
  - `loopsmith apply [RUN_ID] --iteration N --runs-dir runs --dry-run --force --verify`

- [x] Add failing CLI help test requiring `inspect`, `diff`, and `apply`.
- [x] Add failing CLI test for missing run on `inspect`.
- [x] Run `cargo test cli --quiet` and confirm failure.
- [x] Implement subcommands and human output in `src/main.rs`.
- [x] Run `cargo test cli --quiet` and confirm pass.

### Task 5: Documentation And Full Validation

**Files:**
- Modify: `README.md`
- Modify: `docs/loopsmith-best-practices.md`

**Interfaces:**
- Produces user-facing workflow:
  - Run loop.
  - Inspect latest run.
  - Diff candidate.
  - Dry-run apply.
  - Apply with verify.

- [x] Update README with minimal vibecoding workflow commands.
- [x] Update best practices with apply safety contract.
- [x] Run `cargo fmt --check`.
- [x] Run `cargo clippy --locked --all-targets -- -D warnings`.
- [x] Run `cargo test --locked --all-targets`.
- [x] Run a local end-to-end fake-safe workflow using a hand-built run fixture: `inspect`, `diff`, `apply --dry-run`, `apply --verify`.
- [ ] Commit and push.
