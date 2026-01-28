# simple-agents-core

Unified client API for SimpleAgents. This crate wires together providers, routing, caching,
healing, and middleware into a single entry point.

## Features

- **SimpleAgentsClient** with builder configuration
- **Routing** (round-robin, latency, cost, fallback)
- **Transparent caching** via `Cache` implementations
- **Healing helpers** for JSON parsing and schema coercion
- **Middleware hooks** for logging/metrics/tracing integrations

## Quick Start

```rust
use async_trait::async_trait;
use simple_agents_core::{RoutingMode, SimpleAgentsClientBuilder};
use simple_agent_type::prelude::*;
use std::sync::Arc;

struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str { "mock" }
    fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
        Ok(ProviderRequest::new("http://example.com"))
    }
    async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
        Ok(ProviderResponse::new(200, serde_json::Value::Null))
    }
    fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
        Ok(CompletionResponse {
            id: "resp_1".to_string(),
            model: "test-model".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("ok"),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage::new(1, 1),
            created: None,
            provider: Some("mock".to_string()),
            healing_metadata: None,
        })
    }
}

# async fn example() -> Result<()> {
let client = SimpleAgentsClientBuilder::new()
    .with_provider(Arc::new(MockProvider))
    .with_routing_mode(RoutingMode::RoundRobin)
    .build()?;

let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Hello"))
    .build()?;

let response = client.complete(&request).await?;
println!("{}", response.content().unwrap_or(""));
# Ok(())
# }
```

## Healing Helpers

Use healing for JSON outputs:

```rust
use simple_agents_healing::schema::Schema;
# use simple_agent_type::prelude::*;
# use simple_agents_core::SimpleAgentsClientBuilder;
# use std::sync::Arc;
# use async_trait::async_trait;
# struct MockProvider;
# #[async_trait]
# impl Provider for MockProvider {
#     fn name(&self) -> &str { "mock" }
#     fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
#         Ok(ProviderRequest::new("http://example.com"))
#     }
#     async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
#         Ok(ProviderResponse::new(200, serde_json::Value::Null))
#     }
#     fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
#         Ok(CompletionResponse {
#             id: "resp_1".to_string(),
#             model: "test-model".to_string(),
#             choices: vec![CompletionChoice {
#                 index: 0,
#                 message: Message::assistant("{\"count\": \"5\"}"),
#                 finish_reason: FinishReason::Stop,
#                 logprobs: None,
#             }],
#             usage: Usage::new(1, 1),
#             created: None,
#             provider: Some("mock".to_string()),
#             healing_metadata: None,
#         })
#     }
# }
# async fn example() -> Result<()> {
let client = SimpleAgentsClientBuilder::new()
    .with_provider(Arc::new(MockProvider))
    .build()?;

let schema = Schema::object(vec![("count".into(), Schema::Int, true)]);
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Give JSON"))
    .build()?;

let healed = client.complete_with_schema(&request, &schema).await?;
assert_eq!(healed.coerced.value["count"], 5);
# Ok(())
# }
```
