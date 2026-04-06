# SimpleAgents Core Rebuild — Full Context Document

> Created: 2026-04-06
> Purpose: Complete context transfer document. Read this to resume the rebuild from any point.

---

## 1. THE VISION

SimpleAgents is a YAML-driven agent orchestration engine. Think "OpenAI SDK but for multi-step workflows."

**The DX dream:**
```python
client = SimpleAgents("openai", api_key="sk-...")
result = client.run("workflow.yaml", [Message(role=Role.USER, content="hello")])
# or streaming:
for event in client.stream("workflow.yaml", [Message(role=Role.USER, content="hello")]):
    pass  # tokens auto-printed, events streamed
```

**Core principles:**
- Rust core, bindings for Python, Node/TS, WASM
- Low memory footprint
- `Vec<Message>` is THE universal input — everywhere, always, typed objects never dicts
- Outputs are either structured (matching `output_schema` with healing/coercion) or plain string — nothing between
- Every function has explicit typed input/output, tests, is DRY and KISS
- Less is more — the current 56K+ line codebase is the anti-pattern

---

## 2. THE CURRENT CODEBASE (what exists today)

**Repository:** `/home/rishub/Desktop/projects/rishub/SimpleAgents`
**Branch:** `refactor/remove-email-text-workflow-api`
**Workspace version:** `0.3.1`

### Line counts:
- **Total Rust:** 56,264 lines across 13 crates
- **Bindings (WASM JS):** 27,189 lines
- **Total project (all code):** 345,216 lines
- **Documentation files:** 173 markdown files

### Crate inventory (by size):
| Crate | Lines | Fate |
|-------|------:|------|
| `simple-agents-workflow` | 22,567 | **GUT heavily** — keep only yaml_runner + observability/tracing |
| `simple-agents-providers` | 10,201 | **GUT** — keep only OpenAI-compat, delete anthropic/openrouter/metrics/retry |
| `simple-agents-healing` | 5,534 | **TRIM** — keep parser, coercion, schema, string_utils; delete StreamAnnotation, PartialExtractor |
| `simple-agent-type` | 5,159 | **MODIFY** — delete cache/router/config, add MessageContent/ContentPart/TelemetryConfig/ApiFormat |
| `simple-agents-py` | 3,531 | **REWRITE** — strip to 5-method API |
| `simple-agents-router` | 1,785 | **DELETE entirely** |
| `simple-agents-napi` | 1,739 | **REWRITE** — strip to 5-method API |
| `simple-agents-core` | 1,452 | **SIMPLIFY** — remove routing/middleware/cache |
| `simple-agents-cli` | 1,279 | **DELETE entirely** |
| `simple-agents-macros` | 511 | **DELETE entirely** |
| `simple-agents-cache` | 492 | **DELETE entirely** |
| `simple-agents-workflow-workers` | 484 | **DELETE entirely** |

### Binding locations:
- **Python:** `crates/simple-agents-py/` (PyO3/Maturin)
- **Node/TS:** `crates/simple-agents-napi/` (napi-rs)
- **WASM:** `bindings/wasm/simple-agents-wasm/` (wasm-bindgen, **has its own separate workflow engine** — must be rewritten)

---

## 3. PROBLEMS IDENTIFIED (from the analysis)

1. **Two execution engines for the same YAML** — native interpreter vs IR/WorkflowRuntime. They can silently diverge.
2. **The IR is a fantasy** — defines Parallel/Merge/Map/Reduce/Loop/Subgraph but YAML only uses llm_call/switch/custom_worker/end.
3. **WorkflowRuntime is 3,559 lines** of unused DAG scheduler.
4. **Router crate is premature** — users pick one model per YAML node.
5. **Providers are bloated** — 10K lines for what should be one OpenAI-compat HTTP client.
6. **Healing is a separate product** being exposed as primary API surface.
7. **Bindings have 5-30 methods** where 5 suffice.
8. **gRPC workers, CLI, proc macros** are all scope creep.
9. **173 doc files** documenting a cathedral while laying foundation.
10. **WASM has a completely separate workflow engine** not matching the Rust one.
11. **`node_completed` event currently does NOT include the node output** — must fix.
12. **No retry at YAML workflow level** — single failure surfaces immediately.
13. **No partial output on failure** — `WorkflowRunOutput` only built on success.

---

## 4. FEATURES TO SUPPORT (confirmed with user)

1. **YAML workflow execution** — `client.run("workflow.yaml", messages)`
2. **Workflow streaming** — stream each node step + LLM token deltas when enabled
3. **Direct LLM calls with healing** — `client.complete(request)` → healed JSON
4. **Coercion healing** — LLM output auto-coerced to match `output_schema`
5. **Tool calling / function calling** — user provides `ToolExecutor` callbacks
6. **Both OpenAI Chat Completions AND Responses API** (both in initial implementation)
7. **Recovery workflows** — retry per node with backoff, return partial output on failure, checkpoint for resume
8. **Nerdstats** — TTFT, token counts (input/output/reasoning), model name, tokens/sec — behind feature flag
9. **Telemetry** — Jaeger + Langfuse via OTLP. Explicit code config primary, env var fallback
10. **Routing DELETED** — user requirement #10, too much responsibility for this package
11. **All supported languages** — Rust, Python, Node/TS, WASM

---

## 5. KEY DESIGN DECISIONS MADE

### Universal Input: `Vec<Message>` everywhere
- `Message.content` is `MessageContent` enum: `Text(String)` | `Parts(Vec<ContentPart>)`
- `ContentPart` is `Text { text }` | `ImageUrl { image_url: { url, detail? } }` | `Video { url }`
- Matches OpenAI multimodal format exactly
- For workflows: `llm_call` entry nodes receive messages directly. `switch` nodes access message content via execution context. Non-LLM entry nodes access via `$.input.messages`.
- Checkpoint stores `original_messages: Vec<Message>` for resume

### Universal Output Contract
- Structured (matches `output_schema`, backed by healing/coercion) = typed JSON value
- Plain string = raw LLM text
- Nothing in between. No ambiguous partial JSON.

### Streaming DX: `DefaultEventPrinter` built-in
- Rust: `client.stream(path, msgs, &DefaultEventPrinter, opts)` — prints tokens to stdout, lifecycle to stderr
- All other languages: `client.stream(path, msgs)` prints by default
- Custom handler is optional, not required — `CallbackSink` for advanced users only
- This gives identical DX across all languages

### Telemetry Config: Code primary, env fallback
- `TelemetryConfig { enabled, endpoint, protocol, headers, service_name }`
- Falls back to: `SIMPLE_AGENTS_TRACING_ENABLED`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_PROTOCOL`, `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_SERVICE_NAME`
- Langfuse via OTLP with Langfuse attribute helpers (user.id, session.id, observation.*)

### Tool Calling: Callback pattern
- User implements `ToolExecutor` trait (async fn execute(tool_name, arguments) -> Result<Value>)
- Engine handles the LLM → tool → LLM round-trip loop (bounded by `max_tool_roundtrips`)
- Tools declared in YAML on `llm_call` nodes
- Built-in `run_workflow_graph` tool for subgraph invocation exists in current code

### Recovery: Checkpoint + Partial Output
- On node failure: retry with backoff (configurable `RetryConfig { max_attempts, backoff_ms }`)
- If all retries fail: return `WorkflowError::NodeFailed` containing:
  - `partial_output: PartialWorkflowOutput` (completed_trace, completed_outputs, nerdstats up to failure)
  - `checkpoint: WorkflowCheckpoint` (serializable, contains original_messages + all state needed to resume)
- `client.resume(checkpoint, options)` restarts from failed node with all previous outputs intact
- Checkpoint is JSON-serializable — can be stored in Redis/SQS/Kafka/database/file

### OpenAI API Support: Both formats
- `ApiFormat::ChatCompletions` (default) — POST /v1/chat/completions
- `ApiFormat::Responses` — POST /v1/responses
- Key differences handled in provider:
  - Request: `messages` vs `input`/`instructions`, `response_format` vs `text.format`, nested vs flat tools
  - Response: `choices[0].message.content` vs `output` items array
  - Streaming: `data: {...}` chunks vs typed SSE events (`response.output_text.delta`, etc.)
  - Tool results: `role: "tool"` messages vs `function_call_output` items
  - New: `previous_response_id`, `store` for server-side state

---

## 6. TARGET ARCHITECTURE

### 5 Rust crates + 4 bindings → ~12,800 lines total

```
simple-agent-type        (~1,800 lines)  — Message, Request, Response, Provider trait, Coercion, Telemetry
simple-agents-healing    (~1,800 lines)  — JsonishParser, CoercionEngine, Schema, StreamingParser
simple-agents-providers  (~2,500 lines)  — One OpenAI-compatible HTTP provider + SSE streaming
simple-agents-core       (~1,300 lines)  — SimpleAgentsClient (complete, stream_complete, run, stream, resume)
simple-agents-workflow   (~2,200 lines)  — YAML loader + state machine executor + events + recovery
simple-agents-py         (~900 lines)    — PyO3 binding
simple-agents-napi       (~700 lines)    — Node binding
simple-agents-wasm       (~700 lines)    — WASM binding (uses engine via wasm-bindgen)
```

### Unified API (all languages, 5 core methods):

```
Constructor:
  client = SimpleAgents(provider, api_key, options?)

Workflow:
  result = client.run(yaml_path, messages, options?)
  result = client.stream(yaml_path, messages, options?)                   // default: prints tokens
  result = client.stream(yaml_path, messages, on_event, options?)         // custom handler
  result = client.run_with_tools(yaml_path, messages, tool_executor, options?)
  result = client.stream_with_tools(yaml_path, messages, tool_executor, ...)
  result = client.resume(checkpoint, options?)

LLM:
  response = client.complete(request)
  stream   = client.stream_complete(request)
```

### Key types:

```rust
// Client config — all explicit, no hidden flags
pub struct ClientConfig {
    pub provider: String,           // "openai" or any OpenAI-compat name
    pub api_key: String,
    pub base_url: Option<String>,   // override endpoint
    pub api_format: ApiFormat,      // ChatCompletions (default) | Responses
    pub extra_headers: Option<Vec<(String, String)>>,
    pub telemetry: Option<TelemetryConfig>,
    pub default_retry: RetryConfig,
}

// Run options
pub struct RunOptions {
    pub nerdstats: bool,            // default: true
    pub telemetry_enabled: bool,    // default: follows client config
    pub trace_context: Option<TraceContext>,
    pub execution_flags: ExecutionFlags,
}

pub struct ExecutionFlags {
    pub workflow_streaming: bool,    // emit node lifecycle events
    pub node_llm_streaming: bool,   // stream LLM tokens within nodes
}

// Workflow events — typed enum, not stringly-typed
pub enum WorkflowEvent {
    WorkflowStarted { workflow_id },
    NodeStarted { node_id, node_type },
    LlmTokenDelta { node_id, token, token_kind },
    NodeCompleted { node_id, output },           // NOTE: current code MISSING output here
    ToolCallRequested { node_id, tool_name, arguments },
    ToolCallCompleted { node_id, tool_name, output },
    NodeRetrying { node_id, attempt, error },
    NodeFailed { node_id, error },
    WorkflowCompleted { output, metadata },
}

// Output — same shape for success and partial failure
pub struct WorkflowRunOutput {
    pub workflow_id: String,
    pub trace: Vec<String>,
    pub outputs: BTreeMap<String, Value>,
    pub terminal_node: String,
    pub terminal_output: Option<Value>,
    pub metadata: Option<RunMetadata>,
}

// Nerdstats
pub struct RunMetadata {
    pub total_elapsed_ms: u128,
    pub ttft_ms: Option<u128>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub total_reasoning_tokens: Option<u64>,
    pub tokens_per_second: f64,
    pub step_details: Vec<StepTiming>,
    pub trace_id: Option<String>,
}

// Checkpoint for resume
pub struct WorkflowCheckpoint {
    pub workflow_path: String,
    pub failed_node_id: String,
    pub completed_trace: Vec<String>,
    pub completed_outputs: BTreeMap<String, Value>,
    pub globals: BTreeMap<String, Value>,
    pub original_messages: Vec<Message>,
    pub step_timings: Vec<StepTiming>,
    pub token_totals: TokenTotals,
}
```

---

## 7. REUSABLE CODE MAP

### Code to KEEP (trimmed):
| Component | Current File | Lines | Keep |
|-----------|-------------|------:|-----:|
| JsonishParser | `crates/simple-agents-healing/src/parser.rs` | 1,126 | ~700 |
| CoercionEngine | `crates/simple-agents-healing/src/coercion.rs` | 867 | ~500 |
| Schema types | `crates/simple-agents-healing/src/schema.rs` | 391 | ~280 |
| String utils | `crates/simple-agents-healing/src/string_utils.rs` | 267 | ~150 |
| CoercionFlag/Result | `crates/simple-agent-type/src/coercion.rs` | 346 | ~150 |
| Message/Request/Response | `crates/simple-agent-type/src/` | 1,431 | ~900 |
| Provider trait | `crates/simple-agent-type/src/provider.rs` | 493 | ~250 |
| Tool types | `crates/simple-agent-type/src/tool.rs` | 93 | ~120 |
| OpenAI provider | `crates/simple-agents-providers/src/openai/mod.rs` | 1,276 | ~650 |
| SSE streaming | `crates/simple-agents-providers/src/openai/streaming.rs` | 367 | ~280 |
| OpenAI models | `crates/simple-agents-providers/src/openai/models.rs` | 408 | ~280 |
| HTTP utils | `crates/simple-agents-providers/src/utils.rs` + `common/` | 589 | ~300 |
| Healing integration | `crates/simple-agents-providers/src/healing_integration.rs` | 419 | ~300 |
| Core client | `crates/simple-agents-core/src/client.rs` | 832 | ~450 |
| Core healing bridge | `crates/simple-agents-core/src/healing.rs` | 73 | ~73 |
| YAML runner parent module | `crates/simple-agents-workflow/src/yaml_runner.rs` | 5,955 | ~200 (gut to module decls + nerdstats; move API/stream-filters/telemetry-attrs to dedicated files) |
| YAML runner API | `crates/simple-agents-workflow/src/yaml_runner/api.rs` | 481 | ~200 (simplify to run + stream entry points) |
| Native YAML executor | `crates/simple-agents-workflow/src/yaml_runner/execute.rs` | 545 | ~500 |
| Node execution | `crates/simple-agents-workflow/src/yaml_runner/node_execution.rs` | 339 | ~250 |
| Client executor (tools) | `crates/simple-agents-workflow/src/yaml_runner/client_executor.rs` | 957 | ~400 |
| YAML AST types | `crates/simple-agents-workflow/src/yaml_runner/contracts.rs` | 424 | ~350 |
| YAML validation | `crates/simple-agents-workflow/src/yaml_runner/validation.rs` | 252 | ~250 |
| Switch context | `crates/simple-agents-workflow/src/yaml_runner/context.rs` | 180 | ~180 |
| Globals | `crates/simple-agents-workflow/src/yaml_runner/globals.rs` | 110 | ~110 |
| OTel spans | `crates/simple-agents-workflow/src/yaml_runner/spans.rs` | 95 | ~95 |
| Tool normalization | `crates/simple-agents-workflow/src/yaml_runner/llm_tools.rs` | 91 | ~91 |
| OTel tracing | `crates/simple-agents-workflow/src/observability/tracing.rs` | 543 | ~350 |

### Code to CREATE NEW:
| Component | Est. Lines |
|-----------|----------:|
| `telemetry.rs` in types (ApiFormat, TelemetryConfig, TraceContext) | ~60 |
| `responses.rs` in provider (Responses API request/response/streaming) | ~500 |
| `events.rs` replacement (typed WorkflowEvent enum + DefaultEventPrinter) | ~200 |
| `output.rs` (WorkflowRunOutput, RunMetadata, StepTiming, TokenTotals) | ~250 |
| `recovery.rs` (WorkflowCheckpoint, PartialWorkflowOutput) | ~300 |

---

## 8. IMPLEMENTATION PHASES

### Phase 1: Delete Dead Crates & Clean Workspace
- Delete: simple-agents-router, simple-agents-cache, simple-agents-workflow-workers, simple-agents-cli, simple-agents-macros, workers/
- Delete: anthropic/, openrouter/, rate_limit.rs, metrics.rs, retry.rs, streaming_structured.rs from providers
- Delete: ir.rs, runtime.rs, scheduler.rs, worker.rs, worker_adapter.rs, expressions.rs, replay.rs, debug.rs, visualize.rs, checkpoint.rs, state/, recorder.rs, trace.rs, engine.rs, scope.rs from workflow
- Delete: cache.rs, router.rs, config.rs from simple-agent-type
- Gut: simple-agents-core (remove routing.rs, middleware.rs, strip client.rs)

### Phase 2: Rebuild `simple-agent-type`
- Add MessageContent/ContentPart multimodal union to message.rs
- Add Responses API fields to request.rs (instructions, previous_response_id, store)
- Create telemetry.rs (ApiFormat, OtelProtocol, TelemetryConfig, TraceContext)

### Phase 3: Trim `simple-agents-healing`
- Remove StreamAnnotation from schema.rs
- Remove PartialExtractor from streaming.rs
- Trim verbose docs

### Phase 4: Rebuild `simple-agents-providers`
- Rename OpenAIProvider → OpenAiCompatProvider with configurable base_url + api_format
- Create responses.rs for Responses API support
- Wire api_format branching into Provider trait impl

### Phase 5: Rebuild Engine (`simple-agents-core` + `simple-agents-workflow`)
- Replace events.rs with typed WorkflowEvent enum + DefaultEventPrinter
- Create output.rs (WorkflowRunOutput, RunMetadata, StepTiming)
- Create recovery.rs (WorkflowCheckpoint, PartialWorkflowOutput)
- Decompose yaml_runner.rs parent module (5,955 lines → ~80 lines mod.rs):
  - MOVE to new `telemetry.rs` (~350 lines): trace ID resolution (lines 105-236), trace attribute setters (237-316), Langfuse helpers (374-520), trace context utils (578-664)
  - MOVE to new `nerdstats.rs` (~100 lines): `workflow_nerdstats` (lines 317-373) + tests
  - MOVE to new `stream_filters.rs` (~300 lines): JSON delta filter/text formatter (lines 665-913)
  - MOVE to new `subworkflow.rs` (~150 lines): `execute_subworkflow_tool_call`, `build_subworkflow_options` (lines 2305-2433)
  - MOVE to new `loader.rs` (~90 lines): `yaml_value_depth`, `load_workflow_yaml_file` (lines 1344-1431)
  - MOVE schema helpers to existing `validation.rs`: `validate_json_schema`, `validate_schema_instance`, `schema_type`, `schema_expects_object` (lines 521-577)
  - DELETE mermaid generation (lines 914-1337, 424 lines + lines 1705-1774, 70 lines)
  - DELETE IR conversion (lines 1432-1704, 273 lines: `yaml_workflow_to_ir`, `rewrite_yaml_condition_to_ir`, etc.)
  - DELETE `try_run_yaml_via_ir_runtime` (lines 1866-2304, 439 lines: IR runtime bridge + `YamlIrToolExecutor`)
  - DELETE `runner.rs` (286 lines), `typed_contracts.rs` (229 lines)
  - TRIAGE tests (lines 2434-5955, 3,522 lines): ~2,000+ deleted, ~1,000-1,500 moved to submodules
  - COLLAPSE 15+ API wrappers (lines 1775-1865 + api.rs) into 2-3 clean functions
- Refactor executor: Vec<Message> input, retry logic, checkpoint on failure
- Wire SimpleAgentsClient with final API in simple-agents-core

### Phase 6: Rebuild Bindings
- Python: strip to 5-method API with typed Message/Role/ContentPart
- Node: strip to 5-method API
- WASM: rewrite to use real engine

### Phase 7: Cleanup
- Delete 150+ old doc files, keep 5-6
- Update README
- Final cargo test/clippy/fmt
- Target: ~12,800 lines (down from 56K + 27K)

---

## 9. PLAN FILES

Two plan files exist in `.cursor/plans/`:

1. **`simpleagents_core_rebuild_62f11c53.plan.md`** — Architecture plan with full Rust DX examples (11 code examples covering every feature), binding examples for all languages, crate structure, file fate map
2. **`simpleagents_implementation_guide_9f57a2e1.plan.md`** — Step-by-step idiot-proof implementation guide with exact files to create/delete/modify, exact code to write, exact tests, exact shell commands

---

## 10. HOW TOOL CALLING WORKS (current, to preserve)

- Tool types: `ToolDefinition`, `ToolCall`, `ToolChoice` in `crates/simple-agent-type/src/tool.rs`
- Tool results are `Message` with `role: Tool` + `tool_call_id` (no separate `ToolResult` struct)
- YAML declares tools on `llm_call` nodes with `tools`, `tool_choice`, `max_tool_roundtrips`
- Engine executes tools via `YamlWorkflowCustomWorkerExecutor` callback (our `ToolExecutor` trait)
- Built-in `run_workflow_graph` tool for subgraph invocation
- Round-trip loop in `client_executor.rs` lines 38-663: LLM response → parse tool_calls → execute each → append tool result messages → re-call LLM → repeat until non-tool response or max_roundtrips
- Key files: `tool.rs`, `llm_tools.rs`, `client_executor.rs`, `contracts.rs` (YamlLlmCall tools fields)

## 11. HOW NERDSTATS WORK (current, to preserve)

- `workflow_nerdstats()` in `yaml_runner.rs` lines 317-371
- TTFT: measured in `client_executor.rs` as time from request start to first non-empty content chunk
- Per-node: `YamlStepTiming` with optional token fields, filled from `node_usage`
- Aggregates: `YamlTokenTotals` sums across all LLM calls
- `tokens_per_second = output_tokens * 1000 / total_elapsed_ms`
- Controlled by `telemetry.nerdstats` flag (default true)
- Key types: `YamlStepTiming`, `YamlLlmNodeMetrics`, `YamlTokenTotals` in `types.rs`

## 12. HOW TELEMETRY WORKS (current, to preserve)

- OTLP exporter in `observability/tracing.rs` (gRPC or HTTP/protobuf)
- Env vars: `SIMPLE_AGENTS_TRACING_ENABLED`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_PROTOCOL`, `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_SERVICE_NAME`
- Spans: root `workflow.run`, children `workflow.node.execute`
- Langfuse: attributes `langfuse.user.id`, `langfuse.session.id`, `langfuse.observation.*` on spans
- Span helpers in `yaml_runner/spans.rs`
- No Langfuse Rust SDK — all via OTLP with Langfuse-expected attribute keys

## 13. HOW YAML WORKFLOWS ARE STRUCTURED

```yaml
id: my-workflow
entry_node: first_node
nodes:
  - id: first_node
    node_type:
      llm_call:
        model: gpt-4o
        stream: true
        tools: [...]
        tool_choice: auto
        max_tool_roundtrips: 3
    config:
      prompt: "..."
      output_schema: { type: object, properties: {...} }
  - id: router
    node_type:
      switch:
        branches:
          - condition: '$.nodes.first_node.output.field == "value"'
            target: next_node
        default: fallback_node
  - id: end_node
    node_type:
      end: true
edges:
  - from: first_node
    to: router
```

Node types: `llm_call`, `switch`, `custom_worker`, `end`
Switch conditions: simple `==` between dotted path and quoted string
Edges: explicit `from` → `to` pairs; terminal nodes have no outgoing edge

---

## 14. WHAT HAS NOT BEEN DONE YET

**Nothing has been implemented.** Both plans are approved but execution has not started. The codebase is untouched. All changes are documented in the two plan files. Start with Phase 1 of the implementation guide.
