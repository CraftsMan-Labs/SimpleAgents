# Domain Constraints: Python Web/API

- Keep request handlers thin; delegate to services/use-cases.
- Validate input at boundary and convert to typed domain objects.
- Ensure DB/network calls are timeout-bound.
- Emit structured logs with request IDs.
- Return stable, explicit error payloads.
