# Python Review Checklist

## Severity

- `S0`: data loss/security/major reliability issue.
- `S1`: correctness bug with user impact.
- `S2`: maintainability/perf/reliability risk.
- `S3`: style/docs.

## Findings-First Output

1. List findings by severity with file and line.
2. Describe runtime impact.
3. Provide minimal fix direction.
4. Then list testing gaps.

## Checks

- Boundary typing and model validation are explicit.
- Exceptions are typed/intentional and not silently swallowed.
- Async code is bounded and cancellation-safe.
- Critical paths have tests for failure and edge cases.
