# Domain Constraints: Cloud-Native / gRPC / Microservices

Apply for distributed systems with service boundaries and observability requirements.

## Core Constraints

- All remote boundaries need timeout, retry, and circuit-breaker thinking.
- Structured tracing and metrics are mandatory for operability.
- Graceful shutdown must drain in-flight work within budget.
- Backpressure must be explicit at queue and task boundaries.

## Recommended Patterns

- Define clear transport/domain error conversion boundaries.
- Use bounded channels and bounded task concurrency.
- Add health/readiness semantics tied to dependency state.
- Treat config updates as controlled state transitions.

## Common Mistakes

- Infinite retries without jitter/backoff limits.
- Unbounded queues causing memory blowups.
- Logging without trace correlation.
- Abrupt shutdown that drops in-flight work silently.
