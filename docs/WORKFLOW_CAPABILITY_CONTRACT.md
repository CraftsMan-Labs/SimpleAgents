# Workflow Capability Contract (Phase 0)

Status: Drafted and published for Phase 0 foundations.

Scope: Defines additive workflow subsystem boundaries, canonical IR v0, deterministic invariants, validation guarantees, and runtime capability contracts.

## 1) Non-Breaking Scope

- The workflow subsystem is additive and does not replace existing `SimpleAgentsClient` completion/streaming APIs.
- Existing crates and bindings continue to function without workflow adoption.
- Workflow features are introduced via new crate boundaries and typed interfaces.

## 2) Workspace Crate Boundaries

- New crate: `crates/simple-agents-workflow`
- In-scope modules:
  - `ir` - canonical workflow IR types
  - `validation` - normalization and structural diagnostics
  - `runtime` - deterministic execution engine for v0 nodes
  - `trace` - stable trace event schema
  - `recorder` - in-memory trace recorder
  - `replay` - structural replay validation
- Integration boundary:
  - LLM execution is delegated through `LlmExecutor`; production adapter is `impl LlmExecutor for SimpleAgentsClient`.
  - Tool execution is host-injected via `ToolExecutor`.

## 3) Canonical IR v0 Contract

Canonical node taxonomy (locked for v0):

- `start`
- `llm`
- `tool`
- `condition`
- `end`

IR guarantees:

- Version is `v0` (`WORKFLOW_IR_V0`).
- Workflows normalize deterministically (trimmed strings, node sorting by id).
- Nodes have stable `id` and typed node payload (`NodeKind`).

Reference: `crates/simple-agents-workflow/src/ir.rs`.

## 4) Validation and Diagnostic Contract

Validation entrypoint:

- `validate_and_normalize(&WorkflowDefinition) -> Result<WorkflowDefinition, ValidationErrors>`

Guaranteed structural checks:

- unsupported IR version
- empty workflow name or empty workflow
- empty/duplicate node ids
- missing start / multiple start
- missing end
- unknown edge targets
- unreachable nodes
- no path from start to any end
- required field emptiness per node type

Diagnostics are typed with stable codes (`DiagnosticCode`) and severity (`Severity::Error`).

Reference: `crates/simple-agents-workflow/src/validation.rs`.

## 5) Deterministic Execution Invariants

Runtime invariants for v0 executor:

- deterministic normalized workflow is used for execution
- single active node cursor, step-indexed event stream
- bounded execution via `max_steps`
- cooperative cancellation checks before step and between retry attempts
- runtime-owned node policies for retries/timeouts (`NodeExecutionPolicy`)
- trace sequence is monotonic and deterministic within one run

Reference: `crates/simple-agents-workflow/src/runtime.rs`.

## 6) Trace and Replay Contract

Trace schema types:

- `WorkflowTrace`
- `WorkflowTraceMetadata`
- `TraceEvent` with monotonic `seq`
- `TraceEventKind`: `node_enter`, `node_exit`, `node_error`, `terminal`
- `TraceTerminalStatus`: `completed | failed`

Replay contract:

- `replay_trace(&WorkflowTrace) -> Result<ReplayReport, ReplayError>` validates:
  - monotonic sequence
  - node lifecycle consistency (enter/exit/error pairing)
  - terminal event presence
  - no unclosed node lifecycle at end

References:

- `crates/simple-agents-workflow/src/trace.rs`
- `crates/simple-agents-workflow/src/recorder.rs`
- `crates/simple-agents-workflow/src/replay.rs`

## 7) Capability Contract (Existing vs New)

| Capability | Existing core/runtime | New workflow subsystem |
|---|---|---|
| Provider execution | `SimpleAgentsClient::complete` and routing stack | delegated via `LlmExecutor` adapter to core |
| Tool execution | app/tool-specific handling outside workflow engine | host-injected `ToolExecutor` |
| Validation diagnostics | request validation in existing types | workflow graph validation + diagnostic codes |
| Deterministic run model | not workflow-graph aware | step-based deterministic workflow runtime |
| Trace/replay | provider metrics/logging only | typed trace schema + recorder + replay validator |
| Scope boundaries | request-level | runtime scoped state with capability checks |

## 8) Phase 0 Acceptance Mapping

- Crate boundaries: complete
- Canonical IR v0: complete
- Validation/lint pass: complete
- Deterministic invariants + trace schema: complete
- Published capability contract: complete (this document)
