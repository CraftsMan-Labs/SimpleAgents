---
name: typescript-javascript-coding-patterns
description: This skill should be used when the user asks to "implement TypeScript code", "debug JavaScript code", "refactor JS/TS code", "review TypeScript code", "fix async JavaScript issues", "improve TypeScript types", or "apply JavaScript best practices".
---

# TypeScript/JavaScript Coding Patterns

Deliver robust TypeScript and JavaScript systems with explicit contracts, reliable async behavior, and maintainable boundaries. Apply this skill to implementation, debugging, refactoring, and reviews across Node, browser, edge, and shared-library code.

## Purpose

Reduce failure modes caused by weak runtime contracts, unsafe type assertions, unbounded async fan-out, and implicit state coupling. Build code that remains predictable under load, clear under review, and resilient to product evolution.

## Operating Mode

Select mode at task start.

1. `implement`
- Define runtime and type boundaries first.
- Select validation and error strategy.
- Implement with explicit concurrency controls.

2. `debug`
- Capture runtime error, type-check error, flaky test, or perf regression.
- Map symptom to design-level cause.
- Apply coherent fix and regression tests.

3. `review`
- Report findings by severity and impact.
- Provide concrete reproduction path.
- Recommend minimal safe remediation.

4. `refactor`
- Preserve behavior.
- Simplify architecture and contract clarity.
- Strengthen type coverage and test reliability.

## Entry Checklist

1. Runtime target.
- Node service.
- Browser frontend.
- Edge runtime.
- Shared package.

2. Constraint capture.
- Latency/throughput budgets.
- Memory limits and queue depth.
- Compatibility and version policy.
- Failure tolerance and retry policy.

3. Contract capture.
- Input validation boundaries.
- Domain model ownership.
- Error categories and mapping.

4. Concurrency capture.
- Parallelism bounds.
- Cancellation requirements.
- Backpressure behavior.

## Reference Loading Map

Load only required files.

- Async concurrency, cancellation, event-loop health:
`references/async-patterns.md`
- Type boundaries, module design, error and perf strategy:
`references/core-patterns.md`
- Symptom-to-design diagnostics:
`references/error-to-design-map.md`
- Findings-first review rubric:
`references/review-checklist.md`
- Domain constraints:
`references/domain-web.md`
`references/domain-frontend.md`
`references/domain-cli.md`

## Implementation Workflow

1. Define boundary contracts.
- Treat all external input as untrusted.
- Validate boundary payloads before business logic.
- Convert unknown payloads into typed domain models.

2. Define type strategy.
- Use strict TypeScript settings where available.
- Keep `unknown` narrowed quickly.
- Keep `any` localized and justified.
- Use discriminated unions for stateful outcomes.

3. Define module architecture.
- Separate core domain from I/O adapters.
- Keep side effects at edge modules.
- Keep cross-module contracts stable and narrow.

4. Define error strategy.
- Standardize domain error categories.
- Preserve root cause context.
- Convert internal errors to boundary-safe forms.

5. Define async strategy.
- Bound parallel tasks.
- Add cancellation support via abort signals.
- Set timeout budgets for remote calls.
- Define retry policy for transient failures only.

6. Implement observability.
- Attach request or operation IDs.
- Emit structured logs around boundary calls.
- Include timing and retry metadata where relevant.

7. Implement verification.
- Unit tests for pure logic.
- Integration tests for boundary conversion and error mapping.
- Async tests for cancellation, timeout, and bounded fan-out.

## Debugging Workflow

1. Capture failure signal.
- Runtime exceptions.
- Unhandled rejection warnings.
- Type-check diagnostics.
- Memory growth or latency regressions.

2. Translate to design question.
- Missing runtime validation.
- Unsound type assertion usage.
- Async lifecycle unsupervised.
- Module boundary leakage.

3. Apply coherent fix.
- Add or tighten schema validation.
- Replace assertion-heavy flow with narrowing and model conversion.
- Supervise background tasks and propagate cancellation.
- Introduce concurrency caps.

4. Verify and harden.
- Reproduce prior failure.
- Confirm corrected behavior.
- Add regression tests and static checks.

## Linting And Type Gates

Run these checks before declaring JS/TS work complete.

1. Formatting/linting.
- Prefer project script first: `bun run lint` / `npm run lint` / `pnpm lint` / `yarn lint`.
- If no script exists, run ESLint directly (for example `npx eslint .`).

2. Type checking (TypeScript projects).
- Prefer project script first: `bun run typecheck` / `npm run typecheck` / `pnpm typecheck` / `yarn typecheck`.
- If no script exists, run `tsc --noEmit`.

3. Tests/build.
- Run impacted tests and relevant build validation.

4. Fix policy.
- Avoid broad `any`/`@ts-ignore`/eslint disables.
- If suppression is required, keep it local and explain why.

## Publish Checks

Run these commands before publishing a JS/TS package.

1. Pre-publish verification.
- Prefer Bun first: `bun run lint`, `bun run typecheck`, `bun test`.
- Then verify package contents without publishing: `bun publish --dry-run`.

2. Publish command.
- Bun: `bun publish`.
- Fallbacks when Bun is not used: `npm publish` / `pnpm publish` / `yarn npm publish`.

3. Safety checks.
- Confirm version bump and changelog status before publish.
- Ensure auth/token and registry target are correct.

## Review Workflow

Use findings-first review structure.

1. Severity order.
- S0: security, data corruption, outage risk.
- S1: correctness bug.
- S2: reliability/perf/maintainability risk.
- S3: style and docs.

2. Finding structure.
- Defect statement.
- Runtime impact.
- Trigger conditions.
- Minimal remediation.
- Testing gap.

3. JS/TS-specific review lenses.
- Boundary validation completeness.
- Type safety and assertion discipline.
- Async cancellation and timeout behavior.
- Concurrency bounds and queue discipline.
- Error normalization and observability context.
- Shared state coupling across modules.

4. Evidence standard.
- Provide file/line pointers.
- Include concrete runtime flow.
- Avoid speculative risk without scenario.

## Architecture Heuristics

1. Prefer explicit boundaries.
- Convert transport models to domain models early.
- Keep domain logic transport-agnostic.
- Avoid leaking framework types into core modules.

2. Prefer predictable state transitions.
- Represent state with explicit unions.
- Model impossible states out of public APIs.
- Keep transition side effects isolated.

3. Prefer narrow interfaces.
- Export minimal surface area.
- Avoid utility dumping into shared modules.
- Keep dependency direction toward core stability.

4. Prefer composable functions.
- Split orchestration from pure transforms.
- Keep function-level side effects clear.
- Keep test seams obvious.

## Async Principles

1. Supervise promises and tasks.
- Avoid fire-and-forget unless intentionally detached and monitored.
- Aggregate and inspect failures.
- Propagate abort signals through nested operations.

2. Bound fan-out.
- Cap concurrency for external calls.
- Cap queue depth.
- Define overload behavior.

3. Protect event-loop health.
- Isolate CPU-heavy work.
- Avoid synchronous bottlenecks in hot async paths.
- Avoid starvation from uncontrolled microtask chains.

4. Standardize resilience.
- Timeout every remote call.
- Retry only transient categories.
- Use capped exponential backoff with jitter.

## Error Strategy Principles

1. Classify errors.
- Input/validation errors.
- Dependency/transient errors.
- Internal invariant failures.

2. Preserve causality.
- Keep root cause references.
- Attach operation context.
- Avoid lossy string-only conversions.

3. Map per boundary.
- Service API: stable response shape and code mapping.
- Frontend: user-safe messaging with telemetry detail retained.
- CLI: actionable stderr and stable exit code behavior.

4. Avoid anti-patterns.
- Catch-all error swallowing.
- Inconsistent error objects.
- Assertion-driven control flow.

## Performance Workflow

1. Measure first.
- Capture latency, throughput, memory, and event-loop responsiveness.

2. Optimize in order.
- Algorithm and structure.
- Serialization and parsing overhead.
- Concurrency and batching.
- Localized micro-optimizations.

3. Verify tradeoffs.
- Preserve semantics with tests.
- Re-measure under representative conditions.
- Document readability and maintenance costs.

## Tooling and Verification Baseline

Run project-standard checks before finalizing.

- Format.
- Lint.
- Type check.
- Unit and integration tests.
- Async edge-case tests for cancellation and timeout behavior.

Avoid introducing non-standard tooling unless repository policy requires it.

## Deliverable Format

Return outputs in this order.

1. Design summary.
- Type strategy.
- Validation boundaries.
- Error strategy.
- Async/concurrency strategy.

2. Implementation summary.
- Files changed.
- Behavioral impact.
- Compatibility notes.

3. Risk notes.
- Remaining hazards.
- Operational assumptions.
- Monitoring hooks.

4. Verification summary.
- Commands run.
- Tests updated.
- Known gaps.

## Anti-Drift Rules

- Reject unvalidated external input in core logic.
- Reject broad unsafe assertions in shared paths.
- Reject unbounded async fan-out.
- Reject hidden global mutable state dependencies.
- Reject major behavior changes without regression coverage.

## Scenario Playbooks

Use these playbooks for common delivery patterns.

1. Add a new service endpoint.
- Define request schema and response schema.
- Validate payload before domain handling.
- Convert schema output into typed internal model.
- Apply timeout and retry policy on dependency calls.
- Add tests for malformed input, timeout, dependency failure, and success.

2. Harden frontend async data flow.
- Add abort handling for in-flight request replacement.
- Guard state transitions with explicit union states.
- Normalize error shape for UI rendering.
- Add tests for race conditions and stale-response suppression.

3. Refactor assertion-heavy module.
- Replace repeated `as` assertions with narrowing helpers.
- Introduce discriminated unions for branch safety.
- Move runtime validation to boundary adapters.
- Keep internal functions strongly typed and assertion-light.

4. Control queue and fan-out growth in Node service.
- Add explicit concurrency cap for downstream calls.
- Bound queue depth and define overload behavior.
- Add instrumentation for queue depth and latency.
- Add integration tests for saturation behavior.

## Decision Tables

Use these quick choices to reduce ambiguity.

1. Runtime validation placement.
- External inputs: boundary validation mandatory.
- Internal trusted flow: narrow and enforce via type system.
- Shared package API: validate at public entry points.

2. Error representation.
- Cross-layer transport boundary: normalized error envelope.
- Internal domain workflow: typed error categories/unions.
- CLI interactions: actionable stderr and stable exit signaling.

3. Async coordination.
- Small fixed set of parallel tasks: direct promise orchestration.
- High-cardinality external operations: bounded concurrency primitive.
- Cancelable workflows: abort-signal propagation end-to-end.

4. State modeling.
- UI or workflow states with fixed variants: discriminated unions.
- Dynamic maps keyed by IDs: typed records/maps with guards.
- Shared mutable singleton needs: avoid unless lifecycle control is explicit.

## Quality Gate

Before finalizing work, confirm all items.

- Boundary payloads are validated before domain logic.
- Type assertions are minimized and justified.
- Async operations are bounded, cancelable, and timeout-aware.
- Error mapping is stable and observable at boundaries.
- Tests cover success, failure, edge, and cancellation paths.
- Format, lint, type-check, and tests pass with project tooling.

## Additional Resources

Load detailed guides when needed.

- `references/async-patterns.md`
- `references/core-patterns.md`
- `references/error-to-design-map.md`
- `references/review-checklist.md`
- `references/domain-web.md`
- `references/domain-frontend.md`
- `references/domain-cli.md`
