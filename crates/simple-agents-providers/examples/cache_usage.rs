//! Cache usage example.
//!
//! This example demonstrates how to use the in-memory cache to avoid
//! duplicate API calls for identical requests.
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
//! cargo run --example cache_usage
//! ```

use simple_agents_cache::InMemoryCache;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agent_type::cache::{Cache, CacheKey};
use simple_agent_type::prelude::*;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    dotenv::dotenv().ok();

    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Create OpenAI provider (reads OPENAI_API_KEY and optional OPENAI_API_BASE)
    let provider = OpenAIProvider::from_env()?;

    // Create in-memory cache (10MB max, 100 entries max)
    let cache = InMemoryCache::new(10 * 1024 * 1024, 100);

    println!("🤖 SimpleAgents - Cache Usage Example\n");

    let model = std::env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

    // Build completion request
    let request = CompletionRequest::builder()
        .model(&model)
        .message(Message::user("What is 2+2?"))
        .temperature(0.7)
        .max_tokens(50)
        .build()?;

    // Generate cache key
    let cache_key = CacheKey::from_parts(
        provider.name(),
        &request.model,
        &serde_json::to_string(&request.messages)?,
    );

    println!("📦 Cache key: {}", cache_key);

    // First request - cache miss
    println!("\n🔍 First request (cache miss expected)...");
    let start = Instant::now();

    let response = if let Some(cached_data) = cache.get(&cache_key).await? {
        println!("✅ Cache HIT! (unexpected on first request)");
        serde_json::from_slice(&cached_data)?
    } else {
        println!("❌ Cache MISS (expected)");

        // Execute request
        let provider_request = provider.transform_request(&request)?;
        let provider_response = provider.execute(provider_request).await?;
        let response = provider.transform_response(provider_response)?;

        // Cache the response
        let response_bytes = serde_json::to_vec(&response)?;
        cache
            .set(&cache_key, response_bytes, Duration::from_secs(300))
            .await?;
        println!("💾 Cached response for 5 minutes");

        response
    };

    let first_duration = start.elapsed();

    println!("\n📄 Response:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", response.content().unwrap_or("No content"));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⏱️  Duration: {:?}", first_duration);

    // Second request - cache hit
    println!("\n🔍 Second request (cache hit expected)...");
    let start = Instant::now();

    let response2 = if let Some(cached_data) = cache.get(&cache_key).await? {
        println!("✅ Cache HIT!");
        serde_json::from_slice(&cached_data)?
    } else {
        println!("❌ Cache MISS (unexpected)");

        // This shouldn't happen
        let provider_request = provider.transform_request(&request)?;
        let provider_response = provider.execute(provider_request).await?;
        provider.transform_response(provider_response)?
    };

    let second_duration = start.elapsed();

    println!("\n📄 Cached response:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", response2.content().unwrap_or("No content"));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⏱️  Duration: {:?}", second_duration);

    // Performance comparison
    println!("\n📊 Performance Comparison:");
    println!("  First request (API call):  {:?}", first_duration);
    println!("  Second request (cached):   {:?}", second_duration);
    let speedup = first_duration.as_millis() as f64 / second_duration.as_millis() as f64;
    println!("  Speedup: {:.1}x faster", speedup);

    println!("\n💡 Note: The cached response is typically 100-1000x faster");
    println!("   than making an actual API call.");

    Ok(())
}
