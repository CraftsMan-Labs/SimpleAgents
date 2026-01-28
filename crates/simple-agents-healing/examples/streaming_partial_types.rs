//! Streaming with partial types example.
//!
//! This example demonstrates how to use the streaming parser with partial types
//! to progressively extract structured data as it arrives from an LLM.
//!
//! # Run
//!
//! ```bash
//! cargo run --example streaming_partial_types
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use simple_agents_healing::streaming::StreamingParser;
use simple_agents_macros::PartialType;

/// Character information that we'll parse incrementally
#[derive(Debug, Clone, PartialType, Serialize, Deserialize)]
pub struct Character {
    /// Character's name
    pub name: String,
    /// Age in years
    pub age: u32,
    /// Character class/role
    pub class: String,
    /// List of abilities
    #[partial(default)]
    pub abilities: Vec<String>,
    /// Background story
    #[partial(default)]
    pub backstory: Option<String>,
}

fn main() {
    println!("🌊 SimpleAgents - Streaming Partial Types Example\n");

    // Simulate LLM streaming response in chunks
    let chunks = [
        r#"{"name": "Aria"#,
        r#", "age": 2"#,
        r#"5, "class": "Mage""#,
        r#", "abilities": ["Fireball","#,
        r#" "Ice Shield", "Teleport"]"#,
        r#", "backstory": "A mysterious"#,
        r#" wizard from the north"#,
        r#"}"#,
    ];

    let mut parser = StreamingParser::new();
    let mut partial_character = PartialCharacter::default();

    println!("📥 Receiving streaming response...\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Process each chunk
    for (i, chunk) in chunks.iter().enumerate() {
        println!("Chunk {}: {}", i + 1, chunk);

        // Feed chunk to parser
        parser.feed(chunk);

        // Try to get current partial value
        if let Some(current) = parser.try_parse() {
            // Extract partial fields from current JSON
            if let Some(name) = current.value.get("name").and_then(Value::as_str) {
                partial_character.name = Some(name.to_string());
            }
            if let Some(age) = current.value.get("age").and_then(Value::as_u64) {
                partial_character.age = Some(age as u32);
            }
            if let Some(class) = current.value.get("class").and_then(Value::as_str) {
                partial_character.class = Some(class.to_string());
            }
            if let Some(abilities) = current.value.get("abilities").and_then(Value::as_array) {
                let ability_strings: Vec<String> = abilities
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect();
                if !ability_strings.is_empty() {
                    partial_character.abilities = Some(ability_strings);
                }
            }
            if let Some(backstory_val) = current.value.get("backstory") {
                if let Some(backstory) = backstory_val.as_str() {
                    partial_character.backstory = Some(Some(backstory.to_string()));
                }
            }

            // Display current partial state
            println!("  └─ Partial: {:?}", partial_character);
        }

        println!();
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Finalize and get complete value
    match parser.finalize() {
        Ok(result) => {
            println!("✅ Stream complete!");
            println!("   Confidence: {:.2}", result.confidence);
            println!("   Flags: {} transformations", result.flags.len());

            // Convert partial to complete type
            match Character::from_partial(partial_character.clone()) {
                Ok(character) => {
                    println!("\n📊 Complete Character:");
                    println!("   Name: {}", character.name);
                    println!("   Age: {}", character.age);
                    println!("   Class: {}", character.class);
                    println!("   Abilities: {:?}", character.abilities);
                    if let Some(backstory) = character.backstory {
                        println!("   Backstory: {}", backstory);
                    }
                }
                Err(e) => {
                    println!("❌ Failed to convert partial to complete: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Stream error: {}", e);
        }
    }

    println!("\n💡 Key Benefits:");
    println!("   • Progressive data extraction as chunks arrive");
    println!("   • Type-safe partial values (Option<T> for all fields)");
    println!("   • Merge multiple chunks into single partial value");
    println!("   • Convert to complete type when ready");
}
