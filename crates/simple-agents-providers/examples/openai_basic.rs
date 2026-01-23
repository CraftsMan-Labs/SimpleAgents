//! Basic OpenAI completion example.
//!
//! This example demonstrates how to use the OpenAI provider to make a simple
//! completion request.
//!
//! # Prerequisites
//!
//! Set your OpenAI API key:
//! ```bash
//! export OPENAI_API_KEY="sk-..."
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example openai_basic
//! ```

use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agents_types::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Get API key from environment
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY environment variable is required");
    let api_key = ApiKey::new(api_key)?;

    // Create OpenAI provider
    let provider = OpenAIProvider::new(api_key)?;

    println!("🤖 SimpleAgents - OpenAI Basic Example\n");

    // Build completion request
    let request = CompletionRequest::builder()
        .model("gpt-3.5-turbo")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("Explain what Rust is in one sentence."))
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    println!("📤 Sending request to OpenAI...");

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
    println!("  Tokens: {} prompt + {} completion = {} total",
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
        response.usage.total_tokens
    );

    Ok(())
}
