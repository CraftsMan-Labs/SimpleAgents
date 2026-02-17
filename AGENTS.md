# AGENTS Coding Practices

This document defines practical coding standards for contributors and automated agents.

## Core Principles

- Prefer correctness over cleverness.
- Keep changes minimal, local, and reversible.
- Optimize for readability first, then performance.
- Preserve existing public behavior unless a change is intentional and documented.
- Balance developer experience and performance deliberately; optimize APIs and internals so both remain strong.

## Design and Architecture

- Use single-responsibility modules and small functions.
- Avoid duplicate logic; extract shared helpers when repetition appears.
- Keep domain logic separate from transport/binding code.
- Prefer explicit interfaces over hidden side effects.
- Follow object-oriented design principles where applicable: clear abstractions, strong encapsulation, and composable interfaces.
- Avoid vague or unstructured return types; each function should return a well-defined, explicit type/object.

## Rust-First Implementation Policy

- Implement all source-of-truth behavior in Rust crates first.
- Before implementing any feature or fix, search existing crates for similar logic and reuse or extend it instead of duplicating.
- Keep Rust APIs ergonomic and explicit to support clean downstream bindings.
- Optimize for maintainability and developer experience; avoid inefficient, hard-to-read, or fragile abstractions.
- Never add use-case-specific business logic to core crates/bundles; when needed, add generic capabilities and keep use-case rules in workflows/adapters.

## Pre-Implementation Checklist

- Search existing crates/modules for similar behavior before writing new code.
- Prefer extending shared abstractions over adding one-off implementations.
- Confirm use-case-specific logic stays outside core bundles.
- Define explicit input/output types and error contracts before implementation.
- Identify and load relevant skill guidance before major coding work.
- For large tasks, map parent items in `TODO.md` and subagent ownership in `SUBAGENT_TODO.md` before implementation.

## Reliability and Safety

- Do not introduce panic paths in runtime code (`unwrap`/`expect` only in tests or impossible invariants).
- Validate all external inputs at boundaries.
- Use typed errors with actionable messages.
- Respect cancellation, timeouts, and retry boundaries in async paths.
- Use explicit null/none checks instead of truthy/falsy shortcuts in typed code paths (e.g., prefer `a is None` / `a == None` semantics over `if not a` when emptiness and null are different states).

## Truthy/Falsy Clarity

- Treat nullability and emptiness as different states unless explicitly designed otherwise.
- Rust: use `Option<T>` matching (`if let Some(x) = v`) instead of inferred truthy behavior.
- Python: use `is None` / `is not None` for null checks; use explicit length/value checks for emptiness.
- TypeScript/JavaScript: use strict null checks (`value === null` / `value === undefined`) and avoid ambiguous `if (!value)` when empty strings/zero are valid.
- Go: use explicit `nil` checks and clear length checks (`len(s) == 0`) instead of overloaded conditional assumptions.

## Async and Concurrency

- Never block async executors with blocking locks or blocking I/O.
- Avoid `.await` while holding mutex/RwLock guards.
- Bound parallelism and queue sizes.
- Ensure retry loops have explicit max-attempt guards.

## YAML Workflow Standards

- Define `config.output_schema` for every `llm_call` node.
- Prefer one clear responsibility per node.
- Keep routing conditions simple and deterministic.
- Model terminal/closed-session states explicitly.
- Keep interview/chat systems one-question-at-a-time unless intentionally multi-part.

## Language Binding Standards

- Keep behavior consistent across Rust/Python/Node/Go surfaces.
- Reuse shared parsing/mapping logic from core crates when possible.
- Avoid embedding workflow-specific business rules in bindings.
- Maintain backward-compatible wrappers for renamed APIs.
- Build bindings only after the Rust implementation is finalized for the target behavior.
- Ensure each binding has linting, formatting, type-checking/LSP support, and clear local dev workflows.
- Keep binding DX predictable: typed interfaces, actionable errors, and parity tests against Rust behavior.

## Binding Quality Gates

- Rust: `make test-rust`, `make clippy`, `make fmt`.
- Python: `make test-python`; require typing-friendly APIs (`.pyi`/typed signatures) and editor/LSP compatibility.
- Node/TypeScript: `make build-node`, `make test-node`; maintain `.d.ts` correctness and strict type compatibility.
- Go: `make release-go`, `make test-go-bindings`; keep exported APIs typed and `gopls`-friendly.
- Cross-language contract/parity: `make test-binding-contracts` and `make test-binding-layers` for binding consistency.

## Testing and Verification

- Add regression tests for every bug fix.
- Cover both success and failure paths.
- Add concurrency-focused tests for lock/scheduler changes.
- Validate examples still run for touched workflows.
- Prefer project `make` targets for build/test/lint/verification commands when available.
- Do not modify `Makefile` without explicit owner permission; add helper commands only with approval.

## Documentation Standards

- Update docs in the same PR as behavior changes.
- Add docs under `docs/` for every newly implemented feature as soon as the feature is implemented.
- Minor tasks and bug fixes do not require new feature documentation unless behavior or contracts changed.
- Include runnable commands and realistic examples.
- Keep docs explicit about defaults, constraints, and failure modes.
- Prefer concise language; avoid ambiguous instructions.
- Do not create unnecessary `README.md` files; prefer updating existing docs unless a new README is clearly justified.
- Follow task instructions precisely and communicate implementation decisions with clarity.
- If questions, risks, or improvement ideas come up during implementation, raise them proactively.

## Definition of Done

- Rust source-of-truth implementation is complete, tested, and reviewed for reuse opportunities.
- No duplicated or use-case-specific core logic was introduced.
- Function contracts use explicit input/output types and actionable typed errors.
- Required checks pass using project `make` targets relevant to touched areas.
- Docs/examples are updated for behavior changes, defaults, and failure modes.
- Bindings (if touched) preserve parity with Rust behavior and pass language-specific quality gates.

## Task Tracking Expectations

- For each non-trivial task, create or update `TODO.md` to track execution.
- For every tracked item, include short context on why the task is being done and the expected outcome.
- Mark task status as work progresses (pending, in_progress, completed, blocked).
- Add workflow diagrams when they materially improve understanding of system flow.
- Include technical notes (code snippets or pseudocode) when they reduce ambiguity for future contributors.

## Subagent Coordination Standards

- For large tasks, use subagents proactively to parallelize independent workstreams.
- Do not spawn subagents with heavily overlapping scopes; split work into clearly separated ownership areas.
- Before launching each subagent, provide clear instructions: task goal, implementation approach, constraints, verification steps, and expected output format.
- Use the appropriate specialized skills whenever relevant (for subagent and non-subagent work alike).
- Maintain `SUBAGENT_TODO.md` to track all subagents, assigned scope, status, and outcomes; update status as work progresses.
- Keep `SUBAGENT_TODO.md` explicitly aligned with `TODO.md` so each subagent task maps to a parent master task.

## Git and Change Hygiene

- Make focused commits with clear intent.
- Never commit secrets or credential files.
- Do not mix unrelated refactors with feature/bug fixes.
- Keep working tree clean after completing a task.
- After each completed task (or coherent task batch), commit and push to remote using repository git conventions.
- Use meaningful commit messages that describe the intent and impact of the change.
