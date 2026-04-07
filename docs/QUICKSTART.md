# Rust Quick Start

First successful SimpleAgents request using the Rust client.

> **Looking for Python or TypeScript?** Go to [Workflow Quickstart](/WORKFLOW_QUICKSTART).

## Prerequisites

- Rust toolchain (`stable`)
- An API key (OpenAI used below)

## 1. Add dependencies

```toml
[dependencies]
simple-agent-type = "0.3.8"
simple-agents-core = "0.3.8"
simple-agents-providers = "0.3.8"
tokio = { version = "1.35", features = ["full"] }
```

## 2. Set your API key

```bash
export OPENAI_API_KEY="sk-your-key-here"
```

## 3. Write your first request

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

## 4. Run it

```bash
cargo run
```

## Next Steps

- [Rust Usage Guide](/USAGE) -- routing, streaming, healing, caching
- [Workflow Quickstart](/WORKFLOW_QUICKSTART) -- YAML workflows in Python/TypeScript
- [Examples](/EXAMPLES) -- all runnable examples
- [Troubleshooting](/TROUBLESHOOTING) -- common issues
