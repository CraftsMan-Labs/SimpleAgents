//! Basic example demonstrating JSON healing capabilities.
//!
//! Run with: cargo run --example basic_healing

use simple_agents_healing::prelude::*;

fn main() {
    println!("=== SimpleAgents Response Healing Demo ===\n");

    let parser = JsonishParser::new();

    // Example 1: Perfect JSON
    println!("1. Perfect JSON:");
    let perfect = r#"{"name": "Alice", "age": 30}"#;
    demonstrate_parse(&parser, perfect);

    // Example 2: Markdown-wrapped JSON
    println!("\n2. Markdown-wrapped JSON:");
    let markdown = r#"```json
{
    "name": "Bob",
    "age": 25,
    "skills": ["Rust", "Python"]
}
```"#;
    demonstrate_parse(&parser, markdown);

    // Example 3: Trailing commas
    println!("\n3. Trailing commas:");
    let trailing = r#"{"name": "Charlie", "hobbies": ["reading", "coding",],}"#;
    demonstrate_parse(&parser, trailing);

    // Example 4: Single quotes
    println!("\n4. Single quotes:");
    let quotes = r#"{'name': 'Diana', 'city': 'NYC'}"#;
    demonstrate_parse(&parser, quotes);

    // Example 5: Multiple issues
    println!("\n5. Multiple issues combined:");
    let complex = r#"```json
{'name': 'Eve', 'skills': ['AI', 'ML',],}
```"#;
    demonstrate_parse(&parser, complex);

    // Example 6: Confidence threshold
    println!("\n6. Confidence threshold test:");
    let strict_config = ParserConfig {
        min_confidence: 0.96,
        ..Default::default()
    };
    let strict_parser = JsonishParser::with_config(strict_config);

    match strict_parser.parse(markdown) {
        Ok(_) => println!("✓ Passed strict threshold"),
        Err(e) => println!("✗ Failed strict threshold: {}", e),
    }

    println!("\n=== Demo Complete ===");
}

fn demonstrate_parse(parser: &JsonishParser, input: &str) {
    println!("Input:");
    println!("{}", input);
    println!();

    match parser.parse(input) {
        Ok(result) => {
            println!("✓ Parse successful!");
            println!("  Confidence: {:.2}", result.confidence);
            println!(
                "  Value: {}",
                serde_json::to_string_pretty(&result.value).unwrap()
            );

            if !result.flags.is_empty() {
                println!("  Healing applied:");
                for flag in &result.flags {
                    println!("    - {}", flag.description());
                }
            } else {
                println!("  No healing needed (perfect JSON)");
            }
        }
        Err(e) => {
            println!("✗ Parse failed: {}", e);
        }
    }
}
