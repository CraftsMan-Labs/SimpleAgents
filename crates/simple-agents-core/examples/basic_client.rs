use async_trait::async_trait;
use simple_agent_type::prelude::*;
use simple_agents_core::{
    CompletionOptions, CompletionOutcome, RoutingMode, SimpleAgentsClientBuilder,
};
use std::sync::Arc;

struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
        Ok(ProviderRequest::new("http://example.com"))
    }

    async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
        Ok(ProviderResponse::new(200, serde_json::json!({"ok": true})))
    }

    fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
        Ok(CompletionResponse {
            id: "resp_1".to_string(),
            model: "test-model".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("Hello from SimpleAgents"),
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

#[tokio::main]
async fn main() -> Result<()> {
    let client = SimpleAgentsClientBuilder::new()
        .with_provider(Arc::new(MockProvider))
        .with_routing_mode(RoutingMode::RoundRobin)
        .build()?;

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Say hi"))
        .build()?;

    let outcome = client
        .complete(&request, CompletionOptions::default())
        .await?;
    let response = match outcome {
        CompletionOutcome::Response(response) => response,
        _ => return Ok(()),
    };
    println!("{}", response.content().unwrap_or_default());

    Ok(())
}
