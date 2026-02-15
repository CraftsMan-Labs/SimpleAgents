# Workflow Security Hardening Contract

This document defines runtime, expression, and worker hardening controls added in
Phase 9.

## Expression Engine Controls

Implemented in `crates/simple-agents-workflow/src/expressions.rs`:

- `ExpressionLimits::max_expression_chars`
- `ExpressionLimits::max_operator_count`
- `ExpressionLimits::max_depth`
- `ExpressionLimits::max_path_segments`
- `ExpressionLimits::max_cache_entries`

Violations return `ExpressionError::ComplexityLimitExceeded`.

## Runtime Resource Guards

Implemented in `crates/simple-agents-workflow/src/runtime.rs` via
`RuntimeSecurityLimits`:

- `max_expression_scope_bytes` for condition/loop/filter expression scope payloads
- `max_map_items` for `map` node fan-out control
- `max_parallel_branches` for `parallel` node fan-out control
- `max_filter_items` for `filter` input cardinality control

Violations return explicit runtime errors:

- `ExpressionScopeLimitExceeded`
- `MapItemLimitExceeded`
- `ParallelBranchLimitExceeded`
- `FilterItemLimitExceeded`

## Worker Sandbox and Request Contract

Implemented in `crates/simple-agents-workflow/src/worker.rs` via
`WorkerSecurityPolicy`:

- `max_request_timeout_ms`
- `max_request_payload_bytes`
- `max_identifier_length`

Before queueing, every request is validated against this contract. Violations
return `WorkerPoolError::InvalidRequest` and are not executed.

`WorkerErrorCode::InvalidRequest` is propagated for worker protocol parity.

## Secret Handling Contract

- Do not embed plaintext credentials in workflow node definitions or static node
  inputs.
- Pass secret references (for example IDs/handles) and resolve them in trusted
  tool handlers or worker environments.
- Keep workflow traces and benchmark artifacts free of secret payloads.
- Treat `scoped_input` as potentially sensitive and enforce strict size limits to
  reduce accidental data over-exposure.

## Verification Commands

```bash
cargo test -p simple-agents-workflow expressions::tests::rejects_expression_when_depth_limit_exceeded
cargo test -p simple-agents-workflow runtime::tests::rejects_condition_when_expression_scope_exceeds_limit
cargo test -p simple-agents-workflow worker::tests::rejects_request_when_security_contract_is_violated
```
