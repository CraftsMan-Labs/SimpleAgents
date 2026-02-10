# Go Concurrency Patterns

## 1. Context First

- Pass `context.Context` as first param for request-scoped operations.
- Honor cancellation/deadlines in all I/O boundaries.
- Never store context in struct fields.

## 2. Goroutine Lifecycle

- Every goroutine needs ownership and shutdown path.
- Use `errgroup` for coordinated worker lifecycles.
- Avoid fire-and-forget goroutines in server code.

## 3. Channel Patterns

- Use channels for ownership transfer and coordination.
- Close channels from sender side only.
- Use bounded worker pools for backpressure.

## 4. Timeouts and Retries

- Apply deadlines with context + transport timeouts.
- Retry only transient failures with bounded attempts and backoff.

## 5. Common Hazards

- Goroutine leaks from blocked sends/receives.
- Deadlocks from cyclic channel dependencies.
- Shared mutable state races without synchronization.
