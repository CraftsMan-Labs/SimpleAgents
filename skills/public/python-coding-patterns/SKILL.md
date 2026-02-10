---
name: python-coding-patterns
description: This skill should be used when the user asks to "implement Python code", "debug Python code", "refactor Python for maintainability", "review Python code", "fix asyncio issues", "improve Python typing", or "apply Python best practices".
---

# Python Coding Patterns

Produce production-ready Python with explicit contracts, stable boundaries, disciplined concurrency, and strong review standards. Use this skill for implementation, debugging, refactoring, and reviews where long-term maintainability matters as much as short-term correctness.

## Purpose

Drive Python work through repeatable architecture and quality gates. Prevent common drift patterns such as untyped boundary spread, silent exception suppression, ad hoc asyncio usage, oversized modules, and weak test isolation. Preserve delivery speed while improving correctness and operational clarity.

## Operating Mode

Start every task by selecting one mode.

1. `implement`
- Define boundary contracts first.
- Choose typing depth and validation strategy.
- Implement with explicit error and concurrency behavior.

2. `debug`
- Gather failure signal from traceback, test failure, runtime behavior, or profiling.
- Map symptom to design cause.
- Apply coherent fix and add regression coverage.

3. `review`
- Report findings first by severity.
- Explain runtime impact and triggering conditions.
- Recommend minimal corrective path.

4. `refactor`
- Preserve public behavior.
- Improve module boundaries, type clarity, and testability.
- Add tests that lock intended behavior.

## Entry Checklist

Complete this checklist before code changes.

1. Workload classification.
- CPU-heavy.
- I/O-heavy.
- Mixed.

2. Correctness constraints.
- Domain invariants.
- Input/output schema rules.
- Failure classes and recovery expectations.

3. Operational constraints.
- Latency/throughput expectations.
- Memory and queue limits.
- Cancellation and shutdown behavior.

4. Compatibility constraints.
- API and payload stability.
- Migration impact.
- Dependency constraints.

## Reference Loading Map

Load only the files needed for the task.

- Async orchestration and cancellation:
`references/asyncio-patterns.md`
- Typing, architecture, errors, and performance:
`references/core-patterns.md`
- Symptom-to-design troubleshooting:
`references/error-to-design-map.md`
- Findings-first review rubric:
`references/review-checklist.md`
- Domain-specific constraints:
`references/domain-web.md`
`references/domain-cli.md`
`references/domain-data.md`

## Implementation Workflow

Follow this sequence unless constraints require variation.

1. Define boundary contract.
- Specify accepted input models and output models.
- Make optionality explicit.
- Separate external payload shape from internal domain shape.

2. Define typing strategy.
- Type public APIs and boundary adapters first.
- Use protocol-based interfaces for behavior contracts.
- Keep `Any` localized and justified.

3. Define validation strategy.
- Validate untrusted input at boundaries.
- Convert into typed internal representations.
- Reject invalid state early.

4. Define error strategy.
- Use domain-specific exception taxonomy.
- Wrap lower-level exceptions with contextual intent.
- Separate user-facing error message from internal diagnostics.

5. Define concurrency strategy.
- Use synchronous flow for simple CPU or linear logic.
- Use `asyncio` for high-concurrency I/O paths.
- Bound parallelism via semaphore or queue limits.

6. Implement observability.
- Add structured logs around external boundaries.
- Include correlation identifiers for multi-step flows.
- Emit signals that support failure triage.

7. Implement verification.
- Unit tests for core business logic.
- Integration tests for boundary adapters.
- Async tests for cancellation, timeout, and boundedness behavior.

## Debugging Workflow

Use design-trace debugging instead of local patching.

1. Capture primary signal.
- Traceback and failing test.
- Runtime event-loop warnings.
- Latency or memory regression.
- Type-checker diagnostics.

2. Translate symptom to design question.
- Data model mismatch.
- Optionality and nullability drift.
- Async lifecycle unsupervised.
- Error handling too broad or too narrow.

3. Apply coherent fix.
- Change boundary contract when model mismatch exists.
- Replace broad catch with typed error handling.
- Replace unsupervised tasks with structured orchestration.
- Add explicit timeout and retry policy.

4. Verify and harden.
- Reproduce before and after behavior.
- Add regression tests.
- Confirm static typing and linting signals are clean.

## Linting And Type Gates

Run these checks before declaring Python work complete.

1. Formatting and lint.
- `ruff format --check .`
- `ruff check .`

2. Type checking.
- `pyright`
- If `pyright` is not on PATH, use `python -m pyright` or project script (for example `uv run pyright`).

3. Tests.
- Run impacted tests first, then broader suite when risk is high.

4. Fix policy.
- Prefer fixing root causes over adding ignores.
- Add ignore comments only with a short justification.

## Review Workflow

Use findings-first review structure.

1. Severity order.
- S0: security, data loss, critical outage risk.
- S1: correctness bug with user-visible impact.
- S2: reliability/performance/maintainability risk.
- S3: style and documentation.

2. Finding structure.
- What is wrong.
- Why behavior is risky.
- How to trigger.
- Minimal remediation.
- Missing tests.

3. Python-specific review lenses.
- Boundary typing quality.
- Validation completeness at trust boundaries.
- Exception taxonomy and context quality.
- Async cancellation and task supervision.
- Module cohesion and dependency direction.
- Hidden mutable defaults and shared state hazards.

4. Evidence standard.
- Provide file/line pointers.
- Include concrete runtime path.
- Avoid speculative findings without trigger scenario.

## Architecture Heuristics

Use these heuristics to keep codebases evolvable.

1. Separate domain from transport.
- Keep business logic pure and framework-agnostic.
- Keep HTTP/CLI/queue adapters thin.
- Convert errors and payloads at boundaries.

2. Keep modules cohesive.
- Group by capability, not by accidental utility sprawl.
- Keep side effects explicit and localized.
- Prefer dependency injection for testability.

3. Keep interfaces narrow.
- Define protocol contracts on consumer side.
- Avoid broad manager-style classes.
- Prefer simple composable functions over deep inheritance.

4. Keep state transitions explicit.
- Validate state changes in one place.
- Avoid hidden mutation across call graph.
- Treat global mutable state as exceptional.

## Asyncio Principles

Apply these rules for reliable async Python.

1. Supervise task lifecycles.
- Use structured concurrency when available.
- Track and await task completion intentionally.
- Treat orphan tasks as defects unless explicitly detached.

2. Bound concurrency.
- Limit fan-out with semaphores.
- Use bounded queues for producer/consumer flows.
- Define overflow behavior explicitly.

3. Make cancellation reliable.
- Treat cancellation as normal path.
- Use cleanup blocks and re-raise cancellation.
- Define timeout budget for shutdown.

4. Protect loop health.
- Avoid blocking calls in async functions.
- Offload CPU-heavy work from event loop.
- Avoid high-frequency polling without backoff.

## Error Strategy Principles

Standardize failure behavior across modules.

1. Classify errors.
- Validation failures.
- Dependency/transient failures.
- Internal invariant failures.

2. Preserve context.
- Add operation context at boundaries.
- Keep original cause linked for debugging.

3. Map errors by boundary.
- API layer: stable response model.
- CLI layer: actionable terminal messages and exit codes.
- Worker layer: retry and dead-letter policy signals.

4. Avoid anti-patterns.
- Bare `except`.
- Silent pass on errors.
- Error-only string protocol matching.

## Performance Workflow

Optimize only after measurement.

1. Measure baseline.
- Latency and throughput.
- Memory profile.
- Hotspot and allocation profile.

2. Optimize in order.
- Algorithm and data structure.
- Serialization and object churn.
- I/O parallelism and batching.
- Localized micro-optimizations.

3. Verify tradeoffs.
- Preserve behavior with tests.
- Re-run benchmark under representative load.
- Document complexity and readability impact.

## Tooling and Verification Baseline

Apply these quality gates before finalizing work.

- Format check.
- Lint check.
- Type checker pass.
- Unit and integration tests.
- Async edge-case tests for cancellation, timeout, and boundedness.

Prefer project-standard tooling. Use configured tools from repository before introducing alternatives.

## Deliverable Format

Return outputs in this order.

1. Design summary.
- Typing strategy.
- Validation boundaries.
- Error model.
- Concurrency model.

2. Implementation summary.
- Files changed.
- Behavioral impact.
- Compatibility notes.

3. Risk notes.
- Remaining hazards.
- Runtime assumptions.
- Monitoring hooks.

4. Verification summary.
- Commands run.
- Tests added/updated.
- Known gaps.

## Anti-Drift Rules

Apply these rules during maintenance.

- Reject untyped boundary spread in shared modules.
- Reject broad exception handling without rationale.
- Reject unbounded async fan-out in service paths.
- Reject hidden global mutable state.
- Reject large behavior changes without regression tests.

## Scenario Playbooks

Use these playbooks for frequent workflows.

1. Add a new API endpoint with validation.
- Define request and response models explicitly.
- Validate payload at boundary before domain logic.
- Convert transport model to domain model.
- Map domain exceptions to stable API error responses.
- Add tests for invalid input, timeout, dependency failure, and success.

2. Migrate a module from untyped to typed contracts.
- Type public functions first.
- Introduce typed models for shared payloads.
- Replace implicit dictionary usage with explicit structures.
- Eliminate broad `Any` propagation.
- Add type-check gating in verification flow.

3. Stabilize async worker pipeline.
- Introduce bounded queue and worker semaphore.
- Define cancellation and shutdown sequence.
- Add timeout around external dependencies.
- Classify retryable vs permanent failures.
- Add tests for queue saturation and cancellation behavior.

4. Refactor exception-heavy code path.
- Replace broad catches with specific exception types.
- Preserve operation context during wrapping.
- Separate user-facing messages from diagnostic details.
- Add explicit handling for expected recoverable failures.

## Decision Tables

Use these quick choices during implementation.

1. Sync vs async.
- Predominantly I/O with many concurrent waits: async workflow.
- Predominantly CPU-bound transformation: sync/process-oriented workflow.
- Mixed workload: isolate CPU hotspots from event loop paths.

2. Data contract strategy.
- External payloads: validate at boundary.
- Internal domain data: typed models with invariant checks.
- Cross-module contracts: explicit typed interfaces.

3. Error handling strategy.
- User-correctable input issue: validation error class.
- Transient dependency issue: retry-aware dependency error class.
- Invariant breach: fail fast path with strong diagnostics.

4. Test strategy.
- Pure transforms: unit tests with edge cases.
- Adapter behavior: integration tests with boundary mocks or fixtures.
- Async orchestration: timeout/cancellation/boundedness tests.

## Additional Resources

Load detailed references when needed.

- `references/asyncio-patterns.md`
- `references/core-patterns.md`
- `references/error-to-design-map.md`
- `references/review-checklist.md`
- `references/domain-web.md`
- `references/domain-cli.md`
- `references/domain-data.md`
