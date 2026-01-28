//! Basic Anthropic (Claude) completion example.
//!
//! This example demonstrates how to use the Anthropic provider to make a simple
//! completion request with system message extraction.
//!
//! # Prerequisites
//!
//! Set your Anthropic API key:
//! ```bash
//! export ANTHROPIC_API_KEY="sk-ant-..."
//! ```
//! Optionally override the base URL (proxy/router/local):
//! ```bash
//! export ANTHROPIC_API_BASE="http://localhost:4000/v1"
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example anthropic_basic
//! ```

use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    dotenv::dotenv().ok();

    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Create Anthropic provider (reads ANTHROPIC_API_KEY and optional ANTHROPIC_API_BASE)
    let provider = AnthropicProvider::from_env()?;

    println!("🤖 SimpleAgents - Anthropic Basic Example\n");

    // Build completion request with system message
    // Note: System messages are automatically extracted by the Anthropic provider
    let request = CompletionRequest::builder()
        .model("claude-3-opus-20240229")
        .message(Message::system(
            "You are a helpful AI assistant specialized in technology.",
        ))
        .message(Message::user("Explain what Rust is in one sentence."))
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    println!("📤 Sending request to Anthropic (Claude)...");

    // Execute the three-phase provider pattern
    // Phase 1: Transform to provider format (extracts system messages)
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

    println!("\n💡 Note: System messages were automatically extracted and sent");
    println!("   in Anthropic's required format.");

    Ok(())
}
