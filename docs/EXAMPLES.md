# Code Examples

Use this page as a learning path instead of a dump of snippets.
If you are new, go in this order:

1. basic completion
2. simple YAML workflow
3. branching workflow
4. tool-calling workflow
5. multi-turn chat workflow

If you want the fastest workflow setup path, start with [Workflow Quickstart](/WORKFLOW_QUICKSTART).

## Basic Completion

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

## Routing Across Providers

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

## Streaming Responses

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

## Healed JSON

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

## Schema-Coerced JSON

```rust
use simple_agent_type::prelude::*;
use simple_agents_core::{CompletionMode, CompletionOptions, CompletionOutcome};
use simple_agents_healing::schema::Schema;

let schema = Schema::object(vec![
    ("name".into(), Schema::String, true),
    ("age".into(), Schema::Int, true),
]);

let options = CompletionOptions {
    mode: CompletionMode::CoercedSchema(schema),
};

match client.complete(&request, options).await? {
    CompletionOutcome::CoercedSchema(result) => {
        println!("{}", result.coerced.value);
    }
    _ => {}
}
```

## Cache Integration

```rust
use simple_agents_cache::InMemoryCache;
use simple_agents_core::SimpleAgentsClientBuilder;
use std::sync::Arc;
use std::time::Duration;

let cache = Arc::new(InMemoryCache::new(20 * 1024 * 1024, 2000));

let client = SimpleAgentsClientBuilder::new()
    .with_provider(provider)
    .with_cache(cache)
    .with_cache_ttl(Duration::from_secs(600))
    .build()?;
```

## Workflow Learning Path

These examples are ordered from easiest to more advanced.

### 1. Start Simple: Single-Node Workflow

Use this first:

- YAML: `examples/workflow_email/hr-warning-email-subgraph.yaml`
- Why start here: one `llm_call`, one schema, no routing, easy to read

Validate the graph shape first:

```bash
cargo run -p simple-agents-cli -- workflow mermaid examples/workflow_email/hr-warning-email-subgraph.yaml
```

Run it with the Python example runner:

```bash
uv run --directory examples python workflow_email/run_with_python_package.py \
  --workflow examples/workflow_email/hr-warning-email-subgraph.yaml \
  --email "Please draft a warning email for repeated tardiness."
```

### 2. Add Branching: Classifier -> Switch -> Action

Use this next:

- YAML: `examples/workflow_email/email-chat-draft-or-clarify.yaml`
- Pattern: detect state -> route with `switch` -> ask a question or draft an email
- Why it matters: this is the most reusable workflow pattern in the repo

Run it:

```bash
make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml
```

### 3. Add Tool Calling

Use this after you understand branching:

- YAML: `examples/workflow_email/email-chat-draft-with-tool-calling.yaml`
- Pattern: one `llm_call` node with a declared tool
- Why it matters: shows how to keep structured tool input/output inside YAML

Run it:

```bash
make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-with-tool-calling.yaml
```

Example prompt:

```text
Draft a warning email for Priya Sharma for repeated late submissions.
```

### 4. Move to Multi-Turn Chat Workflows

Use these when you want a conversation that keeps history across turns:

- Python: `examples/workflow_email/run_with_chat_history.py`
- Node: `examples/workflow_email/node/run_with_chat_history.js`
- Rust: `examples/workflow_chat_history_rust.rs`

Python runner:

```bash
make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml
```

Node runner:

```bash
make run-node-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml
```

### 5. Use the Rust Workflow API Directly

Simple workflow execution from YAML is available through the Rust workflow crate and language bindings.

Rust core API:

```rust
use serde_json::json;
use simple_agents_workflow::yaml_runner::{
    workflow_execution, YamlWorkflowExecutionFlags, YamlWorkflowExecutionRequest,
    YamlWorkflowExecutorBinding, YamlWorkflowRunOptions, YamlWorkflowSource,
};

let workflow_input = json!({
    "messages": [
        {"role": "user", "content": "Termination request, second warning already issued"}
    ]
});
let options = YamlWorkflowRunOptions::default();
let execution_request = YamlWorkflowExecutionRequest {
    source: YamlWorkflowSource::File(std::path::Path::new(
        "examples/workflow_email/email-unified-chat-intake-classification.yaml",
    )),
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

### Cross-Language Example Files

- Python: `examples/workflow_email/run_with_python_package.py`
- Python (chat history input): `examples/workflow_email/run_with_chat_history.py`
- Rust (chat history input): `examples/workflow_chat_history_rust.rs`
- Python (native YAML tool-calling warning email): `examples/workflow_email/email-chat-draft-with-tool-calling.yaml`
- Python (graph-to-graph tool call orchestrator): `examples/workflow_email/email-chat-orchestrator-with-subgraph-tool.yaml`
- Node (chat history input): `examples/workflow_email/node/run_with_chat_history.js`
- Node: `examples/workflow_email/run_with_node_package.js`

## Chat Workflow Commands

Use these after you understand the learning path above.

### Rust

```bash
make run-rust-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml
make run-rust-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml RUST_CHAT_FLAGS='--max-turns 1 --model gemini-3-flash'
```

### Python

```bash
make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml

# Override all llm_call node models for this run
make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml PY_CHAT_FLAGS='--max-turns 1 --model gemini-3-flash'

# Native YAML tool-calling workflow example
make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-with-tool-calling.yaml

# Example prompt in chat: "Draft a warning email for Priya Sharma for repeated late submissions"

# Parent graph delegates to subgraph via run_workflow_graph tool
make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-orchestrator-with-subgraph-tool.yaml
```

### Node or Bun

```bash
make run-node-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml
make run-node-chat-history JS_RUNTIME=bun WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml

# Override all llm_call node models for this run
make run-node-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml NODE_CHAT_FLAGS='--max-turns 1 --model gemini-3-flash'
```
