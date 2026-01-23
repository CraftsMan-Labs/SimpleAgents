//! Retry logic demonstration.
//!
//! This example demonstrates how the retry logic works with exponential backoff
//! when handling transient failures.
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
//! cargo run --example retry_demo
//! ```

use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::retry::execute_with_retry;
use simple_agents_providers::Provider;
use simple_agents_types::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to see retry logs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Get API key from environment
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY environment variable is required");
    let api_key = ApiKey::new(api_key)?;

    // Create OpenAI provider
    let provider = OpenAIProvider::new(api_key)?;

    println!("🤖 SimpleAgents - Retry Logic Demo\n");

    // Build completion request
    let request = CompletionRequest::builder()
        .model("gpt-3.5-turbo")
        .message(Message::user("Say 'hello'"))
        .max_tokens(10)
        .build()?;

    println!("📤 Making request with retry logic...");
    println!("   Max attempts: 3");
    println!("   Initial backoff: 100ms");
    println!("   Backoff multiplier: 2.0");
    println!("   Jitter: ±30%\n");

    // Get retry configuration from provider
    let retry_config = provider.retry_config();

    // Execute with retry
    let result = execute_with_retry(
        &retry_config,
        Some(provider.name()),
        |e| {
            // Determine if error is retryable
            if let SimpleAgentsError::Provider(provider_error) = e {
                provider_error.is_retryable()
            } else {
                false
            }
        },
        || {
            // Clone provider and request for each attempt
            let provider = provider.clone();
            let request = request.clone();

            async move {
                // Execute three-phase pattern
                let provider_request = provider.transform_request(&request)?;
                let provider_response = provider.execute(provider_request).await?;
                let response = provider.transform_response(provider_response)?;
                Ok(response)
            }
        },
    ).await;

    match result {
        Ok(response) => {
            println!("\n✅ Success!");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", response.content().unwrap_or("No content"));
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        }
        Err(e) => {
            println!("\n❌ Failed after retries: {}", e);
        }
    }

    println!("\n💡 Note: Check the debug logs above to see retry attempts");
    println!("   and backoff durations (if any failures occurred).");

    Ok(())
}
