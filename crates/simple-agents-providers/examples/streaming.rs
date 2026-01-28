//! Streaming completion example.
//!
//! This example demonstrates how to use streaming with the OpenAI provider
//! to receive incremental responses as they're generated.
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
//! cargo run --example streaming
//! ```

use futures_util::StreamExt;
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

    println!("🤖 SimpleAgents - Streaming Example\n");

    let model = std::env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

    // Build streaming completion request
    let request = CompletionRequest::builder()
        .model(&model)
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("Write a haiku about Rust programming."))
        .temperature(0.7)
        .max_tokens(100)
        .stream(true) // Enable streaming
        .build()?;

    println!("📤 Streaming request to OpenAI...\n");

    // Transform and execute with streaming
    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    // Receive and print chunks as they arrive
    println!("📨 Receiving chunks:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                // Extract content from chunk
                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        // Print chunk as it arrives
                        print!("{}", content);
                        use std::io::Write;
                        std::io::stdout().flush().unwrap();

                        full_content.push_str(content);
                    }
                }
            }
            Err(e) => {
                eprintln!("\n❌ Stream error: {}", e);
                break;
            }
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Print summary
    println!("\n📊 Stream completed:");
    println!("  Chunks received: {}", chunk_count);
    println!("  Total length: {} characters", full_content.len());

    Ok(())
}
