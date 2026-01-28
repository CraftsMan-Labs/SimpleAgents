//! Basic OpenRouter completion example.
//!
//! This example demonstrates how to use the OpenRouter provider to access
//! multiple open-source and commercial models through a unified API.
//!
//! # Prerequisites
//!
//! Set your OpenRouter API key:
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example openrouter_basic
//! ```

use simple_agents_providers::openrouter::OpenRouterProvider;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    dotenv::dotenv().ok();

    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Get API key from environment
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable is required");
    let api_key = ApiKey::new(api_key)?;

    // Create OpenRouter provider
    let provider = OpenRouterProvider::new(api_key)?;

    println!("🤖 SimpleAgents - OpenRouter Basic Example\n");

    // Build completion request with model prefix
    // OpenRouter supports various model prefixes:
    // - openai/gpt-4
    // - anthropic/claude-3-opus
    // - meta-llama/llama-2-70b-chat
    // - etc.
    let request = CompletionRequest::builder()
        .model("openai/gpt-3.5-turbo") // Model with provider prefix
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("Explain what OpenRouter is in one sentence."))
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    println!("📤 Sending request to OpenRouter...");
    println!("   Model: openai/gpt-3.5-turbo");

    // Execute the three-phase provider pattern
    // Phase 1: Transform to provider format
    let provider_request = provider.transform_request(&request)?;

    // Phase 2: Execute HTTP request
    let provider_response = provider.execute(provider_request).await?;

    // Phase 3: Transform to unified format
    let response = provider.transform_response(provider_response)?;

    // Print response
    println!("\n✅ Response received:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", response.content().unwrap_or("No content"));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Print metadata
    println!("\n📊 Metadata:");
    println!("  Model: {}", response.model);
    println!(
        "  Tokens: {} prompt + {} completion = {} total",
        response.usage.prompt_tokens, response.usage.completion_tokens, response.usage.total_tokens
    );

    println!("\n💡 Try other models:");
    println!("   - anthropic/claude-3-opus");
    println!("   - meta-llama/llama-2-70b-chat");
    println!("   - google/palm-2-chat-bison");

    Ok(())
}
