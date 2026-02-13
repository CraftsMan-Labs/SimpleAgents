# Go Review Checklist

## Severity

- `S0`: outage/security/data corruption risk.
- `S1`: correctness bug.
- `S2`: reliability/performance/maintainability risk.
- `S3`: style/docs.

## Findings-First Format

1. List findings by severity with file/line.
2. State impact and reproduction conditions.
3. Provide minimal remediation.
4. Then list testing gaps.

## Checks

- Context is propagated and honored.
- Goroutines are bounded and cancellable.
- Errors are wrapped with actionable context.
- Shared state synchronization is explicit and race-safe.
- Critical paths have tests (and race tests where needed).
