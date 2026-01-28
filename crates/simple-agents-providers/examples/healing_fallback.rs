//! Example demonstrating automatic healing fallback for structured outputs.
//!
//! This example shows how the healing system automatically recovers from
//! malformed LLM responses when native structured output parsing fails.
//!
//! Run with:
//! ```bash
//! OPENAI_API_KEY=your_key cargo run --example healing_fallback
//! ```

use serde_json::json;
use simple_agents_providers::healing_integration::HealingConfig;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agent_type::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    println!("=== Healing System Fallback Example ===\n");

    // Create provider with healing enabled
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable required");
    let api_key = ApiKey::new(api_key)?;

    // Configure healing with default settings
    let healing_config = HealingConfig::default();
    println!("Healing config:");
    println!("  - Enabled: {}", healing_config.enabled);
    println!("  - Min confidence: {}", healing_config.min_confidence);
    println!();

    let provider = OpenAIProvider::new(api_key)?.with_healing(healing_config);

    // Define a JSON schema for structured output
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "Name of the person"
            },
            "age": {
                "type": "integer",
                "description": "Age in years"
            },
            "hobbies": {
                "type": "array",
                "items": {"type": "string"},
                "description": "List of hobbies"
            },
            "email": {
                "type": "string",
                "description": "Email address"
            }
        },
        "required": ["name", "age"],
        "additionalProperties": false
    });

    // Create request with JSON schema response format
    let request = CompletionRequest::builder()
        .model("gpt-4o-mini")
        .message(Message::system(
            "You are a helpful assistant that extracts person information from text. \
             Always respond with valid JSON matching the schema.",
        ))
        .message(Message::user(
            "Extract information about this person: \
             John Doe is 30 years old. He enjoys reading, hiking, and photography. \
             You can reach him at john.doe@example.com",
        ))
        .json_schema("PersonInfo", schema)
        .build()?;

    println!("Sending request with JSON schema...");
    println!("Model: {}", request.model);
    println!();

    // Execute request using three-phase provider pattern
    let result = async {
        let provider_request = provider.transform_request(&request)?;
        let provider_response = provider.execute(provider_request).await?;
        provider.transform_response(provider_response)
    }
    .await;

    match result {
        Ok(response) => {
            println!("✓ Response received successfully");
            println!();

            // Check if healing was applied
            if response.was_healed() {
                println!("⚕️  HEALING APPLIED");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

                if let Some(metadata) = &response.healing_metadata {
                    println!("Confidence: {:.2}%", metadata.confidence * 100.0);
                    println!("Original error: {}", metadata.original_error);
                    println!();

                    println!("Transformations applied:");
                    for (i, flag) in metadata.flags.iter().enumerate() {
                        println!("  {}. {}", i + 1, flag.description());
                    }

                    if metadata.has_major_coercions() {
                        println!();
                        println!("⚠️  Warning: Major coercions were applied");
                        println!("   Review the output carefully for correctness");
                    }
                }
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!();
            } else {
                println!("✓ Native parsing succeeded (no healing needed)");
                println!("  Confidence: 100%");
                println!();
            }

            // Display the result
            if let Some(content) = response.content() {
                println!("Extracted data:");
                println!("{}", content);
                println!();

                // Try to parse as JSON to show structured output
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
                    println!("Parsed structure:");
                    println!("{}", serde_json::to_string_pretty(&parsed)?);
                }
            }

            // Show usage statistics
            println!();
            println!("Usage:");
            println!("  Prompt tokens: {}", response.usage.prompt_tokens);
            println!("  Completion tokens: {}", response.usage.completion_tokens);
            println!("  Total tokens: {}", response.usage.total_tokens);
        }
        Err(e) => {
            eprintln!("✗ Request failed: {}", e);
            eprintln!();
            eprintln!("Possible reasons:");
            eprintln!("  - API key is invalid or expired");
            eprintln!("  - Network connectivity issues");
            eprintln!("  - Healing confidence below threshold");
            eprintln!("  - Rate limit exceeded");
            return Err(e);
        }
    }

    println!();
    println!("=== Example Complete ===");

    Ok(())
}
