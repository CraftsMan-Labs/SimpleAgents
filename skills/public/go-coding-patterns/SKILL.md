---
name: go-coding-patterns
description: This skill should be used when the user asks to "implement Go code", "debug Go code", "refactor Go for performance", "review Go code", "fix goroutine and channel issues", "improve Go error handling", or "apply Go best practices".
---

# Go Coding Patterns

Produce production-grade Go with disciplined concurrency, explicit context propagation, coherent error contracts, and review-ready architecture. Apply this skill to implementation, debugging, refactoring, and review tasks across services, workers, CLIs, and shared packages.

## Purpose

Avoid common reliability failures such as goroutine leaks, unbounded fan-out, weak cancellation behavior, error context loss, and interface over-abstraction. Build code that remains clear under maintenance and predictable under load.

## Operating Mode

Choose one mode at task start.

1. `implement`
- Define package and boundary contracts first.
- Choose concurrency and error strategy.
- Implement with explicit shutdown behavior.

2. `debug`
- Capture signal: race, deadlock, panic, wrong behavior, latency regression.
- Map signal to design cause.
- Apply coherent fix and add regression coverage.

3. `review`
- Report findings by severity and impact.
- Provide concrete trigger path.
- Recommend minimal remediation.

4. `refactor`
- Preserve behavior and compatibility.
- Simplify package boundaries and lifecycle control.
- Add tests that lock expected behavior.

## Entry Checklist

1. Workload class.
- CPU-bound.
- I/O-bound.
- Mixed.

2. Correctness constraints.
- Domain invariants.
- Ordering and consistency requirements.
- Failure class semantics.

3. Operational constraints.
- SLO targets.
- Resource limits.
- Shutdown and drain guarantees.

4. Compatibility constraints.
- Public API stability.
- Wire/protocol compatibility.
- Dependency and runtime constraints.

## Reference Loading Map

Load only files required by the current task.

- Goroutine lifecycle, channels, context and cancellation:
`references/concurrency-patterns.md`
- Package/API design, error strategy, performance:
`references/core-patterns.md`
- Symptom-to-design diagnostics:
`references/error-to-design-map.md`
- Findings-first review rubric:
`references/review-checklist.md`
- Domain-specific constraints:
`references/domain-web.md`
`references/domain-cli.md`
`references/domain-data.md`

## Implementation Workflow

1. Define package boundaries.
- Keep responsibilities cohesive.
- Keep dependency direction clear.
- Define interfaces at consumption points.

2. Define API contracts.
- Accept context first on request-scoped operations.
- Return explicit values and errors.
- Avoid hidden side effects.

3. Define concurrency model.
- Assign ownership for each goroutine.
- Define start/stop conditions.
- Define bounded worker and queue behavior.

4. Define cancellation and timeout model.
- Propagate context through all downstream calls.
- Enforce deadlines around external dependencies.
- Define graceful shutdown budget and behavior.

5. Define error model.
- Wrap errors with operation context.
- Preserve inspectability via `errors.Is` and `errors.As`.
- Keep user-facing and internal diagnostics distinct.

6. Implement observability.
- Emit structured logs around boundaries.
- Attach request and operation identifiers.
- Track retries and timeout outcomes.

7. Implement verification.
- Unit tests for pure logic and invariants.
- Integration tests for boundary behavior.
- Race checks for concurrency-sensitive changes.

## Debugging Workflow

1. Capture primary signal.
- Panic trace.
- Deadlock symptom.
- Race detector output.
- Latency/throughput regression.

2. Translate to design question.
- Goroutine ownership undefined.
- Channel lifecycle mismatch.
- Context propagation broken.
- Interface abstraction misplaced.
- Error taxonomy insufficient.

3. Apply coherent fix.
- Introduce lifecycle supervision for goroutines.
- Simplify channel topology and close semantics.
- Propagate context through entire call chain.
- Tighten interface scope and dependency direction.

4. Verify and harden.
- Reproduce failure before and after.
- Add regression and race-focused tests.
- Confirm behavior under cancellation and timeout.

## Linting And Type Gates

Run these checks before declaring Go work complete.

1. Formatting.
- `gofmt -w .` for touched files (or package-local scope).

2. Static analysis.
- `go vet ./...`
- If available, run `staticcheck ./...`.

3. Build/tests.
- `go test ./...`
- For concurrency-sensitive changes, include race checks: `go test -race ./...`.

4. Fix policy.
- Prefer simplifying goroutine/channel design over suppressing warnings.
- Keep lint suppressions rare and justified inline.

## Review Workflow

Use findings-first review output.

1. Severity order.
- S0: outage, corruption, security, deadlock risk.
- S1: correctness bug with user impact.
- S2: reliability/performance/maintainability risk.
- S3: style/docs.

2. Finding structure.
- Defect description.
- Runtime impact.
- Trigger path.
- Minimal remediation.
- Missing tests.

3. Go-specific review lenses.
- Context propagation completeness.
- Goroutine ownership and termination.
- Channel close and receive semantics.
- Boundedness of workers and queues.
- Error wrapping and inspectability.
- Interface granularity and package cohesion.

4. Evidence standard.
- Provide file/line references.
- Provide concrete runtime path.
- Avoid speculative claims without trigger scenario.

## Architecture Heuristics

1. Prefer small cohesive packages.
- Keep package purpose singular.
- Keep imports aligned with dependency direction.
- Avoid utility package dumping.

2. Prefer narrow interfaces.
- Define interfaces where consumed.
- Keep method sets minimal.
- Avoid premature abstraction.

3. Prefer explicit lifecycle management.
- Start goroutines with ownership clarity.
- Stop goroutines via context or close semantics.
- Confirm no leaks on shutdown.

4. Prefer deterministic boundary behavior.
- Validate input early.
- Normalize output and error forms.
- Keep side effects isolated.

## Concurrency Principles

1. Manage goroutine lifecycle explicitly.
- Assign owner.
- Define termination condition.
- Observe and surface failures.

2. Bound concurrency and queues.
- Cap worker counts.
- Cap queue depth.
- Define overload handling.

3. Make cancellation first-class.
- Pass context through all request paths.
- Honor context in downstream operations.
- Use deadlines for external dependencies.

4. Avoid common hazards.
- Unbounded goroutine spawning.
- Cyclic channel waits.
- Shared mutable state without synchronization.

## Error Strategy Principles

1. Classify errors.
- Validation errors.
- Dependency/transient errors.
- Invariant/internal errors.

2. Preserve context.
- Wrap every boundary failure with operation context.
- Preserve root cause for branching and diagnostics.

3. Design for callers.
- Support `errors.Is` and `errors.As` semantics.
- Keep sentinel usage limited and documented.
- Keep boundary-specific error mapping explicit.

4. Avoid anti-patterns.
- Ignored errors.
- String-only matching for branch logic.
- Panic for expected failures.

## Performance Workflow

1. Measure baseline.
- Latency, throughput, and memory profile.
- Lock contention and blocking hotspots.

2. Optimize in order.
- Algorithm and data structures.
- Allocation and object reuse.
- Concurrency and batching model.
- Localized micro-optimizations.

3. Verify tradeoffs.
- Preserve behavior through tests.
- Re-measure with representative workload.
- Document complexity introduced by optimization.

## Tooling and Verification Baseline

Run project-standard quality checks.

- Formatting checks.
- Vet/static checks.
- Lint checks if configured.
- Unit and integration tests.
- Race detection for concurrency-sensitive changes.

Use repository defaults before introducing new tools.

## Deliverable Format

Return outputs in this order.

1. Design summary.
- Package/interface strategy.
- Concurrency and context strategy.
- Error strategy.

2. Implementation summary.
- Files changed.
- Behavioral impact.
- Compatibility notes.

3. Risk notes.
- Remaining hazards.
- Operational assumptions.
- Monitoring expectations.

4. Verification summary.
- Commands run.
- Tests added or updated.
- Known gaps.

## Anti-Drift Rules

- Reject unbounded goroutine fan-out in service paths.
- Reject missing context propagation at boundaries.
- Reject ignored errors in non-trivial paths.
- Reject oversized interfaces without clear consumer need.
- Reject major behavior changes without regression and race coverage.

## Scenario Playbooks

Use these playbooks for frequent operational workflows.

1. Add a new service handler.
- Parse and validate request payload at boundary.
- Pass request-scoped context through service stack.
- Apply timeout on outbound dependency calls.
- Wrap errors with operation context and map to stable response.
- Add tests for success, timeout, cancellation, and dependency failure.

2. Stabilize worker pipeline.
- Define worker count and queue cap explicitly.
- Define retry policy for transient failures only.
- Define dead-letter or failure sink behavior.
- Ensure graceful shutdown drains in-flight items within budget.
- Add tests for saturation and interruption handling.

3. Refactor channel-heavy flow with deadlock risk.
- Diagram send/receive ownership and channel close points.
- Remove cyclic wait dependencies.
- Assign channel close responsibility to sender side only.
- Add timeout or cancellation path for blocking operations.
- Validate with integration and race tests.

4. Reduce interface sprawl.
- Move interfaces to consumer packages.
- Keep method sets minimal and behavior-specific.
- Return concrete implementations from constructors when possible.
- Add contract tests for interface consumers.

## Decision Tables

Use these quick choices during implementation and review.

1. Goroutine usage.
- Long-lived service task: owner + shutdown signal required.
- Short-lived parallel unit of work: bounded worker or errgroup task.
- Fire-and-forget task: avoid unless monitored and justified.

2. Channel strategy.
- Ownership handoff: channel preferred.
- Shared mutable state with low contention: synchronization primitive.
- High-contention shared mutation: redesign toward partitioning or message passing.

3. Error strategy.
- Recoverable operation error: wrap and return.
- Branching required by caller: support `errors.Is` or `errors.As`.
- Invariant breach: fail fast with diagnostic context.

4. Timeout and retry strategy.
- External dependency call: explicit timeout always.
- Transient failures: capped backoff retry.
- Validation or invariant failures: no retry path.

## Quality Gate

Before finalizing output, confirm all items.

- Context propagation is complete across request paths.
- Goroutine ownership and shutdown behavior are explicit.
- Channel close responsibilities are unambiguous.
- Errors are wrapped with operation context and remain inspectable.
- Concurrency is bounded and race-sensitive paths are tested.
- Formatting, static checks, tests, and race checks are complete.

## Additional Resources

Load detail only when needed.

- `references/concurrency-patterns.md`
- `references/core-patterns.md`
- `references/error-to-design-map.md`
- `references/review-checklist.md`
- `references/domain-web.md`
- `references/domain-cli.md`
- `references/domain-data.md`

## Final Readiness Checklist

- Confirm rollback-safe deployment behavior.
- Confirm shutdown behavior under active load.
- Confirm observability fields cover triage-critical paths.
