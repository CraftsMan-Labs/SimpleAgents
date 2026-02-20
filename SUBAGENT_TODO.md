# SUBAGENT TODO

Track subagent assignments for large tasks. Keep scopes non-overlapping and update statuses continuously.
Every subagent item must map to a parent item in `TODO.md`.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Subagent assignments

| Parent TODO | Subagent | Scope | Why | Expected Outcome | Status | Notes |
|---|---|---|---|---|---|---|
| T2 | SA-Workflow-Reliability | `crates/simple-agents-workflow/**` lock-poison panic paths + minor clippy items | Prevent runtime crashes and keep workflow crate robust | Poison-safe locking + cleaner lint posture in workflow crate | completed | Task `ses_3857aaaeaffergfpkCJISdnuHv`; tests+clippy passed |
| T3 | SA-Providers-Reliability | `crates/simple-agents-providers/**` rate limiter, metrics init, http client default behavior | Eliminate hidden startup failures and panic defaults in provider runtime | Fail-fast startup errors and non-panicking lock/client code paths | completed | Task `ses_3857aaa8fffeir9eKGk1hi5DNB`; tests passed |
| T4, T5 | SA-Bindings-Docs-Python | `bindings/go/README.md` and `crates/simple-agents-py/src/lib.rs` refactor/diagnostics | Improve reproducibility and reduce Python drift/debug blind spots | Accurate Go test docs + shared Python helper paths and clearer callback error handling | completed | Task `ses_3857aaa81ffeiE8bGMmtMyTwPt`; checks passed |
| T1 | Main-Agent | Git hygiene for `docs/node_modules/**` tracking and guarding | Critical repository hygiene fix should be coordinated centrally | Vendored docs dependencies untracked and regression-resistant | completed | Added CI guard in `bindings-ci.yml` and removed tracked vendored files |
| T6 | Main-Agent | Cross-task validation and readiness report | Ensure integrated changes pass quality gates | Verified results with pass/fail notes and follow-up actions | completed | Verification commands captured in `TODO.md` |

## Coordination checklist

- Define each subagent scope so no two subagents own overlapping implementation areas.
- Ensure each subagent assignment references the corresponding parent task in `TODO.md`.
- Provide each subagent with clear instructions: goal, approach, constraints, verification, and expected return format.
- Specify required skill usage whenever relevant.
- Review outputs for completeness and mergeability before integration.
