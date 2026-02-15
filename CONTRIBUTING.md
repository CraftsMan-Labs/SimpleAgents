# Contributing to SimpleAgents

Thanks for contributing.

## Required Workflow

1. Work on a feature branch.
2. Keep `TODO.md` updated for your subagent/workstream.
3. Run relevant tests before opening a PR.
4. Include evidence for completed checklist items.

## Subagent Checklist Discipline

- Update `TODO.md` in the same PR as your code changes.
- Mark completed tasks from `[ ]` to `[x]` and add one-line evidence.
- If blocked, mark as `[~]` with blocker reason and owner.
- Do not mark tasks done without tests.

## Skill Usage Discipline

When working through the coding agent workflow:

- Use language-specific skills when applicable:
  - Rust changes: `rust-coding-patterns`
  - Go changes: `go-coding-patterns`
  - Python changes: `python-coding-patterns`
  - Node/TS changes: `typescript-javascript-coding-patterns`
- Keep changes aligned with `CODING_GUIDELINES.md` (KISS, DRY, OOD, no phantom code).

## Pull Request Checklist

- [ ] `TODO.md` updated with task state and evidence
- [ ] Relevant unit/contract/live tests executed
- [ ] Docs updated (if behavior or contracts changed)
- [ ] CI checks green, including binding capability gates
