# Domain Constraints: Fintech / Trading / Payments

Use stricter defaults for correctness, auditability, and deterministic behavior.

## Core Constraints

- Precision correctness is mandatory (no float money math).
- Every state transition should be auditable.
- Retry logic must be idempotent-aware.
- Failure handling must separate transient from permanent errors.

## Recommended Patterns

- Represent money/quantities with decimal/fixed-point newtypes.
- Use typed state transitions (typestate or validated transition methods).
- Record immutable event logs for key operations.
- Attach correlation IDs to all external operations.

## Concurrency and Consistency

- Use explicit ordering/locking strategy for account/position updates.
- Avoid hidden shared mutable state in critical paths.
- Define timeout and retry budgets from business SLAs.

## Common Mistakes

- Using `f32`/`f64` for currency.
- Non-idempotent retries for payment submission.
- Missing audit fields in error paths.
- Ambiguous error categories for operational handling.
