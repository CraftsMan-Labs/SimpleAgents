//! Mock provider implementation example.
//!
//! Shows how to implement the Provider trait.
//!
//! Run with: cargo run --example mock_provider

use async_trait::async_trait;
use simple_agents_types::prelude::*;
use std::time::Duration;

/// A mock provider for testing/demonstration.
struct MockProvider {
    name: String,
    api_key: ApiKey,
}

impl MockProvider {
    fn new(name: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            name: name.into(),
            api_key: ApiKey::new(api_key.into())?,
        })
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        println!("  🔄 Transforming request for {}", self.name);
        println!("     Model: {}", req.model);
        println!("     Messages: {}", req.messages.len());

        // Build provider-specific request
        let provider_req = ProviderRequest::new("https://api.example.com/v1/chat/completions")
            .with_header("Authorization", format!("Bearer {}", self.api_key.expose()))
            .with_header("Content-Type", "application/json")
            .with_body(serde_json::json!({
                "model": req.model,
                "messages": req.messages,
                "temperature": req.temperature,
                "max_tokens": req.max_tokens,
            }));

        Ok(provider_req)
    }

    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
        println!("  📡 Executing request to {}", req.url);
        println!("     Headers: {}", req.headers.len());

        // Mock successful response
        let response = ProviderResponse::new(
            200,
            serde_json::json!({
                "id": "mock_resp_123",
                "model": "mock-model",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "This is a mock response!"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15
                }
            }),
        );

        Ok(response)
    }

    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
        println!("  🔄 Transforming response (status: {})", resp.status);

        if !resp.is_success() {
            return Err(ProviderError::ServerError(format!("HTTP {}", resp.status)).into());
        }

        // Parse provider response
        let body = resp.body;
        let response = CompletionResponse {
            id: body["id"].as_str().unwrap_or("unknown").to_string(),
            model: body["model"].as_str().unwrap_or("unknown").to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant(
                    body["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or(""),
                ),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage {
                prompt_tokens: body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: body["usage"]["completion_tokens"].as_u64().unwrap_or(0)
                    as u32,
                total_tokens: body["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            },
            created: None,
            provider: Some(self.name.clone()),
        };

        Ok(response)
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: true,
            function_calling: true,
            vision: false,
            max_tokens: 4096,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Mock Provider Demo ===\n");

    // Create provider
    let provider = MockProvider::new("mock-ai", "sk-1234567890abcdefghijklmnopqrstuvwxyz")?;

    println!("📦 Provider: {}", provider.name());
    println!("⚙️  Capabilities:");
    println!("   Streaming: {}", provider.capabilities().streaming);
    println!("   Function calling: {}", provider.capabilities().function_calling);
    println!("   Max tokens: {}", provider.capabilities().max_tokens);
    println!();

    // Create request
    let request = CompletionRequest::builder()
        .model("mock-model-v1")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("Hello!"))
        .temperature(0.7)
        .build()?;

    println!("📤 Original request:");
    println!("   Model: {}", request.model);
    println!("   Messages: {}", request.messages.len());
    println!();

    // Transform request
    let provider_req = provider.transform_request(&request)?;
    println!();

    // Execute
    let provider_resp = provider.execute(provider_req).await?;
    println!();

    // Transform response
    let response = provider.transform_response(provider_resp)?;
    println!();

    println!("📥 Final response:");
    println!("   ID: {}", response.id);
    println!("   Content: {:?}", response.content());
    println!("   Usage: {} total tokens", response.usage.total_tokens);
    println!("   Provider: {:?}", response.provider);

    println!("\n=== Demo completed successfully! ===");
    Ok(())
}
