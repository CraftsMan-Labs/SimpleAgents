# Quick Start Guide

This guide gets you from zero to your first successful SimpleAgents request using the unified Rust client.
By the end, you will send a prompt to a provider and print the model output locally.

## Prerequisites

- Rust toolchain installed (`stable` recommended)
- A valid provider key (`OPENAI_API_KEY` used below)
- A new or existing Rust binary project

## Quick Path

### 1. Add dependencies

Update your `Cargo.toml`:

```toml
[dependencies]
simple-agent-type = "0.2.4"
simple-agents-core = "0.2.4"
simple-agents-providers = "0.2.4"
tokio = { version = "1.35", features = ["full"] }
```

### 2. Configure environment variable

```sh
export OPENAI_API_KEY="<your-api-key>"
```

### 3. Send your first request

Create `src/main.rs`:

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

### 4. Run it

```sh
cargo run
```

If setup is correct, you should see a model-generated response in your terminal.

## Mental Model

SimpleAgents separates concerns into three parts:

1. **Provider**: converts your request to provider-specific API calls.
2. **Unified client**: applies shared behaviors like routing, cache, and healing.
3. **Outcome**: returns either standard response, stream, or schema-coerced output.

For deeper architecture boundaries, read [Architecture](/ARCHITECTURE) and [Rust Core Systems](/RUST_CORE_SYSTEMS).

## YAML <-> Code Workflow Migration (Optional)

When migrating workflows, keep node ids and edge semantics stable to preserve behavior and replay traceability.

```yaml
name: review-flow
version: v0
nodes:
  - id: start
    type: start
    next: route
  - id: route
    type: condition
    expression: "$.state.urgent == true"
    on_true: fast_path
    on_false: normal_path
  - id: fast_path
    type: tool
    tool: quick_review
    next: end
  - id: normal_path
    type: llm
    model: gpt-4.1-mini
    prompt: "Review request"
    next: end
  - id: end
    type: end
```

```rust
use serde_json::json;
use simple_agents_workflow::ir::{Node, NodeKind, WorkflowDefinition};

let workflow = WorkflowDefinition {
    version: "v0".to_string(),
    name: "review-flow".to_string(),
    nodes: vec![
        Node {
            id: "start".to_string(),
            kind: NodeKind::Start {
                next: "route".to_string(),
            },
        },
        Node {
            id: "route".to_string(),
            kind: NodeKind::Condition {
                expression: "$.state.urgent == true".to_string(),
                on_true: "fast_path".to_string(),
                on_false: "normal_path".to_string(),
            },
        },
        Node {
            id: "fast_path".to_string(),
            kind: NodeKind::Tool {
                tool: "quick_review".to_string(),
                input: json!({}),
                next: Some("end".to_string()),
            },
        },
        Node {
            id: "normal_path".to_string(),
            kind: NodeKind::Llm {
                model: "gpt-4.1-mini".to_string(),
                prompt: "Review request".to_string(),
                next: Some("end".to_string()),
            },
        },
        Node {
            id: "end".to_string(),
            kind: NodeKind::End,
        },
    ],
};
```

For full bidirectional migration guidance, see [Workflow DSL Migration Cookbook](/WORKFLOW_DSL_MIGRATION_COOKBOOK).

## Troubleshooting

### Missing API key

If you see authentication or key validation errors, verify the variable is set in the same shell session:

```sh
echo "$OPENAI_API_KEY"
```

### Provider/model mismatch

If a model name is rejected, switch to a model supported by the selected provider and retry.

### Build fails on dependencies

Refresh your lockfile and rebuild:

```sh
cargo update && cargo build
```

## Next Steps

- Learn multi-provider routing and streaming in [Usage Guide](/USAGE).
- Explore end-to-end snippets in [Examples](/EXAMPLES).
- Review crate boundaries in [Rust Core Systems](/RUST_CORE_SYSTEMS).
- Use [Troubleshooting](/TROUBLESHOOTING) for setup and contract issues.
- Need role-based navigation? Open [Documentation Map](/DOCS_MAP).
