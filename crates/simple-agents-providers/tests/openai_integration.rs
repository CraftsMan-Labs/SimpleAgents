//! Integration tests for OpenAI provider.
//!
//! These tests require a running API server and are ignored by default.
//! Run with: `cargo test -p simple-agents-providers -- --ignored`

use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_types::prelude::*;

/// Test connection to local LLM proxy server
///
/// This test verifies that we can:
/// 1. Create a provider with custom base URL
/// 2. Make a simple completion request
/// 3. Receive and parse a valid response
///
/// # Configuration
///
/// - API Base: http://localhost:4000
/// - API Key: sk-1234
/// - Model: openai/xai/grok-code-fast-1
///
/// # Running
///
/// ```bash
/// cargo test -p simple-agents-providers test_local_proxy_connection -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore] // Requires local server running
async fn test_local_proxy_connection() {
    // Setup
    let api_key = ApiKey::new("sk-1234")
        .expect("Failed to create API key");

    let provider = OpenAIProvider::with_base_url(
        api_key,
        "http://localhost:4000".to_string(),
    )
    .expect("Failed to create provider");

    // Create a simple test request
    let request = CompletionRequest::builder()
        .model("openai/xai/grok-code-fast-1")
        .message(Message::user("Say 'Hello from SimpleAgents!' and nothing else."))
        .temperature(0.7)
        .max_tokens(50)
        .build()
        .expect("Failed to build request");

    // Transform request
    let provider_request = provider
        .transform_request(&request)
        .expect("Failed to transform request");

    println!("Making request to: {}", provider_request.url);
    println!("Model: openai/xai/grok-code-fast-1");

    // Execute request
    let provider_response = provider
        .execute(provider_request)
        .await
        .expect("Failed to execute request");

    println!("Response status: {}", provider_response.status);
    assert!(
        provider_response.is_success(),
        "Expected successful response, got status: {}",
        provider_response.status
    );

    // Transform response
    let response = provider
        .transform_response(provider_response)
        .expect("Failed to transform response");

    // Assertions
    assert!(!response.id.is_empty(), "Response ID should not be empty");
    assert_eq!(response.model, "openai/xai/grok-code-fast-1", "Model mismatch");
    assert!(!response.choices.is_empty(), "Response should have at least one choice");

    // Get the content
    let content = response
        .content()
        .expect("Response should have content");

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
#[ignore] // Requires local server running
async fn test_local_proxy_multiple_requests() {
    let api_key = ApiKey::new("sk-1234")
        .expect("Failed to create API key");

    let provider = OpenAIProvider::with_base_url(
        api_key,
        "http://localhost:4000".to_string(),
    )
    .expect("Failed to create provider");

    let test_prompts = vec![
        "Count from 1 to 3.",
        "What is 2+2?",
        "Say 'test complete'.",
    ];

    for (i, prompt) in test_prompts.iter().enumerate() {
        println!("\n--- Request {} ---", i + 1);
        println!("Prompt: {}", prompt);

        let request = CompletionRequest::builder()
            .model("openai/xai/grok-code-fast-1")
            .message(Message::user(*prompt))
            .temperature(0.7)
            .max_tokens(50)
            .build()
            .expect("Failed to build request");

        let provider_request = provider
            .transform_request(&request)
            .expect("Failed to transform request");

        let provider_response = provider
            .execute(provider_request)
            .await
            .expect("Failed to execute request");

        let response = provider
            .transform_response(provider_response)
            .expect("Failed to transform response");

        let content = response
            .content()
            .expect("Response should have content");

        println!("Response: {}", content);
        assert!(!content.is_empty(), "Content should not be empty for request {}", i + 1);
    }

    println!("\n✅ Multiple requests test passed!");
}

/// Test error handling with invalid model name
#[tokio::test]
#[ignore] // Requires local server running
async fn test_local_proxy_invalid_model() {
    let api_key = ApiKey::new("sk-1234")
        .expect("Failed to create API key");

    let provider = OpenAIProvider::with_base_url(
        api_key,
        "http://localhost:4000".to_string(),
    )
    .expect("Failed to create provider");

    let request = CompletionRequest::builder()
        .model("invalid-model-that-does-not-exist")
        .message(Message::user("Test"))
        .build()
        .expect("Failed to build request");

    let provider_request = provider
        .transform_request(&request)
        .expect("Failed to transform request");

    let result = provider.execute(provider_request).await;

    // This should fail with a model not found error or similar
    match result {
        Err(e) => {
            println!("✅ Expected error received: {}", e);
            // Check if it's a provider error
            assert!(
                format!("{}", e).contains("Provider") ||
                format!("{}", e).contains("Model") ||
                format!("{}", e).contains("error"),
                "Expected provider/model error, got: {}",
                e
            );
        }
        Ok(_) => {
            panic!("Expected error for invalid model, but request succeeded");
        }
    }
}

/// Test with different temperature values
#[tokio::test]
#[ignore] // Requires local server running
async fn test_local_proxy_temperature_variations() {
    let api_key = ApiKey::new("sk-1234")
        .expect("Failed to create API key");

    let provider = OpenAIProvider::with_base_url(
        api_key,
        "http://localhost:4000".to_string(),
    )
    .expect("Failed to create provider");

    let temperatures = vec![0.0, 0.5, 1.0];

    for temp in temperatures {
        println!("\n--- Testing temperature: {} ---", temp);

        let request = CompletionRequest::builder()
            .model("openai/xai/grok-code-fast-1")
            .message(Message::user("Say hello."))
            .temperature(temp)
            .max_tokens(20)
            .build()
            .expect("Failed to build request");

        let provider_request = provider
            .transform_request(&request)
            .expect("Failed to transform request");

        let provider_response = provider
            .execute(provider_request)
            .await
            .expect("Failed to execute request");

        let response = provider
            .transform_response(provider_response)
            .expect("Failed to transform response");

        let content = response
            .content()
            .expect("Response should have content");

        println!("Response: {}", content);
        assert!(!content.is_empty(), "Content should not be empty");
    }

    println!("\n✅ Temperature variations test passed!");
}

/// Test conversation with multiple messages
#[tokio::test]
#[ignore] // Requires local server running
async fn test_local_proxy_conversation() {
    let api_key = ApiKey::new("sk-1234")
        .expect("Failed to create API key");

    let provider = OpenAIProvider::with_base_url(
        api_key,
        "http://localhost:4000".to_string(),
    )
    .expect("Failed to create provider");

    let request = CompletionRequest::builder()
        .model("openai/xai/grok-code-fast-1")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("What is the capital of France?"))
        .message(Message::assistant("The capital of France is Paris."))
        .message(Message::user("What is its population?"))
        .temperature(0.7)
        .max_tokens(100)
        .build()
        .expect("Failed to build request");

    let provider_request = provider
        .transform_request(&request)
        .expect("Failed to transform request");

    println!("Testing conversation with {} messages", request.messages.len());

    let provider_response = provider
        .execute(provider_request)
        .await
        .expect("Failed to execute request");

    let response = provider
        .transform_response(provider_response)
        .expect("Failed to transform response");

    let content = response
        .content()
        .expect("Response should have content");

    println!("Response: {}", content);
    assert!(!content.is_empty(), "Content should not be empty");

    println!("✅ Conversation test passed!");
}
