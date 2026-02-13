# Rust Review Checklist

## Severity Model

- `S0 Critical`: data loss, security exposure, UB risk, or deadlock/high-probability outage.
- `S1 High`: correctness bug with user-visible wrong behavior.
- `S2 Medium`: maintainability/performance/reliability risk likely to regress.
- `S3 Low`: style/documentation/readability improvements.

## Findings-First Output

1. List findings by severity (`S0` to `S3`).
2. Include exact file path and line reference per finding.
3. State impact and expected runtime behavior.
4. Provide minimal actionable fix.
5. After findings, list testing gaps.

## Correctness and Safety

- Ownership/borrow changes preserve runtime behavior.
- No hidden panic paths in production control flow.
- Unsafe blocks include safety invariants and are justified.
- Error mapping preserves actionable information.

## Async and Concurrency

- No lock guards held across `.await`.
- `Send`/`Sync` assumptions match spawn/executor model.
- Concurrency is bounded (no unbounded fan-out by default).
- Cancellation and shutdown paths are coherent and testable.

## API and Type Design

- Borrow where possible; own where lifecycle requires.
- Newtypes/type constraints enforce key invariants.
- Trait abstraction choice matches performance/flexibility goals.
- Public API surfaces are minimal and stable.

## Performance and Resource Use

- No obvious hot-path allocations/clones.
- Data structures match read/write/access patterns.
- Blocking work isolated from async runtime workers.
- Complexity regressions are identified and justified.

## Tests and Verification

- New behavior has direct tests.
- Edge/failure paths are covered.
- Concurrency-sensitive code has deterministic checks where possible.
- Integration boundaries (network/DB/FFI) have representative tests.

## Review Red Flags

- “Fix” is adding `.clone()` without ownership reasoning.
- `unwrap()` introduced outside tests/proven invariants.
- `Arc<Mutex<T>>` used as default without contention strategy.
- Major logic change with no added tests.
