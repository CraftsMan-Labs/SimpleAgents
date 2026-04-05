# Workflow Capability Contract

Status: Active contract for the current workflow subsystem.

Scope: Defines additive workflow subsystem boundaries, canonical IR/runtime capabilities, deterministic invariants, validation guarantees, replay/checkpoint behavior, YAML execution, and binding exposure expectations.

## 1) Non-Breaking Scope

- The workflow subsystem is additive and does not replace existing `SimpleAgentsClient` completion/streaming APIs.
- Existing crates and bindings continue to function without workflow adoption.
- Workflow features are introduced via new crate boundaries and typed interfaces.

## 2) Workspace Crate Boundaries

- New crate: `crates/simple-agents-workflow`
- In-scope modules:
  - `ir` - canonical workflow IR types
  - `validation` - normalization and structural diagnostics
  - `runtime` - deterministic execution engine
  - `scheduler` - bounded concurrency primitives
  - `state` - hierarchical scoped state + capability checks
  - `trace` - stable trace event schema
  - `recorder` - in-memory trace recorder
  - `replay` - structural replay validation + replay cache policy
  - `checkpoint` - checkpoint store abstractions and filesystem backend
  - `debug` - timeline/retry/replay inspection helpers
  - `observability` - tracing + metrics adapters
  - `visualize` - Mermaid workflow rendering
- `yaml_runner` - YAML workflow execution with step timing metrics
- Integration boundary:
  - LLM execution is delegated through `LlmExecutor`; production adapter is `impl LlmExecutor for SimpleAgentsClient`.
  - Tool execution is host-injected via `ToolExecutor`.

## 3) Canonical IR Contract

Canonical node taxonomy currently supported:

- `start`
- `llm`
- `tool`
- `condition`
- `loop`
- `parallel`
- `merge`
- `map`
- `reduce`
- `subgraph`
- `batch`
- `filter`
- `debounce`
- `throttle`
- `retry_compensate`
- `human_in_the_loop`
- `cache_read`
- `cache_write`
- `event_trigger`
- `router`
- `transform`
- `end`

IR guarantees:

- Version is `v0` (`WORKFLOW_IR_V0`) for canonical IR fixtures and validation.
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

Runtime invariants:

- deterministic normalized workflow is used for execution
- single active node cursor, step-indexed event stream
- bounded execution via `max_steps`
- cooperative cancellation checks before step and between retry attempts
- runtime-owned node policies for retries/timeouts (`NodeExecutionPolicy`)
- trace sequence is monotonic and deterministic within one run
- per-step execution records are stable (`NodeExecution` + `NodeExecutionData`)

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

Replay cache policy surface:

- `ReplayCachePolicy::Always`
- `ReplayCachePolicy::Refresh`
- `ReplayCachePolicy::Mixed`

Checkpoint and resume surface:

- `WorkflowRuntime::execute_resume_from_failure`
- `FilesystemCheckpointStore` and `CheckpointStore` trait

Workflow YAML streaming/healing surface:

- `llm_call.stream` enables node-level LLM delta streaming
- `llm_call.heal` enables healing mode for structured node output
- when `stream=true` and `heal=true`, streaming is disabled for that node with explanatory event text
- `node_llm_input_resolved` emits per-node resolved LLM input telemetry, including:
  - resolved prompt text and original template
  - model/schema and effective stream/heal flags
  - template binding provenance (`expression`, `source_path`, `resolved`, `missing`)

Workflow YAML memory interpolation surface:

- Prompt interpolation supports:
  - `&#123;&#123; input.email_text &#125;&#125;`
  - `&#123;&#123; nodes.&lt;node_id&gt;.output.&lt;field&gt; &#125;&#125;`
  - `&#123;&#123; globals.&lt;key&gt; &#125;&#125;`
- Node config supports global memory writes:
  - `set_globals` for direct key assignment from context paths
  - `update_globals` with mutable ops:
    - `set`
    - `append`
    - `increment`
    - `merge`

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
| YAML workflow execution | not available | `run_workflow_yaml_file_with_client` / `run_workflow_yaml_with_client` |
| Workflow live events | not available | `run_workflow_yaml_with_client_and_custom_worker_and_events` + `YamlWorkflowEventSink` |
| YAML workflow verifier | not available | `verify_yaml_workflow` diagnostics before execution |
| YAML prompt memory | not available | `set_globals` + `update_globals` + `&#123;&#123; globals.* &#125;&#125;` interpolation |
| Visualization | not available | `workflow_to_mermaid` |

## 8) Cross-Language Exposure Contract

Rust remains source of truth. Language bindings should wrap Rust behavior, not re-implement core logic.

- FFI: `sa_run_workflow_yaml*` returns JSON output from Rust runner.
- Go: `Client.Run(...)` / `RunAsync(...)` / `Stream(...)` delegate through FFI (legacy `RunWorkflowYAML*` wrappers retained).
- Node: `Client.executeWorkflowYaml(...)` / `executeWorkflowYamlStream(...)` delegate through Rust typed execution (legacy `runWorkflowYaml*` wrappers retained).
- Python: `Client.run(...)` / `run_async(...)` / `stream(...)` delegate through Rust typed execution (legacy `run_workflow_yaml*` wrappers retained).

**Binding parity note:** The `simple_agents_py.workflow_stream` helpers (`workflow_event_callback`, `stream_workflow`) are **Python-only** convenience for mapping workflow events to structured hook methods. Node.js callers continue to use JSON `onEvent` callbacks (or a future TypeScript helper). The **`split_stream_deltas`** execution flag is **cross-cutting**: it is implemented in Rust, parsed from Python `execution` objects, and exposed as optional `splitStreamDeltas` on `WorkflowYamlRunRequest` in the N-API binding, OR’d with env `SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW` for backward compatibility.

All binding outputs include terminal output, trace, per-step timing, and total runtime.

## 9) Acceptance Mapping

- Crate boundaries: complete
- Canonical IR + advanced nodes: complete
- Validation/lint pass: complete
- Deterministic invariants + trace schema: complete
- Replay/checkpoint controls: complete
- YAML runner + timing outputs: complete
- Cross-language YAML execution exposure: complete
- Published capability contract: complete (this document)
