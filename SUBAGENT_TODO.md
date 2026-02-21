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
| S5 | Main-Agent | `crates/simple-agents-workflow/src/yaml_runner.rs`, `docs/YAML_WORKFLOW_SYSTEM.md`, `examples/workflow_email/email-chat-draft-or-clarify.yaml` | Avoid hardcoded key rendering while preserving readable stream UX | Core `stream_json_as_text` flag emits non-thinking output tokens as plain text lines | completed | Added YAML flag + formatter and wired example workflow to enable it |
| C1, C2, C3, C5, C6, C7 | Main-Agent | `examples/workflow_email/**`, `bindings/go/examples/workflow_chat_history/**`, `docs/EXAMPLES.md`, `Makefile`, `crates/simple-agents-napi/**` | Cross-language examples need practical parity and low-friction run flows | Node/Go/Python chat-history examples + docs include parity flags and consistent make-based run commands across Node and Bun runtimes; Node stream callbacks deliver payloads reliably | completed | Added `run-node-chat-history` with `JS_RUNTIME=node|bun` and fixed NAPI workflow stream callback payload handling |
| P1 | SA-Parity-Matrix | `docs/**`, `parity-fixtures/**`, binding API surfaces (`*.pyi`, `index.d.ts`, `bindings/go/simpleagents.go`) | All implementation and tests need one explicit parity target | Versioned parity matrix + prioritized gap list (P0/P1) with acceptance criteria | pending | Parent: `TODO.md` P1 |
| P2 | SA-FFI-Workflow-Events | `crates/simple-agents-ffi/**`, `crates/simple-agents-ffi/include/simple_agents.h` | Go cannot reach workflow event parity unless FFI exposes event/event-stream surfaces | Backward-compatible FFI functions + callback/event JSON contracts for workflow events | completed | Parent: `TODO.md` P2 |
| P3 | SA-Node-Workflow-Parity | `crates/simple-agents-napi/**` | Node must match Python workflow event capabilities | Node APIs for include-events + workflow event streaming, with typed TS declarations | completed | Parent: `TODO.md` P3 |
| P4 | SA-Go-Workflow-Parity | `bindings/go/**` (excluding docs/examples) | Go must match Python workflow event capabilities | Go client methods + event structs/channels for workflow include-events + streaming | completed | Parent: `TODO.md` P4 |
| P5 | SA-Output-Contract-Parity | `bindings/go/simpleagents.go`, `crates/simple-agents-napi/index.d.ts`, tests for output shape | Contract drift across bindings causes feature loss and confusion | Consistent workflow output fields (metrics/tokens/tps/events) across Node/Go docs/types | completed | Parent: `TODO.md` P5 |
| P6 | SA-Examples-Parity | `examples/workflow_email/node/**`, `bindings/go/examples/workflow_chat_history/**`, `examples/workflow_email/**README.md` | Users need runnable proof of parity for interactive chat-history workflows | Node/Go runners expose Python-equivalent flags and behavior for events/stream/thinking | completed | Parent: `TODO.md` P6 |
| P7 | SA-Parity-Tests | `parity-fixtures/**`, `bindings/go/*test.go`, `crates/simple-agents-napi/test/**`, `crates/simple-agents-py/tests/**` | Parity must be enforced continuously, not one-off | Contract fixtures and tests fail on cross-binding drift for workflow event APIs and payloads | completed | Parent: `TODO.md` P7 |
| P8 | Main-Agent | Integration verification + docs sync + readiness notes | Final gate to ensure all subagent outputs integrate cleanly | Verification report with required make targets and updated docs references | completed | Parent: `TODO.md` P8 |
| D1 | Main-Agent | `examples/workflow_email/email-chat-draft-or-clarify.yaml`, `bindings/go/examples/workflow_chat_history/main.go` | Same prompt produced divergent Go vs Node routing/output shape in chat-history run, causing parity confusion | Deterministic routing behavior for shared YAML prompt path and aligned Go `--show-thinking` stream rendering semantics with Node runner | completed | Parent: `TODO.md` D1 |
| D2 | Main-Agent | `crates/simple-agents-workflow/src/yaml_runner.rs`, `examples/workflow_email/run_with_chat_history.py`, `bindings/go/examples/workflow_chat_history/main.go`, `examples/workflow_email/node/run_with_chat_history.js`, `bindings/go/simpleagents.go` | Users require token-level stream parity metadata and fallback behavior across Python/Go/Node | Stream tokens include stable IDs and step/kind/terminal metadata across runners; Node/Go fallback to normal deltas when raw-thinking stream is absent | completed | Parent: `TODO.md` D2 |

## Coordination checklist

- Define each subagent scope so no two subagents own overlapping implementation areas.
- Ensure each subagent assignment references the corresponding parent task in `TODO.md`.
- Provide each subagent with clear instructions: goal, approach, constraints, verification, and expected return format.
- Specify required skill usage whenever relevant.
- Review outputs for completeness and mergeability before integration.
