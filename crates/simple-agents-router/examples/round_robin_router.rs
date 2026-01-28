//! Round-robin router example.
//!
//! Demonstrates distributing requests across providers in a fixed order.
//!
//! # Run
//!
//! ```bash
//! cargo run -p simple-agents-router --example round_robin_router
//! ```

use async_trait::async_trait;
use simple_agents_router::RoundRobinRouter;
use simple_agent_type::prelude::*;
use std::sync::Arc;

struct MockProvider {
    name: &'static str,
}

impl MockProvider {
    fn new(name: &'static str) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        self.name
    }

    fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
        Ok(ProviderRequest::new("http://example.com"))
    }

    async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
        Ok(ProviderResponse::new(200, serde_json::Value::Null))
    }

    fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
        Ok(CompletionResponse {
            id: "resp_demo".to_string(),
            model: "demo-model".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("ok"),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage::new(1, 1),
            created: None,
            provider: Some(self.name().to_string()),
            healing_metadata: None,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let router = RoundRobinRouter::new(vec![
        Arc::new(MockProvider::new("provider-a")),
        Arc::new(MockProvider::new("provider-b")),
    ])?;

    let request = CompletionRequest::builder()
        .model("demo-model")
        .message(Message::user("Hello"))
        .build()?;

    let first = router.complete(&request).await?;
    let second = router.complete(&request).await?;

    println!(
        "First provider: {}",
        first.provider.as_deref().unwrap_or("unknown")
    );
    println!(
        "Second provider: {}",
        second.provider.as_deref().unwrap_or("unknown")
    );

    Ok(())
}
