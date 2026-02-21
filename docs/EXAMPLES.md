# Code Examples

Practical examples for using SimpleAgents with the unified client.

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

## Workflow YAML + Step Timings

Simple workflow execution from YAML is available through the Rust workflow crate and language bindings.

Rust core API:

```rust
use serde_json::json;
use simple_agents_workflow::run_workflow_yaml_file_with_client;

let output = run_workflow_yaml_file_with_client(
    std::path::Path::new("examples/workflow_email/email-intake-classification.yaml"),
    &json!({
        "email_text": "Termination request, second warning already issued",
        "messages": [
            {"role": "user", "content": "Termination request, second warning already issued"}
        ]
    }),
    &client,
)
.await?;

println!("terminal: {}", output.terminal_node);
println!("total_ms: {}", output.total_elapsed_ms);
for step in output.step_timings {
    println!("{} {}ms", step.node_id, step.elapsed_ms);
}
```

Cross-language runnable examples:

- Python: `examples/workflow_email/run_with_python_package.py`
- Python (chat history input): `examples/workflow_email/run_with_chat_history.py`
- Node (chat history input): `examples/workflow_email/node/run_with_chat_history.js`
- Node: `examples/workflow_email/run_with_node_package.js`
- Go: `bindings/go/examples/workflow_yaml/main.go`
- Go (chat history input): `bindings/go/examples/workflow_chat_history/main.go`
