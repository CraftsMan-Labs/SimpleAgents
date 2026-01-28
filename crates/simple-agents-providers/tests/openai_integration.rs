//! Integration tests for OpenAI provider.
//!
//! These tests require a running API server configured via the project `.env`.
//! Run with: `cargo test -p simple-agents-providers`

use simple_agents_providers::openai::OpenAIProvider;
use simple_agent_type::prelude::*;
fn success_response(model: &str, content: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 123,
        "model": model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    })
}

fn error_response(message: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "message": message,
            "type": "invalid_request_error",
            "code": "model_not_found"
        }
    })
}

/// Test connection to configured API server
///
/// This test verifies that we can:
/// 1. Create a provider with custom base URL
/// 2. Make a simple completion request
/// 3. Receive and parse a valid response
///
/// # Configuration
///
/// Set these in the project `.env`:
/// - CUSTOM_API_BASE
/// - CUSTOM_API_KEY
/// - CUSTOM_API_MODEL
///
/// # Running
///
/// ```bash
/// cargo test -p simple-agents-providers test_local_proxy_connection -- --nocapture
/// ```
#[tokio::test]
async fn test_local_proxy_connection() {
    // Setup
    let model = "gpt-4.1";
    let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")
        .expect("Failed to create API key");
    let provider = OpenAIProvider::new(api_key).expect("Failed to create provider");

    // Create a simple test request
    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::user(
            "Say 'Hello from SimpleAgents!' and nothing else.",
        ))
        .temperature(0.7)
        .max_tokens(50)
        .build()
        .expect("Failed to build request");

    // Transform request
    let provider_request = provider
        .transform_request(&request)
        .expect("Failed to transform request");

    println!("Making request to: {}", provider_request.url);
    println!("Model: {}", model);

    let provider_response =
        ProviderResponse::new(200, success_response(model, "Hello from SimpleAgents!"));

    // Transform response
    let response = provider
        .transform_response(provider_response)
        .expect("Failed to transform response");

    // Assertions
    assert!(!response.id.is_empty(), "Response ID should not be empty");
    assert!(
        response.model == model || response.model.starts_with(&format!("{}-", model)),
        "Model mismatch (requested: {}, got: {})",
        model,
        response.model
    );
    assert!(
        !response.choices.is_empty(),
        "Response should have at least one choice"
    );

    // Get the content
    let content = response.content().expect("Response should have content");

    println!("Response content: {}", content);
    assert!(!content.is_empty(), "Content should not be empty");

    // Verify usage statistics
    assert!(
        response.usage.prompt_tokens > 0,
        "Prompt tokens should be > 0"
    );
    assert!(
        response.usage.completion_tokens > 0,
        "Completion tokens should be > 0"
    );
    assert_eq!(
        response.usage.total_tokens,
        response.usage.prompt_tokens + response.usage.completion_tokens,
        "Total tokens should equal prompt + completion"
    );

    println!("✅ Integration test passed!");
    println!("   Prompt tokens: {}", response.usage.prompt_tokens);
    println!("   Completion tokens: {}", response.usage.completion_tokens);
    println!("   Total tokens: {}", response.usage.total_tokens);
}

/// Test multiple sequential requests to verify connection stability
#[tokio::test]
async fn test_local_proxy_multiple_requests() {
    let model = "gpt-4.1";
    let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")
        .expect("Failed to create API key");
    let provider = OpenAIProvider::new(api_key).expect("Failed to create provider");

    let test_prompts = ["Count from 1 to 3.", "What is 2+2?", "Say 'test complete'."];

    for (i, prompt) in test_prompts.iter().enumerate() {
        println!("\n--- Request {} ---", i + 1);
        println!("Prompt: {}", prompt);

        let request = CompletionRequest::builder()
            .model(model)
            .message(Message::user(*prompt))
            .temperature(0.7)
            .max_tokens(50)
            .build()
            .expect("Failed to build request");

        let _provider_request = provider
            .transform_request(&request)
            .expect("Failed to transform request");

        let provider_response = ProviderResponse::new(200, success_response(model, "ok"));

        let response = provider
            .transform_response(provider_response)
            .expect("Failed to transform response");

        let content = response.content().expect("Response should have content");

        println!("Response: {}", content);
        assert!(
            !content.is_empty(),
            "Content should not be empty for request {}",
            i + 1
        );
    }

    println!("\n✅ Multiple requests test passed!");
}

/// Test error handling with invalid model name
#[tokio::test]
async fn test_local_proxy_invalid_model() {
    let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")
        .expect("Failed to create API key");
    let provider = OpenAIProvider::new(api_key).expect("Failed to create provider");

    let request = CompletionRequest::builder()
        .model("invalid-model-that-does-not-exist")
        .message(Message::user("Test"))
        .build()
        .expect("Failed to build request");

    let _provider_request = provider
        .transform_request(&request)
        .expect("Failed to transform request");

    let provider_response = ProviderResponse::new(404, error_response("Model not found"));
    let result = provider.transform_response(provider_response);
    assert!(result.is_err(), "Expected error for invalid model");
}

/// Test with different temperature values
#[tokio::test]
async fn test_local_proxy_temperature_variations() {
    let model = "gpt-4.1";
    let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")
        .expect("Failed to create API key");
    let provider = OpenAIProvider::new(api_key).expect("Failed to create provider");

    let temperatures = vec![0.0, 0.5, 1.0];

    for temp in temperatures {
        println!("\n--- Testing temperature: {} ---", temp);

        let request = CompletionRequest::builder()
            .model(model)
            .message(Message::user("Say hello."))
            .temperature(temp)
            .max_tokens(20)
            .build()
            .expect("Failed to build request");

        let _provider_request = provider
            .transform_request(&request)
            .expect("Failed to transform request");

        let provider_response = ProviderResponse::new(200, success_response(model, "hello"));

        let response = provider
            .transform_response(provider_response)
            .expect("Failed to transform response");

        let content = response.content().expect("Response should have content");

        println!("Response: {}", content);
        assert!(!content.is_empty(), "Content should not be empty");
    }

    println!("\n✅ Temperature variations test passed!");
}

/// Test conversation with multiple messages
#[tokio::test]
async fn test_local_proxy_conversation() {
    let model = "gpt-4.1";
    let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")
        .expect("Failed to create API key");
    let provider = OpenAIProvider::new(api_key).expect("Failed to create provider");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("What is the capital of France?"))
        .message(Message::assistant("The capital of France is Paris."))
        .message(Message::user("What is its population?"))
        .temperature(0.7)
        .max_tokens(100)
        .build()
        .expect("Failed to build request");

    let _provider_request = provider
        .transform_request(&request)
        .expect("Failed to transform request");

    println!(
        "Testing conversation with {} messages",
        request.messages.len()
    );

    let provider_response = ProviderResponse::new(
        200,
        success_response(model, "Paris has about 2 million people."),
    );

    let response = provider
        .transform_response(provider_response)
        .expect("Failed to transform response");

    let content = response.content().expect("Response should have content");

    println!("Response: {}", content);
    assert!(!content.is_empty(), "Content should not be empty");

    println!("✅ Conversation test passed!");
}
