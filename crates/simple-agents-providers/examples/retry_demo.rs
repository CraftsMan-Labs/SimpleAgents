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
//! Optionally override the base URL (proxy/router/local):
//! ```bash
//! export OPENAI_API_BASE="http://localhost:4000/v1"
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
use simple_agent_type::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    dotenv::dotenv().ok();

    // Initialize tracing to see retry logs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create OpenAI provider (reads OPENAI_API_KEY and optional OPENAI_API_BASE)
    let provider = OpenAIProvider::from_env()?;

    println!("🤖 SimpleAgents - Retry Logic Demo\n");

    let model = std::env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

    // Build completion request
    let request = CompletionRequest::builder()
        .model(&model)
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
    )
    .await;

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
