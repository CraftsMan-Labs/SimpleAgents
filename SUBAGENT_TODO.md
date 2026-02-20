# SUBAGENT TODO

Track subagent assignments for large tasks. Keep scopes non-overlapping and update statuses continuously.
Every subagent item must map to a parent item in `TODO.md`.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Subagent assignments

| Parent TODO | Subagent | Scope | Why | Expected Outcome | Status | Notes |
|---|---|---|---|---|---|---|
| T1, T3 | SA-Contract-Config | `crates/simple-agent-type/**`, `crates/simple-agents-workflow/src/observability/**` | Contract/config must be finalized before runtime and binding wiring | Typed telemetry config + trace context contract + payload mode policy surface | completed | Implemented in workflow options contract + payload mode defaults (full payload, toggle-ready redaction) |
| T2, T4 | SA-Workflow-Runtime | `crates/simple-agents-workflow/src/yaml_runner.rs`, `crates/simple-agents-workflow/src/lib.rs` | Source-of-truth runtime spans and output contract live here | Runtime emits required spans; output includes top-level + metadata trace IDs | completed | Added workflow/node/handler span hooks and output `trace_id` + `metadata.telemetry.trace_id` |
| T5 | SA-Bindings-FFI-Context | `crates/simple-agents-ffi/**`, `bindings/go/**`, `crates/simple-agents-napi/**`, `crates/simple-agents-py/**` | External API layers use bindings/FFI; context propagation must be consistent | Backward-compatible context/options methods across all surfaces | completed | Added FFI `sa_run_workflow_yaml_with_options` and Go/Node/Python options APIs |
| T6 | SA-Tests-Validation | tests touching workflow + bindings contract files only | Ensure trace propagation and mode toggles are proven by tests | Test coverage for parent-child traces, payload modes, mandatory handler spans, output IDs | completed | Added workflow tests for trace ID propagation and payload mode + Go binding tests for options API |
| T7 | SA-Docs-Architecture | `docs/**` | Team needs clear rollout/ops and data-flow reference | Architecture/data-flow + retention/config + Jaeger/PostHog correlation docs | completed | Added `docs/TRACING_ARCHITECTURE.md` and updated workflow/docs map entries |
| T8 | Main-Agent | Integration verification and final readiness report | Final quality gate after subagents complete | Consolidated pass/fail report and next actions | completed | Verification commands executed and recorded in `TODO.md` |
| S1, S2, S3 | Main-Agent | `crates/simple-agents-workflow/src/yaml_runner.rs`, `examples/workflow_email/run_with_chat_history.py`, `docs/BINDINGS_PYTHON.md` | Streaming bug fix is scoped and does not require parallel ownership | Core emits sanitized stream deltas; example output clean; docs updated | completed | Implemented core JSON delta filter and removed stale example fallback messaging |
| S4 | Main-Agent | `crates/simple-agents-workflow/src/yaml_runner.rs`, `docs/BINDINGS_PYTHON.md` | Stream consumers need token-level attribution in core events | Per-token event attributes identify step, token kind, and terminal-node ownership | completed | Added `step_id`, `token_kind`, and `is_terminal_node_token` to workflow stream events |

## Coordination checklist

- Define each subagent scope so no two subagents own overlapping implementation areas.
- Ensure each subagent assignment references the corresponding parent task in `TODO.md`.
- Provide each subagent with clear instructions: goal, approach, constraints, verification, and expected return format.
- Specify required skill usage whenever relevant.
- Review outputs for completeness and mergeability before integration.
