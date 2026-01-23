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
//!
//! # Run
//!
//! ```bash
//! cargo run --example streaming
//! ```

use futures_util::StreamExt;
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

    println!("🤖 SimpleAgents - Streaming Example\n");

    // Build streaming completion request
    let request = CompletionRequest::builder()
        .model("gpt-3.5-turbo")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("Write a haiku about Rust programming."))
        .temperature(0.7)
        .max_tokens(100)
        .stream(true)  // Enable streaming
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
