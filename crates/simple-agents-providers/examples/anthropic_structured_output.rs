//! Example: Anthropic Claude Structured Outputs
//!
//! This example demonstrates using Anthropic's structured output feature
//! with JSON schema validation (available in Claude Sonnet 4.5+ models).
//!
//! ## Usage
//! ```bash
//! export ANTHROPIC_API_KEY="sk-ant-..."
//! cargo run --example anthropic_structured_output
//! ```
//!
//! ## Requirements
//! - Anthropic API key
//! - Model: claude-sonnet-4.5, claude-opus-4.5, claude-haiku-4.5
//!
//! ## Features Demonstrated
//! - Native JSON schema validation
//! - Guaranteed structured output format
//! - Beta feature usage

use serde_json::json;
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup API key
    let api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY environment variable not set");
    let api_key = ApiKey::new(api_key)?;

    // Create provider
    let provider = AnthropicProvider::new(api_key)?;

    println!("=== Anthropic Claude Structured Outputs Demo ===\n");

    // Example 1: Simple Structured Output
    simple_structured_output(&provider).await?;

    println!("\n{}\n", "=".repeat(60));

    // Example 2: Complex Nested Schema
    complex_nested_schema(&provider).await?;

    Ok(())
}

/// Example 1: Simple Structured Output
async fn simple_structured_output(provider: &AnthropicProvider) -> Result<()> {
    println!("1. Simple Structured Output with JSON Schema");
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
                "description": "The user's email address"
            },
            "is_premium": {
                "type": "boolean",
                "description": "Whether the user has a premium account"
            }
        },
        "required": ["name", "age", "email", "is_premium"]
    });

    let request = CompletionRequest::builder()
        .model("claude-sonnet-4.5")
        .message(Message::system(
            "Generate user profiles matching the schema.",
        ))
        .message(Message::user(
            "Create a profile for Maria Garcia, a 35-year-old premium user",
        ))
        .json_schema("user_profile", schema)
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

/// Example 2: Complex Nested Schema
async fn complex_nested_schema(provider: &AnthropicProvider) -> Result<()> {
    println!("2. Complex Nested Schema");
    println!("{}", "-".repeat(60));

    // Define a complex schema with nested structures
    let schema = json!({
        "type": "object",
        "properties": {
            "company": {
                "type": "string",
                "description": "Company name"
            },
            "industry": {
                "type": "string",
                "enum": ["technology", "finance", "healthcare", "retail", "manufacturing"]
            },
            "employees": {
                "type": "array",
                "description": "List of employees",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "role": { "type": "string" },
                        "department": {
                            "type": "string",
                            "enum": ["engineering", "product", "design", "sales", "marketing"]
                        },
                        "skills": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["name", "role", "department", "skills"]
                },
                "minItems": 2
            }
        },
        "required": ["company", "industry", "employees"]
    });

    let request = CompletionRequest::builder()
        .model("claude-sonnet-4.5")
        .message(Message::system(
            "Generate company data matching the schema.",
        ))
        .message(Message::user(
            "Create a technology company with 3 employees from different departments",
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
    assert!(json_output.get("industry").is_some());
    assert!(json_output
        .get("employees")
        .and_then(|e| e.as_array())
        .is_some());

    let employees = json_output["employees"].as_array().unwrap();
    assert!(employees.len() >= 2, "Should have at least 2 employees");

    println!("\n✓ Complex nested structure validated successfully!");

    Ok(())
}
