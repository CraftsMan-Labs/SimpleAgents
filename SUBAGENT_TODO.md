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

## Coordination checklist

- Define each subagent scope so no two subagents own overlapping implementation areas.
- Ensure each subagent assignment references the corresponding parent task in `TODO.md`.
- Provide each subagent with clear instructions: goal, approach, constraints, verification, and expected return format.
- Specify required skill usage whenever relevant.
- Review outputs for completeness and mergeability before integration.
