# Workflow Security Hardening Contract

This guide defines workflow runtime hardening controls for expressions, fan-out limits, worker request contracts, and secret handling.
By the end, you should know which limits are enforced, what errors they raise, and how to validate hardening behavior with targeted tests.

## Prerequisites

- Familiarity with workflow runtime concepts in [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM)
- Ability to run Rust tests in the workspace

## Quick Path

1. Review expression complexity limits.
2. Review runtime fan-out and payload scope limits.
3. Review worker security policy validation.
4. Run targeted verification tests.
5. Confirm secret handling rules in your workflow patterns.

## Security Control Matrix

| Control area | Location | Limits / policy |
|---|---|---|
| Expression engine | `crates/simple-agents-workflow/src/expressions.rs` | `max_expression_chars`, `max_operator_count`, `max_depth`, `max_path_segments`, `max_cache_entries` |
| Runtime resource guards | `crates/simple-agents-workflow/src/runtime.rs` | `max_expression_scope_bytes`, `max_map_items`, `max_parallel_branches`, `max_filter_items` |
| Worker request contract | `crates/simple-agents-workflow/src/worker.rs` | `max_request_timeout_ms`, `max_request_payload_bytes`, `max_identifier_length` |
| YAML file load guardrails | `crates/simple-agents-workflow/src/yaml_runner.rs` | canonical path check, regular-file check, `.yaml/.yml` extension allowlist, file-size and depth bounds |
| Sensitive provider serialization | `crates/simple-agent-type/src/config.rs` | `ProviderConfig.api_key` serialization is redacted |

## Expression Engine Controls

Expression complexity violations return:

- `ExpressionError::ComplexityLimitExceeded`

Use these limits to prevent expensive or adversarial expressions from exhausting runtime resources.

## Runtime Resource Guards

`RuntimeSecurityLimits` enforces bounded execution on high-fanout and large-scope operations.

Explicit runtime errors:

- `ExpressionScopeLimitExceeded`
- `MapItemLimitExceeded`
- `ParallelBranchLimitExceeded`
- `FilterItemLimitExceeded`

Treat these as policy failures, not transient retries.

## Worker Sandbox and Request Contract

Every worker request is validated before queueing. Violations are rejected and not executed.

- Worker-side error surface: `WorkerPoolError::InvalidRequest`
- Protocol-level parity code: `WorkerErrorCode::InvalidRequest`

This protects runtime worker pools from oversized payloads and malformed identifiers/timeouts.

## Secret Handling Rules

- Do not embed plaintext credentials in workflow node definitions.
- Pass secret references (IDs/handles) and resolve in trusted handlers/workers.
- Keep traces and benchmark artifacts free of secret payloads.
- Treat `scoped_input` as sensitive and enforce strict size boundaries.

## Verification Commands

```bash
cargo test -p simple-agents-workflow expressions::tests::rejects_expression_when_depth_limit_exceeded
cargo test -p simple-agents-workflow runtime::tests::rejects_condition_when_expression_scope_exceeds_limit
cargo test -p simple-agents-workflow worker::tests::rejects_request_when_security_contract_is_violated
```

## Troubleshooting

### Security errors appear after adding workflow features

Check whether new expressions, map fan-out, or worker payload shape exceeded configured limits.

### Worker requests rejected unexpectedly

Validate timeout, payload byte size, and identifier lengths against `WorkerSecurityPolicy` limits.

### Limits feel too strict for workload

Tune limits deliberately and re-run security-focused tests; avoid removing limits without replacement protections.

## Next Steps

- Connect guardrails to runtime design in [YAML Workflow System Guide](/YAML_WORKFLOW_SYSTEM).
- Use [Workflow Debugging UX](/WORKFLOW_DEBUGGING) to inspect failures caused by policy limits.
- Validate performance impact in [Workflow Performance](/WORKFLOW_PERFORMANCE).
