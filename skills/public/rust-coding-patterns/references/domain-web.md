# Domain Constraints: Web/API Services

Apply these constraints for `axum`, `actix-web`, and HTTP microservices.

## Core Constraints

- Handlers must be cancellation-safe and timeout-aware.
- Shared app state should be thread-safe (`Arc`-based).
- Error responses must be explicit and stable (status + machine-readable body).
- Request paths should emit structured tracing with request IDs.

## Recommended Patterns

- State: `Arc<AppState>` with internal sync primitives only where needed.
- Validation: parse/validate at boundary; pass typed domain values inward.
- Errors: map domain errors to HTTP status codes via dedicated conversion layer.
- Concurrency: bounded fan-out for downstream calls.

## Common Mistakes

- Using non-`Send` state in handlers.
- Holding lock guards while awaiting downstream services.
- Returning opaque 500 errors for domain failures.
- Missing request-scoped correlation fields in logs.
