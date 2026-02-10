# Async Patterns (JS/TS)

## 1. Concurrency Control

- Use `Promise.all` only when full fan-out is safe.
- Use bounded concurrency (`p-limit`, pools, queues) for external calls.
- Keep queue depth and retry budgets explicit.

## 2. Cancellation and Timeouts

- Use `AbortController` for cancelable operations.
- Tie every network/long-running operation to timeout policy.
- Propagate abort signals through layers.

## 3. Error Handling in Async Flows

- Do not ignore rejected promises.
- In services, map low-level errors to stable domain/application errors.
- Treat retry as policy-driven, not ad hoc loops.

## 4. Event Loop Health

- Avoid CPU-heavy synchronous work on the main event loop.
- Move CPU-heavy work to workers/child processes where needed.
- Avoid long microtask chains that starve I/O.

## 5. Anti-Patterns

- Fire-and-forget promises without supervision.
- Nested `then` chains when `async/await` is clearer.
- Infinite retry loops without backoff/jitter/cap.
