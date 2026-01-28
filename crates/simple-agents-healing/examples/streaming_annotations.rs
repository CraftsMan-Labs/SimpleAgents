//! Streaming with annotations example.
//!
//! This example demonstrates how to use stream annotations to control
//! when fields are emitted during streaming:
//!
//! - `Normal`: Emit as soon as available
//! - `NotNull`: Don't emit until non-null
//! - `Done`: Only emit when complete
//!
//! # Run
//!
//! ```bash
//! cargo run --example streaming_annotations
//! ```

use simple_agents_healing::schema::{Field, ObjectSchema, StreamAnnotation};
use simple_agents_healing::streaming::PartialExtractor;
use simple_agents_healing::Schema;

fn main() {
    println!("🏷️  SimpleAgents - Streaming Annotations Example\n");

    // Define schema with stream annotations
    let schema = ObjectSchema::new(vec![
        // ID should only be emitted when non-null
        Field::optional("id", Schema::String).with_stream_annotation(StreamAnnotation::NotNull),
        // Name can be emitted as soon as available
        Field::required("name", Schema::String).with_stream_annotation(StreamAnnotation::Normal),
        // Status should only be emitted when complete
        Field::required("status", Schema::String).with_stream_annotation(StreamAnnotation::Done),
        // Progress can be emitted normally
        Field::optional("progress", Schema::UInt).with_stream_annotation(StreamAnnotation::Normal),
    ]);

    println!("📋 Schema with annotations:");
    for field in &schema.fields {
        let annotation = match field.stream_annotation {
            StreamAnnotation::Normal => "Normal (emit immediately)",
            StreamAnnotation::NotNull => "NotNull (wait for non-null)",
            StreamAnnotation::Done => "Done (wait for complete)",
        };
        println!("   • {}: {}", field.name, annotation);
    }
    println!();

    // Simulate streaming chunks
    let chunks = [
        r#"{"id": null, "name": "Task 1""#,
        r#", "progress": 10"#,
        r#", "status": "in_progr"#,
        r#"ess", "id": "task_123"}"#,
    ];

    let mut extractor = PartialExtractor::new();

    println!("📥 Processing streaming chunks:\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    for (i, chunk) in chunks.iter().enumerate() {
        println!("\nChunk {}: {}", i + 1, chunk);

        // Feed chunk to extractor
        if let Some(partial) = extractor.feed(chunk) {
            println!("  Emitted fields:");

            // Check which fields should be emitted based on annotations
            // Get the field annotations from schema
            let id_annotation = schema
                .fields
                .iter()
                .find(|f| f.name == "id")
                .map(|f| f.stream_annotation)
                .unwrap_or(StreamAnnotation::Normal);

            let status_annotation = schema
                .fields
                .iter()
                .find(|f| f.name == "status")
                .map(|f| f.stream_annotation)
                .unwrap_or(StreamAnnotation::Normal);

            // Apply annotation logic
            if let Some(id) = partial.get("id") {
                match id_annotation {
                    StreamAnnotation::NotNull => {
                        if !id.is_null() {
                            println!("    ✓ id: {} (NotNull satisfied)", id);
                        } else {
                            println!("    ⏸ id: null (waiting for NotNull)");
                        }
                    }
                    _ => println!("    ✓ id: {}", id),
                }
            }

            if let Some(name) = partial.get("name") {
                // Normal: Emit immediately
                println!("    ✓ name: {} (Normal emission)", name);
            }

            if let Some(progress) = partial.get("progress") {
                // Normal: Emit immediately
                println!("    ✓ progress: {} (Normal emission)", progress);
            }

            if let Some(status) = partial.get("status") {
                match status_annotation {
                    StreamAnnotation::Done => {
                        // Done: Only emit when value is complete (heuristic: valid string)
                        if let Some(s) = status.as_str() {
                            if s.len() > 5 && !s.contains('"') {
                                println!("    ✓ status: {} (Done - complete)", status);
                            } else {
                                println!("    ⏸ status: {} (waiting for Done)", status);
                            }
                        }
                    }
                    _ => println!("    ✓ status: {}", status),
                }
            }
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Finalize stream
    match extractor.finalize() {
        Ok(value) => {
            println!("\n✅ Stream complete!");
            println!("   Final value: {}", value);
        }
        Err(e) => {
            println!("\n❌ Stream error: {}", e);
        }
    }

    println!("\n💡 Annotation Benefits:");
    println!("   • NotNull: Avoid emitting null placeholders");
    println!("   • Done: Wait for complete values (e.g., full sentences)");
    println!("   • Normal: Stream data as fast as possible");
    println!("   • Field-level control over emission timing");
}
