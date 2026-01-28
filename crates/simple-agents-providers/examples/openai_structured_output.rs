//! Example: OpenAI Native Structured Outputs
//!
//! This example demonstrates using OpenAI's native structured output feature
//! with JSON schema validation (no coercion/healing needed).
//!
//! ## Usage
//! ```bash
//! export OPENAI_API_KEY="sk-..."
//! cargo run --example openai_structured_output
//! ```
//!
//! ## Requirements
//! - OpenAI API key
//! - Model: gpt-4o-mini, gpt-4o, or gpt-4-turbo (structured outputs supported)
//!
//! ## Features Demonstrated
//! - Native JSON schema validation
//! - Guaranteed structured output format
//! - No need for healing/coercion
//! - Type-safe schema definition

use serde_json::json;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup API key
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");
    let api_key = ApiKey::new(api_key)?;

    // Create provider
    let provider = OpenAIProvider::new(api_key)?;

    println!("=== OpenAI Native Structured Outputs Demo ===\n");

    // Example 1: Simple JSON Object Mode
    simple_json_mode(&provider).await?;

    println!("\n{}\n", "=".repeat(60));

    // Example 2: Structured Output with JSON Schema
    structured_output_with_schema(&provider).await?;

    println!("\n{}\n", "=".repeat(60));

    // Example 3: Complex Nested Schema
    complex_nested_schema(&provider).await?;

    Ok(())
}

/// Example 1: Simple JSON Object Mode
/// This mode ensures the output is valid JSON but doesn't validate the structure.
async fn simple_json_mode(provider: &OpenAIProvider) -> Result<()> {
    println!("1. JSON Object Mode (no schema validation)");
    println!("{}", "-".repeat(60));

    let request = CompletionRequest::builder()
        .model("gpt-4o-mini")
        .message(Message::system(
            "You are a helpful assistant that outputs JSON.",
        ))
        .message(Message::user(
            "Generate a user profile with name, age, and email",
        ))
        .json_mode() // Enable JSON object mode
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    let json_output: serde_json::Value = serde_json::from_str(response.content().unwrap_or(""))?;

    println!("Response:");
    println!("{}", serde_json::to_string_pretty(&json_output)?);

    Ok(())
}

/// Example 2: Structured Output with JSON Schema
/// This mode validates the output against a JSON schema, guaranteeing the structure.
async fn structured_output_with_schema(provider: &OpenAIProvider) -> Result<()> {
    println!("2. Structured Output with JSON Schema");
    println!("{}", "-".repeat(60));

    // Define JSON schema for the response
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "The user's full name"
            },
            "age": {
                "type": "integer",
                "description": "The user's age in years",
                "minimum": 0,
                "maximum": 150
            },
            "email": {
                "type": "string",
                "format": "email",
                "description": "The user's email address"
            },
            "is_premium": {
                "type": "boolean",
                "description": "Whether the user has a premium account"
            }
        },
        "required": ["name", "age", "email", "is_premium"],
        "additionalProperties": false
    });

    let request = CompletionRequest::builder()
        .model("gpt-4o-mini")
        .message(Message::system(
            "Generate a user profile matching the schema.",
        ))
        .message(Message::user(
            "Create a profile for Sarah Chen, a 28-year-old premium user",
        ))
        .json_schema("user_profile", schema) // Use structured output with schema
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    let json_output: serde_json::Value = serde_json::from_str(response.content().unwrap_or(""))?;

    println!("Response (validated against schema):");
    println!("{}", serde_json::to_string_pretty(&json_output)?);

    // The output is guaranteed to match the schema
    assert!(json_output.get("name").is_some());
    assert!(json_output.get("age").is_some());
    assert!(json_output.get("email").is_some());
    assert!(json_output.get("is_premium").is_some());

    println!("\n✓ Output validated against schema successfully!");

    Ok(())
}

/// Example 3: Complex Nested Schema
/// Demonstrates structured output with nested objects and arrays.
async fn complex_nested_schema(provider: &OpenAIProvider) -> Result<()> {
    println!("3. Complex Nested Schema");
    println!("{}", "-".repeat(60));

    // Define a complex schema with nested structures
    let schema = json!({
        "type": "object",
        "properties": {
            "company": {
                "type": "string",
                "description": "Company name"
            },
            "employees": {
                "type": "array",
                "description": "List of employees",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "role": { "type": "string" },
                        "salary": { "type": "number" },
                        "skills": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["name", "role", "salary", "skills"],
                    "additionalProperties": false
                },
                "minItems": 1
            },
            "total_budget": {
                "type": "number",
                "description": "Total salary budget"
            }
        },
        "required": ["company", "employees", "total_budget"],
        "additionalProperties": false
    });

    let request = CompletionRequest::builder()
        .model("gpt-4o-mini")
        .message(Message::system("Generate company data matching the schema."))
        .message(Message::user(
            "Create a tech company with 3 employees: a senior engineer, a product manager, and a designer. Include realistic salaries and skills."
        ))
        .json_schema("company_data", schema)
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    let json_output: serde_json::Value = serde_json::from_str(response.content().unwrap_or(""))?;

    println!("Response (complex nested structure):");
    println!("{}", serde_json::to_string_pretty(&json_output)?);

    // Validate structure
    assert!(json_output.get("company").is_some());
    assert!(json_output
        .get("employees")
        .and_then(|e| e.as_array())
        .is_some());
    assert!(json_output.get("total_budget").is_some());

    let employees = json_output["employees"].as_array().unwrap();
    assert!(employees.len() >= 3, "Should have at least 3 employees");

    println!("\n✓ Complex nested structure validated successfully!");

    Ok(())
}
