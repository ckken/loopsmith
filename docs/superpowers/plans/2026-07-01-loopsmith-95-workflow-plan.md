# Loopsmith 95 Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise Loopsmith from a solid MVP to a 95+ local AI repair workflow by adding configurable workflow profiles, audited hooks, and multi-review agent orchestration while preserving the single-writer safety model.

**Architecture:** Keep the existing `run -> inspect -> diff -> apply` flow. Add configuration-level workflow policy (`profile`, `review_agents`, `hooks`), execute review agents as isolated read-only review phases, merge their findings into one repair input, and run audited hook commands at stable lifecycle events.

**Tech Stack:** Rust 2024, `clap`, `serde`, `serde_json`, existing fake executor tests, no new dependencies.

## Global Constraints

- Do not call real `codex exec` in automated tests.
- Preserve source safety: `run` edits only candidate workspaces; `apply` remains explicit.
- Multi-review agents must be read-only; repair remains single writer.
- Hook commands must write auditable command/stdout/stderr/result files.
- Keep CI and release green: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`.

---

## 95+ Acceptance Criteria

- Product: built-in workflow profiles make the tool usable without hand-designing every config.
- Safety: lifecycle hooks can gate pre-run and pre-apply operations and record evidence.
- Orchestration: multiple review agents can be triggered by config/profile and merged into one repair input.
- Auditability: hooks and review agents leave deterministic files under `runs/<run-id>/`.
- Usability: README and best practices explain when to use goal, hooks, profiles, and subagents.
- Verification: unit, integration, fake-safe E2E, CI, and release package checks all pass.

## File Structure

- Create `src/hooks.rs`
  - Owns `HooksConfig`, hook command execution, and hook audit files.
- Modify `src/config.rs`
  - Add `profile`, `review_agents`, `hooks`, validation, profile defaults, and per-agent review phase config.
- Modify `src/runner.rs`
  - Execute `pre_run`, `post_iteration`, `on_failure`.
  - Execute multi-review agents under `iteration_N/review/<agent-id>/`.
  - Merge agent findings before repair.
- Modify `src/run_state.rs`
  - Store hooks in `RunManifest`.
  - Execute `pre_apply` and `post_apply`.
- Modify `src/main.rs`
  - Add `profiles` command listing built-in profiles and their agent behavior.
- Modify `tests/cli.rs`, `tests/workflow.rs`, and runner/config unit tests.
- Modify `README.md`, `docs/loopsmith-best-practices.md`, `docs/acceptance.md`.

---

### Task 1: Profile And Agent Config

**Files:**
- Modify: `src/config.rs`
- Test: `src/config.rs`

**Interfaces:**
- Produces:
  - `WorkflowProfile`
  - `ReviewAgentConfig`
  - `LoopConfig::effective_profile() -> WorkflowProfile`
  - `LoopConfig::effective_review_agents() -> Vec<ReviewAgentConfig>`
  - `LoopConfig::review_phase_config_for(agent: &ReviewAgentConfig) -> LoopConfig`

- [x] Add failing tests for loading `profile`, `review_agents`, and `hooks`.
- [x] Add failing tests for `multi-review` default agents.
- [x] Add failing tests rejecting unsupported profile and unsafe agent IDs.
- [x] Implement profile and agent config.
- [x] Run `cargo test config --quiet`.

### Task 2: Audited Hooks

**Files:**
- Create: `src/hooks.rs`
- Modify: `src/lib.rs`
- Test: `src/hooks.rs`

**Interfaces:**
- Produces:
  - `HooksConfig`
  - `HookAudit`
  - `run_hook(event: &str, command: &str, cwd: &Path, output_dir: &Path) -> Result<VerifyResult>`
  - `run_required_hook(event: &str, command: &str, cwd: &Path, output_dir: &Path) -> Result<VerifyResult>`

- [x] Add failing tests proving hook execution writes `command.txt`, `stdout.txt`, `stderr.txt`, and `result.json`.
- [x] Add failing test proving required hook returns an error on non-zero exit.
- [x] Implement `src/hooks.rs`.
- [x] Export `pub mod hooks;`.
- [x] Run `cargo test hooks --quiet`.

### Task 3: Multi-Review Runner

**Files:**
- Modify: `src/runner.rs`
- Test: `src/runner.rs`

**Interfaces:**
- Consumes: `ReviewAgentConfig`, `LoopConfig::effective_review_agents`, `run_required_hook`, `run_hook`.
- Produces:
  - Review directories: `iteration_N/review/<agent-id>/`.
  - Merged review JSON with `agents` and `findings`.
  - Hook audit directories for `pre_run`, `post_iteration`, `on_failure`.

- [x] Add failing runner test proving `profile = "multi-review"` runs three review agents and merges their findings.
- [x] Add failing runner test proving `pre_run` writes hook audit before review.
- [x] Add failing runner test proving failed `post_iteration` hook keeps the run failed and records delta.
- [x] Implement multi-review execution and hook integration.
- [x] Run `cargo test runner --quiet`.

### Task 4: Apply Hooks And Profiles CLI

**Files:**
- Modify: `src/run_state.rs`
- Modify: `src/main.rs`
- Test: `tests/workflow.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: `RunManifest.hooks`.
- Produces:
  - Apply hook audit directories under `runs/<run-id>/hooks/apply/`.
  - CLI command: `loopsmith profiles`.

- [x] Add failing workflow test proving `pre_apply` and `post_apply` hooks run during apply.
- [x] Add failing CLI help test requiring `profiles`.
- [x] Implement apply hooks and `profiles`.
- [x] Run `cargo test workflow cli --quiet` or the equivalent focused tests.

### Task 5: Docs And Full Validation

**Files:**
- Modify: `README.md`
- Modify: `docs/loopsmith-best-practices.md`
- Modify: `docs/acceptance.md`
- Modify: `docs/superpowers/plans/2026-07-01-loopsmith-95-workflow-plan.md`

**Interfaces:**
- Produces:
  - Updated scorecard and usage docs for goal/profile/hooks/subagents.
  - Full validation evidence.

- [x] Update README with profile, hook, and multi-review examples.
- [x] Update best practices with the 95+ operating model.
- [x] Update acceptance fake-safe workflow to cover apply hooks.
- [x] Run `cargo fmt --check`.
- [x] Run `cargo clippy --locked --all-targets -- -D warnings`.
- [x] Run `cargo test --locked --all-targets`.
- [x] Run local fake-safe E2E.
- [ ] Commit, push, tag release if version changes.
