# codex-loop

`codex-loop` is a local workflow scaffold for building auditable Codex repair loops.

The first target is a small command-line runner that wraps `codex exec` into a repeatable cycle:

1. Review an artifact with structured findings.
2. Repair a bounded copy of the artifact.
3. Run mechanical validation.
4. Save a per-iteration `record.json`.
5. Stop when validation passes, the max attempt limit is reached, or human review is required.

The implementation plan is tracked in:

- `docs/superpowers/plans/2026-06-30-codex-loop-implementation-plan.md`

## Design Direction

- Keep Codex as the repair engine, not the orchestrator.
- Keep the outer loop deterministic and auditable.
- Prefer mechanical validation over subjective judging.
- Store every prompt, schema, response, validation result, and diff under a run directory.
- Default to `workspace-write` sandboxing for local automation.

## Status

Repository initialized with planning artifacts only. No runtime implementation has been added yet.
