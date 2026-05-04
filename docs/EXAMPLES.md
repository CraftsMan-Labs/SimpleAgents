# Code Examples

Use this page as a learning path. If you are new, go in this order:

1. Basic completion (Rust)
2. Simple YAML workflow (Python / TypeScript)
3. Streaming workflow
4. Image input workflow
5. Observability (Langfuse / Jaeger)

For the fastest workflow setup, start with [Workflow Quickstart](/WORKFLOW_QUICKSTART).

## Basic Completion (Rust)

```rust
use simple_agent_type::prelude::*;
use simple_agents_core::{CompletionOptions, CompletionOutcome, SimpleAgentsClientBuilder};
use simple_agents_providers::OpenAiCompatProvider;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = Arc::new(OpenAiCompatProvider::new(api_key)?);

    let client = SimpleAgentsClientBuilder::new()
        .with_provider(provider)
        .build()?;

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello, world!"))
        .build()?;

    let outcome = client.complete(&request, CompletionOptions::default()).await?;
    if let CompletionOutcome::Response(response) = outcome {
        println!("{}", response.content().unwrap_or_default());
    }

    Ok(())
}
```

## Routing Across Providers (Rust)

```rust
use simple_agent_type::prelude::*;
use simple_agents_core::{RoutingMode, SimpleAgentsClientBuilder};
use simple_agents_providers::OpenAiCompatProvider;
use std::sync::Arc;

let openai = Arc::new(OpenAiCompatProvider::new(ApiKey::new("sk-...")?)?);
let azure = Arc::new(OpenAiCompatProvider::with_base_url(
    ApiKey::new("sk-...")?,
    "https://my-azure.openai.azure.com/v1",
)?);

let client = SimpleAgentsClientBuilder::new()
    .with_providers(vec![openai, azure])
    .with_routing_mode(RoutingMode::Fallback)
    .build()?;
```

## Streaming (Rust)

```rust
use futures_util::StreamExt;
use simple_agent_type::prelude::*;
use simple_agents_core::{CompletionOptions, CompletionOutcome};

let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Stream a short poem"))
    .stream(true)
    .build()?;

match client.complete(&request, CompletionOptions::default()).await? {
    CompletionOutcome::Stream(mut stream) => {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if let Some(choice) = chunk.choices.first() {
                if let Some(content) = &choice.delta.content {
                    print!("{}", content);
                }
            }
        }
    }
    _ => {}
}
```

## Healed JSON (Rust)

```rust
use simple_agent_type::prelude::*;
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, HealingSettings,
    SimpleAgentsClientBuilder,
};

let client = SimpleAgentsClientBuilder::new()
    .with_provider(provider)
    .with_healing_settings(HealingSettings::default())
    .build()?;

let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Return JSON with name and age"))
    .build()?;

let options = CompletionOptions {
    mode: CompletionMode::HealedJson,
};

match client.complete(&request, options).await? {
    CompletionOutcome::HealedJson(result) => {
        println!("{}", result.parsed.value);
    }
    _ => {}
}
```

## Workflow Examples

All workflow examples live under `examples/`. Each has a Python and TypeScript variant.

### Example Files

**Python** (`examples/python-test-simpleAgents/`):

| Location | What it demonstrates |
|---|---|
| `runners/test-py-simple-agents.py` | Normal (blocking) YAML workflow run |
| `runners/test-py-simple-agents-streaming.py` | Streaming with `WorkflowExecutionFlags` |
| `runners/test-py-simple-agents-invoice-image.py` | Image input (multimodal) |
| `runners/test-py-simple-agents-invoice-image-streaming.py` | Image input + streaming |
| `runners/test-py-simple-agents-streaming-langfuse.py` | Streaming + Langfuse OTLP |
| `runners/test-py-simple-agents-invoice-image-jaeger.py` | Image input + Jaeger OTLP (`*-jaegar.py` remains as a compatibility alias) |
| `apps/fastapi_workflow_stream.py` | FastAPI SSE streaming endpoint |
| `workflows/email-classification/handlers.py` | Custom worker handler (`get_seller_name`) |
| `runners/test-py-simple-agents-eval.py` | Output-shaped eval dataset for `workflows/friendly/friendly.yaml` |
| `runners/test-py-simple-agents-invoice-image-evals.py` | Invoice workflow evals for node-level and terminal-node checks |
| `runners/test-py-simple-agents-rag-eval.py` | Mocked RAG chunk workflow + custom eval handler |
| `example_paths.py` | Shared path helpers (`workflows/`, `evals/`, `assets/`) |

**TypeScript** (`examples/napi-test-simpleAgents/`):

| Location | What it demonstrates |
|---|---|
| `runners/test-simple-agents.ts` | Normal (blocking) YAML workflow run |
| `runners/test-simple-agents-streaming.ts` | Streaming with execution flags |
| `runners/test-simple-agents-invoice-image.ts` | Image input (multimodal) |
| `runners/test-simple-agents-streaming-langfuse.ts` | Streaming + Langfuse OTLP |
| `runners/test-simple-agents-invoice-image-jaeger.ts` | Image input + Jaeger OTLP (`*-jaegar.ts` remains as a compatibility alias) |
| `workflows/email-classification/handlers.ts` | Custom worker dispatch (`getSellerName`) |
| `runners/test-simple-agents-eval.ts` | Output-shaped eval (datasets under `evals/`; friendly workflow may point at Python sibling) |
| `runners/test-simple-agents-rag-eval.js` | Mocked RAG chunks + `evaluate_rag_chunks` custom eval |
| `example_paths.ts` | Path helpers aligned with Python `example_paths.py` |

**YAML workflows**

| Location | Notes |
|---|---|
| `python-test-simpleAgents/workflows/email-classification/test.yaml` | Full email classification demo; paired with Python `handlers.py`. |
| `python-test-simpleAgents/workflows/friendly/friendly.yaml` | Minimal single-node chat bot (`friendly-eval` golden tests point here). |
| `napi-test-simpleAgents/workflows/email-classification/test.yaml` | Same routing graph as Python; paired with TypeScript `handlers.ts`. |

**Eval suites**

| Location | Notes |
|---|---|
| `python-test-simpleAgents/evals/friendly/` | Friendly-chat golden dataset (`friendly-eval.dataset.jsonl`). |
| `python-test-simpleAgents/evals/invoice/` | Invoice regression suites (paths vs terminal-only). |
| `python-test-simpleAgents/evals/rag/` | Mocked `rag-eval` workflow + scorer. |
| `napi-test-simpleAgents/evals/friendly/` | Friendly eval referencing Python sibling workflows/datasets (`../../../python-test-simpleAgents/…`). |
| `napi-test-simpleAgents/evals/rag/` | NAPI-local rag golden dataset; runner supplies the workflow path and evaluator callback. |
| `napi-test-simpleAgents/evals/invoice/` | Invoice multimodal path evals (parity with Python; JSONL generated beside YAML). |

### Running the Examples

**Python** (from `examples/`):

```bash
cd examples
uv sync
cd python-test-simpleAgents

# Normal run
uv run python runners/test-py-simple-agents.py

# Streaming
uv run python runners/test-py-simple-agents-streaming.py

# With image (place JPEG at assets/test-invoice.jpeg)
uv run python runners/test-py-simple-agents-invoice-image.py

# Streaming + Langfuse
uv run python runners/test-py-simple-agents-streaming-langfuse.py

# Image + Jaeger
uv run python runners/test-py-simple-agents-invoice-image-jaeger.py

# Output-shaped eval
uv run python runners/test-py-simple-agents-eval.py

# Invoice workflow evals
uv run python runners/test-py-simple-agents-invoice-image-evals.py

# FastAPI server
uv run uvicorn apps.fastapi_workflow_stream:app --reload
```

**TypeScript / Bun** (from `examples/napi-test-simpleAgents/`):

```bash
cd examples/napi-test-simpleAgents
bun install

# Normal run
bun run run
# or
bun run runners/test-simple-agents.ts

# Streaming
bun run stream
# or
bun run runners/test-simple-agents-streaming.ts

# With image (JPEG at sibling `python-test-simpleAgents/assets/test-invoice.jpeg`)
bun run invoice-image
# or
bun run runners/test-simple-agents-invoice-image.ts

# Streaming + Langfuse
bun run stream:langfuse
# or
bun run runners/test-simple-agents-streaming-langfuse.ts

# Image + Jaeger
bun run invoice-image:jaeger
# or
bun run runners/test-simple-agents-invoice-image-jaeger.ts

# Invoice multimodal workflow evals (regenerates JSONL, then runs suites)
bun run invoice-image:evals
# or
bun run runners/test-simple-agents-invoice-image-evals.ts

# Output-shaped eval
bun run runners/test-simple-agents-eval.ts
```

## Workflow Evals

Eval datasets are JSONL golden records. Each row includes workflow `input` and an `expected_output` object shaped like the normal workflow run output.

```json
{"id":"hello-basic","input":{"messages":[{"role":"user","content":"Reply with exactly: hello"}]},"expected_output":{"terminal_node":"chat_reply","trace":["chat_reply"],"outputs":{"chat_reply":{"output":"hello"}}}}
```

Eval YAML suites are no longer needed. The runners pass `workflow_path` / `dataset_path` in code and provide an evaluator callback that receives `{input, expected_output, actual_output, record}` (Python) or `{ input, expectedOutput, actualOutput, record }` (TypeScript). Use built-ins such as Python `output_subset` / `terminal_node_exact`, or write a small function for workflow-specific checks. Invoice multimodal evals use **`evals/invoice/generated/*.dataset.jsonl`**: Python runner `examples/python-test-simpleAgents/runners/test-py-simple-agents-invoice-image-evals.py` runs two invoice goldens that must **pass** plus two deliberate mismatch datasets that must **fail** when routing is correct. The NAPI runner `examples/napi-test-simpleAgents/runners/test-simple-agents-invoice-image-evals.ts` runs the two golden datasets only.

Or use the package.json scripts:

```bash
bun run run          # normal
bun run stream       # streaming
bun run stream:langfuse   # streaming + langfuse
bun run invoice-image     # image input
bun run invoice-image:jaeger  # image + jaeger
```

### Workflow Learning Path

**1. Start simple: single-node workflow**

Read `examples/python-test-simpleAgents/workflows/friendly/friendly.yaml` -- one `llm_call` node, no routing, plain text output.

Run it:

```bash
# In runners/test-py-simple-agents-streaming.py, point workflow_file at workflows/friendly/friendly.yaml
```

**2. Add classification + routing + custom workers**

Read `examples/python-test-simpleAgents/workflows/email-classification/test.yaml` -- multi-node email classification with:
- `llm_call` nodes for classification and extraction
- `switch` nodes for deterministic routing
- `custom_worker` node for stakeholder lookup
- `heal: true` and `stream: true` on all LLM nodes
- Template interpolation (<code v-pre>{{ nodes.X.output.Y }}</code>)

**3. Add streaming**

Compare `runners/test-py-simple-agents.py` (blocking) with `runners/test-py-simple-agents-streaming.py`:
- Add `WorkflowExecutionFlags(node_llm_streaming=True)`
- Use `client.stream_workflow()` with an `on_event` callback

**4. Add image input**

Compare `runners/test-py-simple-agents.py` with `runners/test-py-simple-agents-invoice-image.py`:
- Change `content` from a string to a list with `text` + `image_url` parts
- Base64-encode the image file

**5. Add observability**

Compare the base streaming example with `runners/test-py-simple-agents-streaming-langfuse.py`:
- Configure OTLP env vars for Langfuse or Jaeger
- Add `WorkflowRunOptions(telemetry=WorkflowTelemetryConfig(enabled=True))`

### Skill-Builder Example YAMLs

Under `skills/simpleagents-builder/examples/`:

| File | Pattern |
|---|---|
| `minimal-chat.yaml` | Minimal single-node chat workflow |
| `email-classification.yaml` | Deterministic email classification with custom worker enrichment |

## Rust Workflow API

Direct Rust workflow execution:

```rust
use serde_json::json;
use simple_agents_workflow::yaml_runner::{
    workflow_execution, YamlWorkflowExecutionFlags, YamlWorkflowExecutionRequest,
    YamlWorkflowExecutorBinding, YamlWorkflowRunOptions, YamlWorkflowSource,
};

let workflow_input = json!({
    "messages": [
        {"role": "user", "content": "Classify this email about an invoice from Google."}
    ]
});
let options = YamlWorkflowRunOptions::default();
let execution_request = YamlWorkflowExecutionRequest {
    source: YamlWorkflowSource::File(std::path::Path::new("workflow.yaml")),
    workflow_input: &workflow_input,
    executor: YamlWorkflowExecutorBinding::Client(&client),
    custom_worker: None,
    options: &options,
    flags: YamlWorkflowExecutionFlags::default(),
};
let output = workflow_execution::run(execution_request).await?;

println!("terminal: {}", output.terminal_node);
println!("total_ms: {}", output.total_elapsed_ms);
for step in output.step_timings {
    println!("{} {}ms", step.node_id, step.elapsed_ms);
}
```

## Cross-Language Example Files

| Language | File | Purpose |
|---|---|---|
| Python | `examples/python-test-simpleAgents/runners/test-py-simple-agents.py` | Normal workflow run |
| Python | `examples/python-test-simpleAgents/runners/test-py-simple-agents-streaming.py` | Streaming workflow |
| Python | `examples/python-test-simpleAgents/runners/test-py-simple-agents-invoice-image.py` | Image input |
| Python | `examples/python-test-simpleAgents/runners/test-py-simple-agents-streaming-langfuse.py` | Langfuse tracing |
| Python | `examples/python-test-simpleAgents/runners/test-py-simple-agents-invoice-image-jaeger.py` | Jaeger tracing |
| Python | `examples/python-test-simpleAgents/apps/fastapi_workflow_stream.py` | FastAPI SSE server |
| TypeScript | `examples/napi-test-simpleAgents/runners/test-simple-agents.ts` | Normal workflow run |
| TypeScript | `examples/napi-test-simpleAgents/runners/test-simple-agents-streaming.ts` | Streaming workflow |
| TypeScript | `examples/napi-test-simpleAgents/runners/test-simple-agents-invoice-image.ts` | Image input |
| TypeScript | `examples/napi-test-simpleAgents/runners/test-simple-agents-streaming-langfuse.ts` | Langfuse tracing |
| TypeScript | `examples/napi-test-simpleAgents/runners/test-simple-agents-invoice-image-jaeger.ts` | Jaeger tracing |
| TypeScript | `examples/napi-test-simpleAgents/runners/test-simple-agents-invoice-image-evals.ts` | Invoice multimodal eval suites |
| Rust | `examples/full_api_example.rs` | Full Rust client API |
| Rust | `examples/python_client.py` | Python client API (completions, streaming, healing, tools) |
| Rust | `examples/node_client.js` | Node client API (completions, streaming) |
