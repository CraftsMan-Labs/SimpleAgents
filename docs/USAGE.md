# Usage Guide

This guide covers core usage of SimpleAgents with the unified Rust client.

## Installation

Add the core crates to your `Cargo.toml`:

```toml
[dependencies]
simple-agent-type = "0.2.4"
simple-agents-core = "0.2.4"
simple-agents-providers = "0.2.4"
simple-agents-cache = "0.2.4" # optional
simple-agents-healing = "0.2.4" # optional (schemas/healing config)
tokio = { version = "1.35", features = ["full"] }
futures-util = "0.3"
```

## Basic Usage (Unified Client)

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
        .message(Message::user("Hello from SimpleAgents"))
        .build()?;

    let outcome = client.complete(&request, CompletionOptions::default()).await?;
    if let CompletionOutcome::Response(response) = outcome {
        println!("{}", response.content().unwrap_or_default());
    }

    Ok(())
}
```

## Routing with Multiple Providers

```rust
use simple_agent_type::prelude::*;
use simple_agents_core::{RoutingMode, SimpleAgentsClientBuilder};
use simple_agents_providers::{anthropic::AnthropicProvider, openai::OpenAIProvider};
use std::sync::Arc;

let openai = Arc::new(OpenAIProvider::new(ApiKey::new("sk-...")?)?);
let anthropic = Arc::new(AnthropicProvider::new(ApiKey::new("sk-...")?)?);

let client = SimpleAgentsClientBuilder::new()
    .with_providers(vec![openai, anthropic])
    .with_routing_mode(RoutingMode::RoundRobin)
    .build()?;
```

## Streaming

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

## Healing and Schema Coercion

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

## Caching

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

## Direct Provider Usage (Advanced)

If you need to bypass the unified client, you can call providers directly. This is useful for testing or custom orchestration, but most users should prefer `simple-agents-core`.

```rust
use simple_agent_type::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

let provider = OpenAIProvider::new(ApiKey::new("sk-...")?)?;
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Hello"))
    .build()?;

let provider_request = provider.transform_request(&request)?;
let provider_response = provider.execute(provider_request).await?;
let response = provider.transform_response(provider_response)?;
```

## Next Steps

- Review [Rust Core Systems](/RUST_CORE_SYSTEMS) for crate responsibilities.
- See [Examples](/EXAMPLES) for more examples.
- Check [API Surface](/API) for the API surface map.
- Binding guides: [Python](/BINDINGS_PYTHON), [Node.js / TypeScript](/BINDINGS_NODE), [Go](/BINDINGS_GO).
- Need a task-oriented index? Use the [Documentation Map](/DOCS_MAP).
