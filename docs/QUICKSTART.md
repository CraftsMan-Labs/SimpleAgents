# Quick Start Guide

Get up and running with SimpleAgents in a few minutes.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
simple-agent-type = "0.2.4"
simple-agents-core = "0.2.4"
simple-agents-providers = "0.2.4"
tokio = { version = "1.35", features = ["full"] }
```

## First Request

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

## Next Steps

- Read [Usage Guide](/USAGE) for routing, streaming, caching, and healing.
- Explore [Examples](/EXAMPLES) for more workflows.
- Review [Rust Core Systems](/RUST_CORE_SYSTEMS) for crate map and extension points.
- Check [Capability Matrix](/CAPABILITY_MATRIX) for cross-language parity expectations.
- Use [Troubleshooting](/TROUBLESHOOTING) when setup or contract checks fail.
- Need a different path? Open the [Documentation Map](/DOCS_MAP).
