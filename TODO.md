# Repository Stabilization Plan

Date: 2026-02-20
Source: Findings documented in `CODE_REVIEW_TASK_SHEET.md`

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Master tasks

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| T1 | Remove tracked `docs/node_modules/**` and enforce hygiene guard | Vendored dependencies are a critical repo hygiene and supply-chain risk | `docs/node_modules/**` removed from git tracking, lockfile retained, guard prevents regressions | completed |
| T2 | Eliminate runtime panic paths in `simple-agents-workflow` | `expect/unwrap` on poisoned locks can crash runtime behavior | Lock poisoning handled safely with recovery or typed errors | completed |
| T3 | Fix provider reliability contracts in `simple-agents-providers` | Hidden startup failures and panic defaults create false readiness and crashes | Prometheus init fails fast, rate limiter lock handling is non-panicking, `HttpClient` defaults are safe | completed |
| T4 | Align Go binding test documentation with actual local requirements | Current instructions are not reproducible without linker setup | `bindings/go/README.md` has accurate local test instructions | completed |
| T5 | Address minor maintainability and lint debt | Reduces future drift and keeps quality gates clean | Clippy minor items fixed; Python duplication/error handling improved | completed |
| T6 | Run verification suite for touched areas and finalize readiness notes | Ensure changes are correct and stable before next work | Relevant tests/lints pass and results are documented | completed |

## Execution notes

- Tasks T2-T5 are designed for parallel non-overlapping subagent execution.
- Every subagent task is mapped in `SUBAGENT_TODO.md`.
- Main agent will integrate, validate, and finalize T6.

## Verification completed

- `git ls-files "docs/node_modules" "docs/node_modules/**" | wc -l` (result: `0`)
- `cargo test -p simple-agents-workflow --lib --tests`
- `cargo clippy -p simple-agents-workflow --all-targets -- -D warnings`
- `cargo test -p simple-agents-providers --lib --tests`
- `cargo check -p simple-agents-py`
- `make test-go-bindings`
