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
//! Optionally override the base URL (proxy/router/local):
//! ```bash
//! export OPENAI_API_BASE="http://localhost:4000/v1"
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example openai_basic
//! ```

use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    dotenv::dotenv().ok();

    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Create OpenAI provider (reads OPENAI_API_KEY and optional OPENAI_API_BASE)
    let provider = OpenAIProvider::from_env()?;

    println!("🤖 SimpleAgents - OpenAI Basic Example\n");

    let model = std::env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

    // Build completion request
    let request = CompletionRequest::builder()
        .model(&model)
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
    println!(
        "  Tokens: {} prompt + {} completion = {} total",
        response.usage.prompt_tokens, response.usage.completion_tokens, response.usage.total_tokens
    );

    Ok(())
}
