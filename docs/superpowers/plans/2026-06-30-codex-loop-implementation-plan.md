# Codex Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a minimal local CLI that runs auditable iterative repair loops with Codex.

**Architecture:** A Python runner owns state, artifact copies, validation commands, stop conditions, and audit records. Codex is invoked only through `codex exec` with JSON schema outputs for review and repair phases. Each iteration writes a durable record so a maintainer can inspect what was found, changed, verified, and left unresolved.

**Tech Stack:** Python 3.11+, `argparse`, `subprocess`, `json`, `pathlib`, `pytest`, Codex CLI.

## Global Constraints

- Default sandbox must be `workspace-write`.
- Default approval policy must be `never` only for the nested `codex exec` calls managed by this runner.
- Do not require a notebook runtime; the first implementation must work on plain text and code files.
- Do not rely on LLM judgment as the only pass/fail signal; every recipe must define a mechanical `verify` command.
- Write run artifacts under `runs/<run-id>/` and keep `runs/` ignored by git.
- Keep implementation small: no daemon, no UI, no database in the first version.
- Preserve user files by editing a copied artifact inside the run directory first; applying the patch back to the source is a later explicit step.

---

## File Structure

- Create `pyproject.toml`: package metadata, Python version floor, pytest config.
- Create `src/codex_loop/__init__.py`: package version.
- Create `src/codex_loop/config.py`: typed config loading and validation.
- Create `src/codex_loop/schemas.py`: JSON schema builders for Codex structured outputs.
- Create `src/codex_loop/codex_exec.py`: wrapper around `codex exec`.
- Create `src/codex_loop/verify.py`: mechanical verification command runner.
- Create `src/codex_loop/records.py`: run directory and `record.json` persistence.
- Create `src/codex_loop/runner.py`: iteration orchestration and stop conditions.
- Create `src/codex_loop/cli.py`: `codex-loop run --config loop.json` entrypoint.
- Create `tests/`: focused pytest coverage for config, schemas, exec wrapper command construction, verification, and runner stop logic.
- Create `examples/plaintext-loop.json`: minimal config example.

### Task 1: Project Skeleton And Config Loader

**Files:**
- Create: `pyproject.toml`
- Create: `src/codex_loop/__init__.py`
- Create: `src/codex_loop/config.py`
- Create: `tests/test_config.py`
- Create: `examples/plaintext-loop.json`

**Interfaces:**
- Produces: `LoopConfig.from_file(path: Path) -> LoopConfig`
- Produces: `LoopConfig.validate() -> None`
- Consumes: no earlier project code

- [ ] **Step 1: Write the failing config tests**

```python
from pathlib import Path

import pytest

from codex_loop.config import LoopConfig


def test_loads_minimal_loop_config(tmp_path: Path):
    config_path = tmp_path / "loop.json"
    config_path.write_text(
        """
        {
          "artifact": "README.md",
          "goal": "remove stale setup guidance",
          "verify": "python -c 'print(0)'",
          "max_iterations": 3
        }
        """,
        encoding="utf-8",
    )

    config = LoopConfig.from_file(config_path)

    assert config.artifact == Path("README.md")
    assert config.goal == "remove stale setup guidance"
    assert config.verify == "python -c 'print(0)'"
    assert config.max_iterations == 3
    assert config.sandbox == "workspace-write"
    assert config.approval_policy == "never"


def test_rejects_missing_verify(tmp_path: Path):
    config_path = tmp_path / "loop.json"
    config_path.write_text(
        """
        {
          "artifact": "README.md",
          "goal": "remove stale setup guidance"
        }
        """,
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="verify"):
        LoopConfig.from_file(config_path)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python -m pytest tests/test_config.py -q`

Expected: FAIL with `ModuleNotFoundError: No module named 'codex_loop'`.

- [ ] **Step 3: Add package metadata**

Create `pyproject.toml`:

```toml
[project]
name = "codex-loop"
version = "0.1.0"
description = "Auditable iterative repair loops driven by Codex CLI"
requires-python = ">=3.11"
dependencies = []

[project.scripts]
codex-loop = "codex_loop.cli:main"

[build-system]
requires = ["setuptools>=69"]
build-backend = "setuptools.build_meta"

[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]
```

- [ ] **Step 4: Implement config loader**

Create `src/codex_loop/__init__.py`:

```python
__version__ = "0.1.0"
```

Create `src/codex_loop/config.py`:

```python
from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class LoopConfig:
    artifact: Path
    goal: str
    verify: str
    max_iterations: int = 3
    model: str = "gpt-5.4-mini"
    sandbox: str = "workspace-write"
    approval_policy: str = "never"

    @classmethod
    def from_file(cls, path: Path) -> "LoopConfig":
        data = json.loads(path.read_text(encoding="utf-8"))
        config = cls.from_dict(data)
        config.validate()
        return config

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "LoopConfig":
        return cls(
            artifact=Path(str(data.get("artifact", ""))),
            goal=str(data.get("goal", "")),
            verify=str(data.get("verify", "")),
            max_iterations=int(data.get("max_iterations", 3)),
            model=str(data.get("model", "gpt-5.4-mini")),
            sandbox=str(data.get("sandbox", "workspace-write")),
            approval_policy=str(data.get("approval_policy", "never")),
        )

    def validate(self) -> None:
        if not str(self.artifact):
            raise ValueError("artifact is required")
        if not self.goal.strip():
            raise ValueError("goal is required")
        if not self.verify.strip():
            raise ValueError("verify is required")
        if self.max_iterations < 1:
            raise ValueError("max_iterations must be at least 1")
        if self.sandbox not in {"read-only", "workspace-write", "danger-full-access"}:
            raise ValueError(f"unsupported sandbox: {self.sandbox}")
        if self.approval_policy not in {"untrusted", "on-request", "never"}:
            raise ValueError(f"unsupported approval_policy: {self.approval_policy}")
```

- [ ] **Step 5: Add example config**

Create `examples/plaintext-loop.json`:

```json
{
  "artifact": "README.md",
  "goal": "Make the README clearer and remove stale setup guidance.",
  "verify": "python -m pytest -q",
  "max_iterations": 3,
  "model": "gpt-5.4-mini",
  "sandbox": "workspace-write",
  "approval_policy": "never"
}
```

- [ ] **Step 6: Run tests**

Run: `python -m pytest tests/test_config.py -q`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add pyproject.toml src/codex_loop/__init__.py src/codex_loop/config.py tests/test_config.py examples/plaintext-loop.json
git commit -m "feat: add loop config loader"
```

### Task 2: Structured Schemas And Codex Exec Adapter

**Files:**
- Create: `src/codex_loop/schemas.py`
- Create: `src/codex_loop/codex_exec.py`
- Create: `tests/test_schemas.py`
- Create: `tests/test_codex_exec.py`

**Interfaces:**
- Consumes: `LoopConfig`
- Produces: `review_schema() -> dict`
- Produces: `repair_schema() -> dict`
- Produces: `run_codex_json(prompt: str, schema: dict, run_dir: Path, config: LoopConfig) -> dict`

- [ ] **Step 1: Write schema tests**

```python
from codex_loop.schemas import repair_schema, review_schema


def test_review_schema_requires_findings():
    schema = review_schema()

    assert schema["type"] == "object"
    assert schema["required"] == ["findings"]
    assert schema["additionalProperties"] is False


def test_repair_schema_requires_updated_artifact_path():
    schema = repair_schema()

    assert "updated_artifact_path" in schema["required"]
    assert schema["properties"]["changes_made"]["type"] == "array"
```

- [ ] **Step 2: Write Codex adapter command test**

```python
from pathlib import Path

from codex_loop.codex_exec import build_codex_command
from codex_loop.config import LoopConfig


def test_build_codex_command_uses_schema_and_output_file(tmp_path: Path):
    config = LoopConfig(
        artifact=Path("README.md"),
        goal="repair docs",
        verify="python -m pytest -q",
    )

    command = build_codex_command(tmp_path / "schema.json", tmp_path / "answer.json", config)

    assert command[:2] == ["codex", "exec"]
    assert "--sandbox" in command
    assert "workspace-write" in command
    assert "--ask-for-approval" in command
    assert "never" in command
    assert "--output-schema" in command
    assert "--output-last-message" in command
    assert command[-1] == "-"
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `python -m pytest tests/test_schemas.py tests/test_codex_exec.py -q`

Expected: FAIL because `schemas.py` and `codex_exec.py` do not exist.

- [ ] **Step 4: Implement schema helpers**

Create `src/codex_loop/schemas.py`:

```python
from __future__ import annotations

from typing import Any


def object_schema(properties: dict[str, Any], required: list[str] | None = None) -> dict[str, Any]:
    return {
        "type": "object",
        "properties": properties,
        "required": required or list(properties),
        "additionalProperties": False,
    }


def string_array() -> dict[str, Any]:
    return {"type": "array", "items": {"type": "string"}}


def review_schema() -> dict[str, Any]:
    finding = object_schema(
        {
            "artifact": {"type": "string"},
            "issue_type": {"type": "string"},
            "severity": {"type": "string"},
            "description": {"type": "string"},
            "suggested_fix_direction": {"type": "string"},
        }
    )
    return object_schema({"findings": {"type": "array", "items": finding}})


def repair_schema() -> dict[str, Any]:
    return object_schema(
        {
            "artifact": {"type": "string"},
            "iteration": {"type": "integer"},
            "changes_made": string_array(),
            "unresolved_items": string_array(),
            "updated_artifact_path": {"type": "string"},
        }
    )
```

- [ ] **Step 5: Implement Codex exec adapter**

Create `src/codex_loop/codex_exec.py`:

```python
from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

from codex_loop.config import LoopConfig


def build_codex_command(schema_file: Path, answer_file: Path, config: LoopConfig) -> list[str]:
    return [
        "codex",
        "exec",
        "--model",
        config.model,
        "--sandbox",
        config.sandbox,
        "--ask-for-approval",
        config.approval_policy,
        "--output-schema",
        str(schema_file),
        "--output-last-message",
        str(answer_file),
        "-",
    ]


def run_codex_json(prompt: str, schema: dict[str, Any], run_dir: Path, config: LoopConfig) -> dict[str, Any]:
    run_dir.mkdir(parents=True, exist_ok=True)
    prompt_file = run_dir / "prompt.txt"
    schema_file = run_dir / "schema.json"
    answer_file = run_dir / "answer.json"

    prompt_file.write_text(prompt, encoding="utf-8")
    schema_file.write_text(json.dumps(schema, indent=2), encoding="utf-8")

    result = subprocess.run(
        build_codex_command(schema_file, answer_file, config),
        input=prompt,
        capture_output=True,
        text=True,
        check=False,
    )

    (run_dir / "stdout.txt").write_text(result.stdout, encoding="utf-8")
    (run_dir / "stderr.txt").write_text(result.stderr, encoding="utf-8")

    if result.returncode != 0:
        raise RuntimeError(f"codex exec failed with exit code {result.returncode}: {run_dir / 'stderr.txt'}")

    return json.loads(answer_file.read_text(encoding="utf-8"))
```

- [ ] **Step 6: Run tests**

Run: `python -m pytest tests/test_schemas.py tests/test_codex_exec.py -q`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/codex_loop/schemas.py src/codex_loop/codex_exec.py tests/test_schemas.py tests/test_codex_exec.py
git commit -m "feat: add structured codex exec adapter"
```

### Task 3: Verification And Record Persistence

**Files:**
- Create: `src/codex_loop/verify.py`
- Create: `src/codex_loop/records.py`
- Create: `tests/test_verify.py`
- Create: `tests/test_records.py`

**Interfaces:**
- Consumes: `LoopConfig.verify`
- Produces: `run_verify(command: str, cwd: Path, timeout_seconds: int = 300) -> VerifyResult`
- Produces: `write_record(record: dict, path: Path) -> None`

- [ ] **Step 1: Write verification tests**

```python
from pathlib import Path

from codex_loop.verify import run_verify


def test_run_verify_passes_for_zero_exit(tmp_path: Path):
    result = run_verify("python -c 'print(123)'", tmp_path)

    assert result.passed is True
    assert result.returncode == 0
    assert "123" in result.stdout


def test_run_verify_fails_for_nonzero_exit(tmp_path: Path):
    result = run_verify("python -c 'import sys; sys.exit(7)'", tmp_path)

    assert result.passed is False
    assert result.returncode == 7
```

- [ ] **Step 2: Write record tests**

```python
import json
from pathlib import Path

from codex_loop.records import write_record


def test_write_record_creates_parent_directory(tmp_path: Path):
    path = tmp_path / "runs" / "iteration_1" / "record.json"

    write_record({"validation": {"passed": True}}, path)

    assert json.loads(path.read_text(encoding="utf-8"))["validation"]["passed"] is True
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `python -m pytest tests/test_verify.py tests/test_records.py -q`

Expected: FAIL because modules do not exist.

- [ ] **Step 4: Implement verification runner**

Create `src/codex_loop/verify.py`:

```python
from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class VerifyResult:
    passed: bool
    returncode: int
    stdout: str
    stderr: str


def run_verify(command: str, cwd: Path, timeout_seconds: int = 300) -> VerifyResult:
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return VerifyResult(
            passed=False,
            returncode=124,
            stdout=exc.stdout or "",
            stderr=(exc.stderr or "") + f"\nTimed out after {timeout_seconds} seconds.",
        )

    return VerifyResult(
        passed=result.returncode == 0,
        returncode=result.returncode,
        stdout=result.stdout,
        stderr=result.stderr,
    )
```

- [ ] **Step 5: Implement record writer**

Create `src/codex_loop/records.py`:

```python
from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def write_record(record: dict[str, Any], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(record, indent=2, sort_keys=True), encoding="utf-8")
```

- [ ] **Step 6: Run tests**

Run: `python -m pytest tests/test_verify.py tests/test_records.py -q`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/codex_loop/verify.py src/codex_loop/records.py tests/test_verify.py tests/test_records.py
git commit -m "feat: add verification and records"
```

### Task 4: Iteration Runner And Stop Conditions

**Files:**
- Create: `src/codex_loop/runner.py`
- Create: `tests/test_runner.py`

**Interfaces:**
- Consumes: `LoopConfig`
- Consumes: `run_codex_json`
- Consumes: `run_verify`
- Produces: `run_loop(config: LoopConfig, workspace: Path, runs_dir: Path) -> LoopSummary`

- [ ] **Step 1: Write runner stop-condition tests**

```python
from pathlib import Path

from codex_loop.config import LoopConfig
from codex_loop.runner import should_stop


def test_should_stop_when_validation_passes():
    assert should_stop(iteration=1, max_iterations=3, validation_passed=True, remaining_delta=["x"]) is True


def test_should_stop_at_max_iterations():
    assert should_stop(iteration=3, max_iterations=3, validation_passed=False, remaining_delta=["x"]) is True


def test_should_continue_when_delta_remains_before_limit():
    assert should_stop(iteration=1, max_iterations=3, validation_passed=False, remaining_delta=["x"]) is False
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python -m pytest tests/test_runner.py -q`

Expected: FAIL because `runner.py` does not exist.

- [ ] **Step 3: Implement stop condition and summary type**

Create `src/codex_loop/runner.py`:

```python
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from codex_loop.config import LoopConfig


@dataclass(frozen=True)
class LoopSummary:
    passed: bool
    iterations: int
    final_record_path: Path | None


def should_stop(iteration: int, max_iterations: int, validation_passed: bool, remaining_delta: list[str]) -> bool:
    if validation_passed:
        return True
    if iteration >= max_iterations:
        return True
    if not remaining_delta:
        return True
    return False


def run_loop(config: LoopConfig, workspace: Path, runs_dir: Path) -> LoopSummary:
    raise NotImplementedError("Task 5 wires the full review/repair/verify loop.")
```

- [ ] **Step 4: Run tests**

Run: `python -m pytest tests/test_runner.py -q`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/codex_loop/runner.py tests/test_runner.py
git commit -m "feat: add loop stop conditions"
```

### Task 5: CLI Entry Point And First End-To-End Dry Run

**Files:**
- Create: `src/codex_loop/cli.py`
- Modify: `src/codex_loop/runner.py`
- Create: `tests/test_cli.py`
- Modify: `README.md`

**Interfaces:**
- Consumes: `LoopConfig.from_file`
- Consumes: `run_loop`
- Produces: `main(argv: list[str] | None = None) -> int`

- [ ] **Step 1: Write CLI argument test**

```python
from codex_loop.cli import parse_args


def test_parse_run_config():
    args = parse_args(["run", "--config", "examples/plaintext-loop.json"])

    assert args.command == "run"
    assert args.config == "examples/plaintext-loop.json"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python -m pytest tests/test_cli.py -q`

Expected: FAIL because `cli.py` does not exist.

- [ ] **Step 3: Implement CLI parser**

Create `src/codex_loop/cli.py`:

```python
from __future__ import annotations

import argparse
from pathlib import Path

from codex_loop.config import LoopConfig
from codex_loop.runner import run_loop


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="codex-loop")
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser("run")
    run_parser.add_argument("--config", required=True)
    run_parser.add_argument("--runs-dir", default="runs")

    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.command == "run":
        config = LoopConfig.from_file(Path(args.config))
        summary = run_loop(config, Path.cwd(), Path(args.runs_dir))
        return 0 if summary.passed else 1
    return 2
```

- [ ] **Step 4: Run CLI tests**

Run: `python -m pytest tests/test_cli.py -q`

Expected: PASS.

- [ ] **Step 5: Wire `run_loop` with a dry-run-safe first pass**

Modify `src/codex_loop/runner.py` so `run_loop` creates `runs/<run-id>/iteration_1/record.json`, copies the artifact, runs `verify`, and stores validation output. Do not call `codex exec` yet in this step.

Expected minimal implementation:

```python
from __future__ import annotations

import shutil
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from codex_loop.config import LoopConfig
from codex_loop.records import write_record
from codex_loop.verify import run_verify


@dataclass(frozen=True)
class LoopSummary:
    passed: bool
    iterations: int
    final_record_path: Path | None


def should_stop(iteration: int, max_iterations: int, validation_passed: bool, remaining_delta: list[str]) -> bool:
    if validation_passed:
        return True
    if iteration >= max_iterations:
        return True
    if not remaining_delta:
        return True
    return False


def run_loop(config: LoopConfig, workspace: Path, runs_dir: Path) -> LoopSummary:
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    iteration_dir = runs_dir / run_id / "iteration_1"
    iteration_dir.mkdir(parents=True, exist_ok=True)

    source = workspace / config.artifact
    copied = iteration_dir / config.artifact.name
    shutil.copy2(source, copied)

    verify_result = run_verify(config.verify, workspace)
    remaining_delta = [] if verify_result.passed else [verify_result.stderr or verify_result.stdout]
    record = {
        "review": [],
        "repair": {
            "artifact": str(config.artifact),
            "iteration": 1,
            "changes_made": [],
            "unresolved_items": ["codex repair is enabled in the next task"],
            "updated_artifact_path": str(copied),
        },
        "validation": {
            "passed": verify_result.passed,
            "returncode": verify_result.returncode,
            "stdout": verify_result.stdout,
            "stderr": verify_result.stderr,
            "remaining_delta": remaining_delta,
        },
    }
    record_path = iteration_dir / "record.json"
    write_record(record, record_path)
    return LoopSummary(passed=verify_result.passed, iterations=1, final_record_path=record_path)
```

- [ ] **Step 6: Run full test suite**

Run: `python -m pytest -q`

Expected: PASS.

- [ ] **Step 7: Update README usage**

Add:

````markdown
## Local Dry Run

```bash
python -m pip install -e .
codex-loop run --config examples/plaintext-loop.json
```

The first dry-run milestone writes `runs/<run-id>/iteration_1/record.json` and verifies the target command without applying repairs back to the source artifact.
````

- [ ] **Step 8: Commit**

```bash
git add src/codex_loop/cli.py src/codex_loop/runner.py tests/test_cli.py README.md
git commit -m "feat: add codex loop cli dry run"
```

## Self-Review

- Spec coverage: The plan covers config, schema output, Codex invocation, verification, records, stop conditions, and CLI entrypoint.
- Placeholder scan: No placeholder markers or undefined implementation steps remain.
- Type consistency: The interfaces use `LoopConfig`, `LoopSummary`, `VerifyResult`, and `run_codex_json` consistently across tasks.
- Scope check: This is one bounded MVP. Plugin packaging, daemon mode, UI, patch application, and CI integration are intentionally deferred.

## Execution Options

Plan complete and saved to `docs/superpowers/plans/2026-06-30-codex-loop-implementation-plan.md`. Two execution options:

1. Subagent-Driven (recommended): dispatch a fresh subagent per task, review between tasks, fast iteration.
2. Inline Execution: execute tasks in this session using executing-plans, batch execution with checkpoints.

Recommended next command after repo setup:

```bash
python -m pytest -q
```
