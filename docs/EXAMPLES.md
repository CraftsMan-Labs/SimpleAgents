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
use simple_agents_providers::openai::OpenAIProvider;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = Arc::new(OpenAIProvider::new(api_key)?);

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
use simple_agents_providers::{anthropic::AnthropicProvider, openai::OpenAIProvider};
use std::sync::Arc;

let openai = Arc::new(OpenAIProvider::new(ApiKey::new("sk-...")?)?);
let anthropic = Arc::new(AnthropicProvider::new(ApiKey::new("sk-...")?)?);

let client = SimpleAgentsClientBuilder::new()
    .with_providers(vec![openai, anthropic])
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

| File | What it demonstrates |
|---|---|
| `test-py-simple-agents.py` | Normal (blocking) YAML workflow run |
| `test-py-simple-agents-streaming.py` | Streaming with `WorkflowExecutionFlags` |
| `test-py-simple-agents-invoice-image.py` | Image input (multimodal) |
| `test-py-simple-agents-invoice-image-streaming.py` | Image input + streaming |
| `test-py-simple-agents-streaming-langfuse.py` | Streaming + Langfuse OTLP |
| `test-py-simple-agents-invoice-image-jaegar.py` | Image input + Jaeger OTLP |
| `fastapi_workflow_stream.py` | FastAPI SSE streaming endpoint |
| `handlers.py` | Custom worker handler (`get_seller_name`) |

**TypeScript** (`examples/napi-test-simpleAgents/`):

| File | What it demonstrates |
|---|---|
| `test-simple-agents.ts` | Normal (blocking) YAML workflow run |
| `test-simple-agents-streaming.ts` | Streaming with execution flags |
| `test-simple-agents-invoice-image.ts` | Image input (multimodal) |
| `test-simple-agents-streaming-langfuse.ts` | Streaming + Langfuse OTLP |
| `test-simple-agents-invoice-image-jaegar.ts` | Image input + Jaeger OTLP |
| `handlers.ts` | Custom worker dispatch (`getSellerName`) |

**YAML workflows** (`examples/python-test-simpleAgents/` and `examples/napi-test-simpleAgents/`):

| File | What it demonstrates |
|---|---|
| `test.yaml` | Email hierarchical classification with finance enrichment and custom worker |
| `friendly.yaml` | Minimal single-node chat bot |

### Running the Examples

**Python** (from `examples/`):

```bash
cd examples
uv sync
cd python-test-simpleAgents

# Normal run
uv run python test-py-simple-agents.py

# Streaming
uv run python test-py-simple-agents-streaming.py

# With image
uv run python test-py-simple-agents-invoice-image.py

# Streaming + Langfuse
uv run python test-py-simple-agents-streaming-langfuse.py

# Image + Jaeger
uv run python test-py-simple-agents-invoice-image-jaegar.py

# FastAPI server
uv run uvicorn fastapi_workflow_stream:app --reload
```

**TypeScript / Bun** (from `examples/napi-test-simpleAgents/`):

```bash
cd examples/napi-test-simpleAgents
bun install

# Normal run
bun run test-simple-agents.ts

# Streaming
bun run test-simple-agents-streaming.ts

# With image
bun run test-simple-agents-invoice-image.ts

# Streaming + Langfuse
bun run test-simple-agents-streaming-langfuse.ts

# Image + Jaeger
bun run test-simple-agents-invoice-image-jaegar.ts
```

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

Read `examples/python-test-simpleAgents/friendly.yaml` -- one `llm_call` node, no routing, plain text output.

Run it:

```bash
# In the streaming example, change the workflow_file line to point to friendly.yaml
```

**2. Add classification + routing + custom workers**

Read `examples/python-test-simpleAgents/test.yaml` -- multi-node email classification with:
- `llm_call` nodes for classification and extraction
- `switch` nodes for deterministic routing
- `custom_worker` node for stakeholder lookup
- `heal: true` and `stream: true` on all LLM nodes
- Template interpolation (`{{ nodes.X.output.Y }}`)

**3. Add streaming**

Compare `test-py-simple-agents.py` (blocking) with `test-py-simple-agents-streaming.py`:
- Add `WorkflowExecutionFlags(node_llm_streaming=True)`
- Use `client.stream_workflow()` with an `on_event` callback

**4. Add image input**

Compare `test-py-simple-agents.py` with `test-py-simple-agents-invoice-image.py`:
- Change `content` from a string to a list with `text` + `image_url` parts
- Base64-encode the image file

**5. Add observability**

Compare the base streaming example with `test-py-simple-agents-streaming-langfuse.py`:
- Configure OTLP env vars for Langfuse or Jaeger
- Add `WorkflowRunOptions(telemetry=WorkflowTelemetryConfig(enabled=True))`

### Skill-Builder Example YAMLs

Under `skills/simpleagents-builder/examples/`:

| File | Pattern |
|---|---|
| `python-intern-fun-interview-system.yaml` | Multi-turn interview loop with state detection, routing, and custom workers |
| `email-chat-draft-or-clarify.yaml` | Draft or clarify email flow with switch routing |

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
| Python | `examples/python-test-simpleAgents/test-py-simple-agents.py` | Normal workflow run |
| Python | `examples/python-test-simpleAgents/test-py-simple-agents-streaming.py` | Streaming workflow |
| Python | `examples/python-test-simpleAgents/test-py-simple-agents-invoice-image.py` | Image input |
| Python | `examples/python-test-simpleAgents/test-py-simple-agents-streaming-langfuse.py` | Langfuse tracing |
| Python | `examples/python-test-simpleAgents/test-py-simple-agents-invoice-image-jaegar.py` | Jaeger tracing |
| Python | `examples/python-test-simpleAgents/fastapi_workflow_stream.py` | FastAPI SSE server |
| TypeScript | `examples/napi-test-simpleAgents/test-simple-agents.ts` | Normal workflow run |
| TypeScript | `examples/napi-test-simpleAgents/test-simple-agents-streaming.ts` | Streaming workflow |
| TypeScript | `examples/napi-test-simpleAgents/test-simple-agents-invoice-image.ts` | Image input |
| TypeScript | `examples/napi-test-simpleAgents/test-simple-agents-streaming-langfuse.ts` | Langfuse tracing |
| TypeScript | `examples/napi-test-simpleAgents/test-simple-agents-invoice-image-jaegar.ts` | Jaeger tracing |
| Rust | `examples/full_api_example.rs` | Full Rust client API |
| Rust | `examples/python_client.py` | Python client API (completions, streaming, healing, tools) |
| Rust | `examples/node_client.js` | Node client API (completions, streaming) |
