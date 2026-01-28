//! Provider streaming with healing integration.
//!
//! This example demonstrates how to use the healing system with provider streaming
//! to parse structured data from LLM responses as they arrive.
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
//! cargo run --example streaming_with_healing
//! ```

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use simple_agents_healing::streaming::StreamingParser;
use simple_agents_macros::PartialType;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;

/// Recipe structure to parse from LLM response
#[derive(Debug, Clone, PartialType, Serialize, Deserialize)]
pub struct Recipe {
    /// Recipe name
    pub name: String,
    /// Cuisine type
    pub cuisine: String,
    /// Preparation time in minutes
    pub prep_time: u32,
    /// List of ingredients
    #[partial(default)]
    pub ingredients: Vec<String>,
    /// Cooking instructions
    #[partial(default)]
    pub instructions: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present
    dotenv::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create provider (reads OPENAI_API_KEY and optional OPENAI_API_BASE)
    let provider = OpenAIProvider::from_env()?;

    println!("🍳 SimpleAgents - Streaming with Healing Example\n");

    let model = std::env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

    // Create request asking for JSON recipe
    let request = CompletionRequest::builder()
        .model(&model)
        .message(Message::system(
            "You are a helpful cooking assistant. Always respond with valid JSON.",
        ))
        .message(Message::user(
            r#"Give me a recipe for pasta carbonara in JSON format:
            {
                "name": "recipe name",
                "cuisine": "cuisine type",
                "prep_time": minutes,
                "ingredients": ["ingredient 1", "ingredient 2", ...],
                "instructions": ["step 1", "step 2", ...]
            }"#,
        ))
        .temperature(0.7)
        .max_tokens(500)
        .stream(true)
        .build()?;

    println!("📤 Requesting recipe from OpenAI (streaming)...\n");

    // Transform and execute with streaming
    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    // Create healing parser for streaming JSON
    let mut healing_parser = StreamingParser::new();
    let mut partial_recipe = PartialRecipe::default();
    let mut full_content = String::new();

    println!("📥 Receiving and parsing chunks:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Process stream chunks
    let mut chunk_count = 0;
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                // Extract content from chunk
                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        chunk_count += 1;
                        full_content.push_str(content);

                        // Feed to healing parser
                        healing_parser.feed(content);

                        // Try to extract partial structured data
                        if let Some(current) = healing_parser.try_parse() {
                            // Update partial recipe from current JSON
                            if let Some(name) = current
                                .value
                                .get("name")
                                .and_then(serde_json::Value::as_str)
                            {
                                partial_recipe.name = Some(name.to_string());
                            }
                            if let Some(cuisine) = current
                                .value
                                .get("cuisine")
                                .and_then(serde_json::Value::as_str)
                            {
                                partial_recipe.cuisine = Some(cuisine.to_string());
                            }
                            if let Some(prep_time) = current
                                .value
                                .get("prep_time")
                                .and_then(serde_json::Value::as_u64)
                            {
                                partial_recipe.prep_time = Some(prep_time as u32);
                            }
                            if let Some(ingredients) = current
                                .value
                                .get("ingredients")
                                .and_then(serde_json::Value::as_array)
                            {
                                let items: Vec<String> = ingredients
                                    .iter()
                                    .filter_map(serde_json::Value::as_str)
                                    .map(String::from)
                                    .collect();
                                if !items.is_empty() {
                                    partial_recipe.ingredients = Some(items.clone());
                                    println!("  🥘 Ingredients ({} so far)", items.len());
                                }
                            }
                            if let Some(instructions) = current
                                .value
                                .get("instructions")
                                .and_then(serde_json::Value::as_array)
                            {
                                let steps: Vec<String> = instructions
                                    .iter()
                                    .filter_map(serde_json::Value::as_str)
                                    .map(String::from)
                                    .collect();
                                if !steps.is_empty() {
                                    partial_recipe.instructions = Some(steps.clone());
                                    println!("  📝 Instructions ({} steps so far)", steps.len());
                                }
                            }
                        }
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

    // Finalize healing parser
    match healing_parser.finalize() {
        Ok(result) => {
            println!("\n✅ Stream complete!");
            println!("   Chunks received: {}", chunk_count);
            println!("   Content length: {} characters", full_content.len());
            println!("   Confidence: {:.2}", result.confidence);

            // Display any healing transformations
            if !result.flags.is_empty() {
                println!("   Healing applied:");
                for flag in &result.flags {
                    println!("     • {:?}", flag);
                }
            }

            // Convert partial to complete recipe
            match Recipe::from_partial(partial_recipe.clone()) {
                Ok(recipe) => {
                    println!("\n📖 Complete Recipe:");
                    println!("   Name: {}", recipe.name);
                    println!("   Cuisine: {}", recipe.cuisine);
                    println!("   Prep Time: {} minutes", recipe.prep_time);
                    println!("\n   Ingredients:");
                    for (i, ingredient) in recipe.ingredients.iter().enumerate() {
                        println!("     {}. {}", i + 1, ingredient);
                    }
                    println!("\n   Instructions:");
                    for (i, instruction) in recipe.instructions.iter().enumerate() {
                        println!("     {}. {}", i + 1, instruction);
                    }
                }
                Err(e) => {
                    println!("\n⚠️  Could not convert to complete recipe: {}", e);
                    println!("   Partial data available:");
                    println!("   {:?}", partial_recipe);
                }
            }
        }
        Err(e) => {
            println!("\n❌ Parsing error: {}", e);
            println!("   Raw content:\n{}", full_content);
        }
    }

    println!("\n💡 Demonstrated Features:");
    println!("   ✓ Provider streaming (OpenAI)");
    println!("   ✓ Incremental JSON healing");
    println!("   ✓ Progressive structured data extraction");
    println!("   ✓ Partial type safety with PartialType");
    println!("   ✓ Graceful handling of malformed JSON");

    Ok(())
}
