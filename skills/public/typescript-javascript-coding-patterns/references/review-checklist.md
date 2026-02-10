# JS/TS Review Checklist

## Severity

- `S0`: security/data-loss/outage risk.
- `S1`: correctness bug.
- `S2`: maintainability/performance risk.
- `S3`: style/docs.

## Findings-First Format

1. List findings by severity with file/line.
2. Explain impact and trigger conditions.
3. Recommend minimal fix.
4. Then note missing tests.

## Checks

- Input boundaries validated (not trusted by default).
- Async paths bounded, cancellable, and error-aware.
- Type contracts are explicit and not bypassed with unsafe assertions.
- Tests cover unhappy paths and concurrency/resource edge cases.
