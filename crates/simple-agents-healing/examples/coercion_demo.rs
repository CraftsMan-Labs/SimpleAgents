//! Demonstration of the type coercion engine.
//!
//! Shows how the coercion engine handles:
//! - String to number coercion
//! - Fuzzy field matching (case-insensitive, snake_case ↔ camelCase)
//! - Default value injection
//! - Union type resolution
//! - Confidence scoring

use serde_json::json;
use simple_agents_healing::prelude::*;

fn main() {
    println!("=== SimpleAgents Coercion Engine Demo ===\n");

    // Example 1: String to number coercion
    example_1_string_to_number();

    // Example 2: Fuzzy field matching
    example_2_fuzzy_field_matching();

    // Example 3: Default value injection
    example_3_default_values();

    // Example 4: Union resolution
    example_4_union_resolution();

    // Example 5: Combined parser + coercion
    example_5_parser_with_coercion();

    // Example 6: Low confidence threshold
    example_6_confidence_threshold();
}

fn example_1_string_to_number() {
    println!("Example 1: String to Number Coercion");
    println!("=====================================");

    let engine = CoercionEngine::new();

    // String "42" should coerce to int 42
    let input = json!({"age": "25", "score": 98.5, "count": "100"});
    let schema = Schema::object(vec![
        ("age".into(), Schema::Int, true),
        ("score".into(), Schema::Float, true),
        ("count".into(), Schema::Int, true),
    ]);

    let result = engine.coerce(&input, &schema).unwrap();

    println!("Input:  {}", input);
    println!("Output: {}", result.value);
    println!("Confidence: {:.2}", result.confidence);
    println!("Flags: {:#?}", result.flags);
    println!();
}

fn example_2_fuzzy_field_matching() {
    println!("Example 2: Fuzzy Field Matching");
    println!("================================");

    let engine = CoercionEngine::new();

    // Mismatched field names that should be matched
    let input = json!({
        "FIRSTNAME": "Alice",      // Case mismatch
        "last_name": "Smith",      // snake_case vs camelCase
        "usrName": "alice123",     // Typo (should match userName)
        "emailAdress": "alice@example.com"  // Typo (should match emailAddress)
    });

    let schema = Schema::object(vec![
        ("firstName".into(), Schema::String, true),
        ("lastName".into(), Schema::String, true),
        ("userName".into(), Schema::String, true),
        ("emailAddress".into(), Schema::String, true),
    ]);

    let result = engine.coerce(&input, &schema).unwrap();

    println!("Input:  {}", input);
    println!("Output: {}", result.value);
    println!("Confidence: {:.2}", result.confidence);
    println!("Fuzzy matches:");
    for flag in &result.flags {
        if let CoercionFlag::FuzzyFieldMatch { expected, found } = flag {
            println!("  - '{}' matched to '{}'", found, expected);
        }
    }
    println!();
}

fn example_3_default_values() {
    println!("Example 3: Default Value Injection");
    println!("===================================");

    let engine = CoercionEngine::new();

    // Input missing optional fields
    let input = json!({
        "name": "Alice"
    });

    let schema = Schema::Object(ObjectSchema {
        fields: vec![
            Field::required("name", Schema::String),
            Field::optional("age", Schema::Int).with_default(json!(25)),
            Field::optional("active", Schema::Bool).with_default(json!(true)),
            Field::optional("score", Schema::Float).with_default(json!(0.0)),
        ],
        allow_additional_fields: false,
    });

    let result = engine.coerce(&input, &schema).unwrap();

    println!("Input:  {}", input);
    println!("Output: {}", result.value);
    println!("Confidence: {:.2}", result.confidence);
    println!("Default values used:");
    for flag in &result.flags {
        if let CoercionFlag::UsedDefaultValue { field } = flag {
            println!("  - {}", field);
        }
    }
    println!();
}

fn example_4_union_resolution() {
    println!("Example 4: Union Type Resolution");
    println!("=================================");

    let engine = CoercionEngine::new();

    // Schema accepts either Int or String
    let schema = Schema::union(vec![Schema::Int, Schema::String]);

    // Case 1: Pure int
    let result = engine.coerce(&json!(42), &schema).unwrap();
    println!("Input: 42 (int)");
    println!(
        "Output: {} (confidence: {:.2})",
        result.value, result.confidence
    );

    // Case 2: Pure string
    let result = engine.coerce(&json!("hello"), &schema).unwrap();
    println!("Input: \"hello\" (string)");
    println!(
        "Output: {} (confidence: {:.2})",
        result.value, result.confidence
    );

    // Case 3: Ambiguous (string that could be int)
    let result = engine.coerce(&json!("123"), &schema).unwrap();
    println!("Input: \"123\" (string)");
    println!(
        "Output: {} (confidence: {:.2})",
        result.value, result.confidence
    );
    println!("  Note: String is exact match (1.0), Int requires coercion (0.9)");
    println!();
}

fn example_5_parser_with_coercion() {
    println!("Example 5: Parser + Coercion Integration");
    println!("=========================================");

    let parser = JsonishParser::new();
    let engine = CoercionEngine::new();

    // Malformed JSON with type mismatches
    let malformed = r#"```json
    {
        "name": "Bob",
        "AGE": "30",         // String instead of int, wrong case
        "is_active": true,
        "score": 95.5
    }
    ```"#;

    let schema = Schema::object(vec![
        ("name".into(), Schema::String, true),
        ("age".into(), Schema::Int, true),
        ("isActive".into(), Schema::Bool, true),
        ("score".into(), Schema::Float, true),
    ]);

    // Step 1: Parse the malformed JSON
    let parse_result = parser.parse(malformed).unwrap();
    println!("Step 1 - Parse malformed JSON:");
    println!("  Confidence: {:.2}", parse_result.confidence);
    println!("  Parser flags: {} fixes", parse_result.flags.len());

    // Step 2: Coerce to schema
    let coerce_result = engine.coerce(&parse_result.value, &schema).unwrap();
    println!("\nStep 2 - Coerce to schema:");
    println!("  Output: {}", coerce_result.value);
    println!("  Confidence: {:.2}", coerce_result.confidence);
    println!(
        "  Total transformations: {}",
        parse_result.flags.len() + coerce_result.flags.len()
    );
    println!();
}

fn example_6_confidence_threshold() {
    println!("Example 6: Confidence Threshold");
    println!("================================");

    // Engine with strict confidence requirement
    let config = CoercionConfig {
        min_confidence: 0.95,
        ..Default::default()
    };
    let engine = CoercionEngine::with_config(config);

    let input = json!({
        "NAME": "Alice",     // Case mismatch (-0.05)
        "AGE": "30"          // Type coercion (-0.10)
    });

    let schema = Schema::object(vec![
        ("name".into(), Schema::String, true),
        ("age".into(), Schema::Int, true),
    ]);

    match engine.coerce(&input, &schema) {
        Ok(result) => {
            println!("Success: {}", result.value);
            println!("Confidence: {:.2}", result.confidence);
        }
        Err(HealingError::LowConfidence {
            confidence,
            threshold,
        }) => {
            println!("❌ Coercion failed!");
            println!("  Confidence: {:.2}", confidence);
            println!("  Threshold:  {:.2}", threshold);
            println!("  Reason: Too many transformations required");
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
    println!();
}
