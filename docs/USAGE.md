# Usage Guide

This guide shows the core runtime patterns of SimpleAgents with the unified Rust client.
By the end, you will know how to use completion, routing, streaming, schema coercion, and cache in production-oriented flows.

## Prerequisites

- Rust toolchain installed
- One provider API key configured (examples use OpenAI)
- Familiarity with the setup from [Quick Start](/QUICKSTART)

## Quick Path: Unified Client Baseline

Add core crates to `Cargo.toml`:

```toml
[dependencies]
simple-agent-type = "0.3.11"
simple-agents-core = "0.3.11"
simple-agents-providers = "0.3.11"
simple-agents-healing = "0.3.11" # optional (schemas/healing config)
tokio = { version = "1.35", features = ["full"] }
futures-util = "0.3"
```

Minimum completion flow:

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
        .message(Message::user("Hello from SimpleAgents"))
        .build()?;

    let outcome = client.complete(&request, CompletionOptions::default()).await?;
    if let CompletionOutcome::Response(response) = outcome {
        println!("{}", response.content().unwrap_or_default());
    }

    Ok(())
}
```

## Core Patterns

### Route across providers

Use multiple providers with an explicit routing strategy:

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
    .with_routing_mode(RoutingMode::RoundRobin)
    .build()?;
```

### Stream responses

```rust
use futures_util::StreamExt;
use simple_agent_type::prelude::*;
use simple_agents_core::{CompletionOptions, CompletionOutcome};

let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Stream me a short story"))
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

### Heal and coerce to schema

```rust
use simple_agent_type::prelude::*;
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, HealingSettings,
    SimpleAgentsClientBuilder,
};
use simple_agents_healing::schema::Schema;

let client = SimpleAgentsClientBuilder::new()
    .with_provider(provider)
    .with_healing_settings(HealingSettings::default())
    .build()?;

let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Return JSON with name and age"))
    .build()?;

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

### Add response cache

```rust
use simple_agents_cache::InMemoryCache;
use simple_agents_core::SimpleAgentsClientBuilder;
use std::sync::Arc;
use std::time::Duration;

let cache = Arc::new(InMemoryCache::new(10 * 1024 * 1024, 1000));

let client = SimpleAgentsClientBuilder::new()
    .with_provider(provider)
    .with_cache(cache)
    .with_cache_ttl(Duration::from_secs(3600))
    .build()?;
```

## Advanced: Direct Provider Access

Use provider-specific execution only when you intentionally bypass unified-client behaviors.

```rust
use simple_agent_type::prelude::*;
use simple_agents_providers::OpenAiCompatProvider;

let provider = OpenAiCompatProvider::new(ApiKey::new("sk-...")?)?;
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Hello"))
    .build()?;

let provider_request = provider.transform_request(&request)?;
let provider_response = provider.execute(provider_request).await?;
let response = provider.transform_response(provider_response)?;
```

## Troubleshooting

### Unexpected non-stream response

If your code expects a stream, ensure `.stream(true)` is set on the request before calling `complete`.

### Schema coercion does not trigger

Confirm `CompletionOptions.mode` is set to `CompletionMode::CoercedSchema(...)` and you handle `CompletionOutcome::CoercedSchema`.

### Routing does not appear active

Confirm the client is built with `with_providers(...)` and an explicit `with_routing_mode(...)`.

## Next Steps

- [Workflow Quickstart](/WORKFLOW_QUICKSTART) -- YAML workflows in Python/TypeScript
- [Python Binding](/BINDINGS_PYTHON) -- Python API details
- [Node.js Binding](/BINDINGS_NODE) -- TypeScript API details
- [Examples](/EXAMPLES) -- all runnable examples
