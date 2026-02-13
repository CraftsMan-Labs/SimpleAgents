# Error-to-Design Map (Python)

## Runtime and Logic Failures

- Frequent `AttributeError`/`KeyError`: unclear data contracts -> define typed models.
- Frequent `NoneType` errors: nullable boundaries unclear -> explicit Optional handling.
- Repeated broad exception catches: missing error taxonomy -> introduce domain exceptions.

## Async Failures

- “Task exception was never retrieved”: unstructured task lifecycle -> use TaskGroup or explicit joins.
- Event loop blocked warnings: sync calls in async path -> isolate via thread/process executors.
- Cancellation bugs: swallowed `CancelledError` -> cleanup then re-raise.

## Type Checker Failures

- Persistent `Any` spread: weak boundary typing -> type external interfaces first.
- Complex union narrowing failures: overloaded responsibilities -> split functions and types.
