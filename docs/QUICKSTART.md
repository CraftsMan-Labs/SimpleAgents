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

## YAML <-> Code Workflow Migration

When a workflow starts as YAML, keep the same node ids and edges when moving to code-first canonical IR.

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

For a full conversion checklist (YAML -> code and code -> YAML), see [Workflow DSL Migration Cookbook](/WORKFLOW_DSL_MIGRATION_COOKBOOK).

## Next Steps

- Read [Usage Guide](/USAGE) for routing, streaming, caching, and healing.
- Explore [Examples](/EXAMPLES) for more workflows.
- Review [Rust Core Systems](/RUST_CORE_SYSTEMS) for crate map and extension points.
- Check [Capability Matrix](/CAPABILITY_MATRIX) for cross-language parity expectations.
- Use [Troubleshooting](/TROUBLESHOOTING) when setup or contract checks fail.
- Need a different path? Open the [Documentation Map](/DOCS_MAP).
