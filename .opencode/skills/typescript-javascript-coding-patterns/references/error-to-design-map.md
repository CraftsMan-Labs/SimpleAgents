# Error-to-Design Map (JS/TS)

## Type and Contract Failures

- Frequent type assertions (`as Foo`) to force code to compile -> weak model boundaries; introduce runtime validation and tighter types.
- Repeated `undefined` access failures -> optionality unmanaged; explicit narrowing and defaults needed.

## Async Failures

- Unhandled promise rejections -> unsupervised async tasks; centralize task orchestration.
- Timeout and retry chaos -> no unified policy; create shared resilience utilities.

## Runtime Bugs

- “Works in dev, fails in prod” serialization/input issues -> missing boundary validation.
- Memory growth under load -> unbounded concurrency/queues; enforce limits and backpressure.
