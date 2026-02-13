# Asyncio Patterns

## Table of Contents

1. Runtime and Task Lifecycle
2. Structured Concurrency
3. Concurrency Limits and Backpressure
4. Cancellation and Shutdown
5. Async Error Handling
6. Shared State Patterns
7. Async Anti-Patterns

## 1. Runtime and Task Lifecycle

- Use `asyncio.run()` as top-level entrypoint.
- Keep async boundaries explicit; avoid hidden event-loop management.

## 2. Structured Concurrency

- Prefer `asyncio.TaskGroup` (3.11+) for sibling task orchestration.
- Treat orphan tasks as bugs unless explicitly detached.

## 3. Concurrency Limits and Backpressure

- Use `asyncio.Semaphore` for bounded fan-out.
- Use `asyncio.Queue(maxsize=...)` for producer/consumer backpressure.

## 4. Cancellation and Shutdown

- Treat cancellation as normal control flow.
- Catch `asyncio.CancelledError` only to clean up, then re-raise.
- Add timeout budgets with `asyncio.timeout()`.

## 5. Async Error Handling

- Aggregate task failures intentionally (`ExceptionGroup` in 3.11+).
- Classify retryable vs permanent errors.
- Keep retries bounded with timeout + jitter/backoff.

## 6. Shared State Patterns

- Prefer message passing over shared mutable state.
- If required, guard state with `asyncio.Lock` and tight critical sections.

## 7. Async Anti-Patterns

- Blocking calls in async paths (`time.sleep`, sync DB/http clients).
- Unbounded `create_task` loops.
- Swallowing cancellation or task exceptions.
