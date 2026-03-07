# SUBAGENT TODO

Track active subagent assignments only. Completed items have been removed.
Every subagent item must map to a parent item in `TODO.md`.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Subagent assignments

| Parent TODO | Subagent | Scope | Why | Expected Outcome | Status | Notes |
|---|---|---|---|---|---|---|
| P1 | SA-Parity-Matrix | `docs/**`, `parity-fixtures/**`, binding API surfaces (`*.pyi`, `index.d.ts`, `bindings/go/simpleagents.go`) | All implementation and tests need one explicit parity target | Versioned parity matrix + prioritized gap list (P0/P1) with acceptance criteria | pending | Parent: `TODO.md` P1 |
| T1 | SA-Telemetry-Sampling | `crates/simple-agents-workflow/src/yaml_runner.rs`, tracing docs, binding docs | Trace cost control requires enforced sampling semantics rather than metadata-only fields | Deterministic per-trace sampling (`sample_rate`), validation, metadata exposure, and docs alignment | completed | Parent: `TODO.md` T1 |
| D3 | SA-Docs-IA | `docs/.vitepress/config.mjs`, `docs/DOCS_MAP.md` | Navigation quality is central to docs DX and should be validated separately | Sidebar/docs-map parity with role-based reading paths and minimal overlap | completed | Parent: `TODO.md` D3 |
| D4 | SA-Docs-Content | `docs/QUICKSTART.md`, `docs/USAGE.md`, workflow and architecture guides | Priority pages need consistent rewrite to shared template without breaking technical depth | Outcome-first, runnable, cross-linked pages with troubleshooting and next steps | completed | Parent: `TODO.md` D4 |
| D5 | SA-Docs-QA | docs CI workflow files and docs quality checks | Prevent regressions and stale links as docs volume grows | Automated docs build + link/style checks on PRs | completed | Parent: `TODO.md` D5 |

## Coordination checklist

- Define each subagent scope so no two subagents own overlapping implementation areas.
- Ensure each subagent assignment references the corresponding parent task in `TODO.md`.
- Provide each subagent with clear instructions: goal, approach, constraints, verification, and expected return format.
- Specify required skill usage whenever relevant.
- Review outputs for completeness and mergeability before integration.
