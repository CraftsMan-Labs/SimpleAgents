# Domain Constraints: Go Web/Microservices

- Request-scoped context propagation is mandatory.
- Timeouts required for all outbound calls.
- Structured logs with request correlation IDs.
- Graceful shutdown drains in-flight requests.
- Bound fan-out to downstream dependencies.
