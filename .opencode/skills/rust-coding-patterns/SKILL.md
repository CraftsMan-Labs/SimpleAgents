---
name: rust-coding-patterns
description: This skill should be used when the user asks to "implement Rust code", "debug Rust compiler errors", "refactor Rust for performance", "review Rust code", "fix ownership and borrowing issues", "improve Tokio async patterns", or "apply Rust best practices".
---

# Rust Coding Patterns

Deliver production-grade Rust by choosing explicit tradeoffs for ownership, concurrency, error strategy, performance, and API design. Use this guide to drive implementation, debugging, refactoring, and review workflows from first principles instead of patch-level fixes.

## Purpose

Build Rust solutions that are idiomatic, safe, observable, and maintainable under real workloads. Prevent repeated low-value loops around borrow checker errors, async deadlocks, ad hoc retries, and clone-heavy design drift. Encode constraints in type signatures, module boundaries, and test design so behavior remains robust as code evolves.

## Operating Mode

Select one mode at the start of each task.

1. `implement`
- Define constraints first.
- Design API and ownership shape.
- Implement with explicit error and concurrency strategy.

2. `debug`
- Collect failure signal: compiler code, panic, wrong output, perf regression.
- Map symptom to design cause before patching.
- Apply smallest coherent design fix, then verify.

3. `review`
- Report findings first, ordered by severity and behavioral risk.
- Prove risk with concrete paths and line-level evidence.
- Recommend minimal safe remediation and tests.

4. `refactor`
- Preserve external behavior.
- Improve model clarity, invariants, and runtime safety.
- Add tests to lock intended behavior.

## Entry Checklist

Complete this checklist before writing or changing code.

1. Identify workload class.
- CPU-bound.
- I/O-bound.
- Mixed.

2. Identify correctness constraints.
- Invariants that must never break.
- State transitions that must be legal by construction.
- Failure modes that callers must handle.

3. Identify operational constraints.
- Latency and throughput targets.
- Resource limits (memory, connections, queue depth).
- Shutdown and cancellation guarantees.

4. Identify compatibility constraints.
- Public API stability.
- Data format compatibility.
- Migration impact.

## Reference Loading Map

Load only the references needed by the task.

- Async runtime, task orchestration, channels, shutdown:
`references/async-tokio.md`
- Ownership, mutability, type-driven design, error strategy, dispatch, performance:
`references/core-patterns.md`
- Compiler/runtime symptom-to-design mapping:
`references/error-to-design-map.md`
- Findings-first review rubric and severity:
`references/review-checklist.md`
- Domain constraints:
`references/domain-web.md`
`references/domain-cli.md`
`references/domain-fintech.md`
`references/domain-cloud-native.md`

## Implementation Workflow

Follow this sequence unless task constraints require a deviation.

1. Define boundary contract.
- Write public function signatures first.
- Prefer borrowed parameters for read-only inputs.
- Return owned values when lifetime escape is required.
- Encode fallibility in return type rather than hidden panics.

2. Choose ownership model.
- Use owned fields for aggregate roots.
- Use borrowing for transient read paths.
- Use shared pointers only when ownership is truly shared.
- Use `Weak` only when breaking cycles is intentional.

3. Choose mutability model.
- Prefer immutable-by-default data flow.
- Scope `&mut` borrows tightly.
- Use interior mutability only with clear justification.
- In concurrent contexts, choose lock primitive by read/write profile.

4. Choose concurrency model.
- CPU-bound: threads/rayon.
- I/O-bound: Tokio async.
- Mixed: async orchestration + bounded blocking isolation.
- Bound fan-out and queue depth before first benchmark.

5. Choose error model.
- Library boundary: typed errors.
- Application boundary: contextual aggregation.
- Distinguish retryable, non-retryable, and invariant failures.
- Avoid panic-based control flow.

6. Add observability.
- Instrument boundary operations.
- Add stable identifiers to spans/logs.
- Expose enough context to debug concurrency and retries.

7. Add verification.
- Unit tests for pure logic and invariants.
- Integration tests for boundary behavior.
- Concurrency tests for cancellation/shutdown and boundedness.
- Regression tests for prior failures.

## Debugging Workflow

Use design-trace debugging, not local symptom suppression.

1. Parse error signal.
- Compiler code (`E0382`, `E0502`, `E0277`, and others).
- Runtime panic path.
- Deadlock/stall symptom.
- Perf hotspot.

2. Map to root design question.
- Ownership unclear.
- Borrow scope too broad.
- Trait abstraction mismatch.
- Async lifecycle unsupervised.
- Error taxonomy incomplete.

3. Apply coherent fix.
- Move boundary, not only statement order, when model is wrong.
- Replace clone workaround with data-flow redesign.
- Replace unbounded spawn with bounded orchestration.
- Replace ad hoc retries with policy-based wrapper.

4. Verify behavior.
- Reproduce pre-fix failure.
- Confirm expected post-fix behavior.
- Add regression test.

## Linting And Type Gates

Run these checks before declaring Rust work complete.

1. Formatting.
- `cargo fmt --all --check`

2. Linting.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`

3. Build/tests.
- `cargo check --workspace`
- Run impacted tests (and wider tests for risky changes).

4. Fix policy.
- Do not silence lints without clear, documented justification.
- Prefer API/design fixes over local allow attributes.

## Review Workflow

When reviewing code, prioritize correctness and risk.

1. Severity order.
- S0: outage, security, UB, corruption, deadlock.
- S1: user-visible wrong behavior.
- S2: reliability/perf/maintainability risk.
- S3: readability/style/docs.

2. Finding structure.
- Problem statement.
- Runtime impact.
- Trigger condition.
- Minimal remediation.
- Test coverage gap.

3. Rust-specific review lenses.
- Ownership transfer correctness.
- Borrow scope and aliasing assumptions.
- Lock scope and await boundaries.
- Send/Sync assumptions around task spawning.
- Error type semantics and context preservation.
- Hidden allocations/clones in hot paths.

4. Evidence standard.
- Provide file/line references.
- Use concrete control-flow path.
- Avoid speculative claims without traceability.

## Architecture Heuristics

Use these heuristics to keep Rust code resilient.

1. Prefer capability-oriented APIs.
- Expose operations by trait or method sets that express allowed actions.
- Reduce mutable surface area.

2. Encode invariants in types.
- Newtypes for semantic IDs and constrained values.
- Typestate for valid transitions.
- Private fields plus validated constructors.

3. Keep boundaries explicit.
- Domain layer independent from transport and storage details.
- Conversion at boundaries, not deep in core logic.
- Avoid leaking low-level errors as public domain errors.

4. Keep abstractions honest.
- Use static dispatch where performance matters and type space is known.
- Use dynamic dispatch for open plugin-like extension surfaces.
- Use enum for closed variant sets.

## Async and Concurrency Principles

Follow these rules for Tokio-based systems.

1. Supervise task lifecycles.
- Track all spawned tasks.
- Handle join errors explicitly.
- Ensure shutdown path reaches every long-lived task.

2. Enforce bounded concurrency.
- Set explicit concurrency limits.
- Avoid unbounded queue growth.
- Propagate backpressure signals.

3. Make cancellation cooperative.
- Check cancellation tokens in task loops.
- Keep cleanup paths deterministic and short.
- Define timeout budget for graceful shutdown.

4. Protect event loop health.
- Avoid blocking calls on runtime workers.
- Isolate blocking work with explicit boundary.
- Avoid long-held locks in async paths.

## Error Strategy Principles

Standardize error behavior across modules.

1. Classify errors.
- Domain validation errors.
- Dependency/transient errors.
- Invariant violations.

2. Preserve context.
- Add operation context near fallible boundaries.
- Keep root causes available for observability.

3. Design for callers.
- Return actionable categories.
- Keep user-facing messages stable and clear.
- Reserve internal detail for logs and telemetry.

4. Avoid anti-patterns.
- Broad panic usage.
- String-only error matching.
- Silent error swallowing.

## Performance Workflow

Optimize only after measuring, then prioritize by impact.

1. Measure baseline.
- Capture latency, throughput, memory, and allocation profile.

2. Optimize in order.
- Algorithm.
- Data layout.
- Allocation pressure.
- Concurrency strategy.

3. Verify correctness and regressions.
- Preserve semantics through tests.
- Re-measure under representative workload.
- Document tradeoffs introduced by optimization.

## Quality Gate

Before finalizing output, confirm all items.

- Public APIs express ownership and fallibility clearly.
- Invariants are enforced by type or constructor.
- Async paths are bounded, cancellable, and supervised.
- Lock guards are not held across await points.
- Error strategy is explicit at each boundary.
- Tests cover success, failure, edge, and shutdown behavior.
- Formatting, linting, and test suite complete.

## Deliverable Format

Return deliverables in this order.

1. Design summary.
- Ownership model.
- Concurrency model.
- Error strategy.
- Key tradeoffs.

2. Implementation summary.
- Files changed.
- Behavioral impact.
- Backward compatibility notes.

3. Risk notes.
- Remaining hazards.
- Operational assumptions.
- Monitoring hooks.

4. Verification summary.
- Commands run.
- Tests added/updated.
- Known gaps.

## Anti-Drift Rules

Apply these rules during maintenance.

- Reject clone-first patches that hide ownership issues.
- Reject unbounded spawn/queue patterns in service code.
- Reject panic-based handling for expected failures.
- Reject broad abstractions without runtime or API justification.
- Reject major behavior changes without regression coverage.

## Scenario Playbooks

Use these playbooks to accelerate common tasks.

1. Add a new async service endpoint.
- Define typed request and response models.
- Validate and convert inputs at boundary.
- Select bounded concurrency for downstream fan-out.
- Define timeout and retry policy per dependency.
- Add tracing fields for request ID and dependency timing.
- Add tests for success, timeout, cancellation, and dependency failure.

2. Resolve repeated ownership compiler errors in a module.
- Stop local patching with ad hoc clone insertion.
- Draw ownership map for key values across function boundaries.
- Decide where ownership transfer is intended.
- Convert read-only parameters to borrows.
- Introduce owned return types where references escape scope.
- Re-run compiler and add regression tests around changed flow.

3. Refactor lock-heavy async flow.
- Identify lock acquisition sites and await boundaries.
- Reduce critical section scope.
- Move expensive I/O work outside lock scope.
- Replace shared state with channel handoff where feasible.
- Add stress tests and shutdown tests.

4. Improve performance in a hot request path.
- Capture baseline profile and allocation metrics.
- Remove avoidable clones and intermediate allocations.
- Revisit data structure fit and access pattern locality.
- Evaluate static vs dynamic dispatch cost in hot loops.
- Re-measure and document tradeoffs.

## Decision Tables

Use these quick choices during implementation.

1. Expected absence vs failure.
- Absence is normal lookup outcome: model as optional path.
- Caller must branch by failure type: return typed error category.
- Failure indicates invariant break: fail fast and surface context.

2. Shared state vs message passing.
- Multiple actors mutate shared value with contention risk: prefer message passing.
- Shared immutable snapshot across tasks: shared immutable ownership.
- Rare writes with many reads: reader-optimized synchronization.

3. Static vs dynamic dispatch.
- Hot path with known type set: static dispatch preferred.
- Runtime-selected behavior with open extension set: dynamic dispatch preferred.
- Closed variant set requiring exhaustive handling: enum-based design preferred.

4. Retry decisions.
- Retry only transient dependency failures.
- Avoid retry on validation and invariant failures.
- Enforce cap, timeout budget, and jittered backoff.

## Additional Resources

Load detailed guides when needed.

- `references/async-tokio.md`
- `references/core-patterns.md`
- `references/error-to-design-map.md`
- `references/review-checklist.md`
- `references/domain-web.md`
- `references/domain-cli.md`
- `references/domain-fintech.md`
- `references/domain-cloud-native.md`
