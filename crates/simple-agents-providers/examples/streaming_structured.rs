//! Example demonstrating streaming structured outputs with progressive parsing.
//!
//! This example shows how to use StructuredStream to get structured output
//! from streaming responses, with automatic healing support.
//!
//! Note: This is a simplified example. Full provider integration for structured
//! streaming requires implementing ProviderStructuredExt trait.
//!
//! Run with:
//! ```bash
//! cargo run --example streaming_structured
//! ```

use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use simple_agents_providers::streaming_structured::{StructuredEvent, StructuredStream};
use simple_agent_type::response::{ChoiceDelta, CompletionChunk, FinishReason, MessageDelta};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalysisResult {
    sentiment: String,
    confidence: f32,
    key_points: Vec<String>,
}

#[tokio::main]
async fn main() {
    println!("=== Streaming Structured Output Example ===\n");

    // Simulate a streaming response that delivers JSON gradually
    let json_response = r#"{
  "sentiment": "positive",
  "confidence": 0.92,
  "key_points": [
    "Great product quality",
    "Fast shipping",
    "Excellent customer service"
  ]
}"#;

    // Split into chunks to simulate streaming
    let chunks: Vec<Result<CompletionChunk, simple_agent_type::error::SimpleAgentsError>> = vec![
        Ok(CompletionChunk {
            id: "chunk_1".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: Some(simple_agent_type::message::Role::Assistant),
                    content: Some(json_response[..50].to_string()),
                },
                finish_reason: None,
            }],
            created: None,
        }),
        Ok(CompletionChunk {
            id: "chunk_2".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: None,
                    content: Some(json_response[50..100].to_string()),
                },
                finish_reason: None,
            }],
            created: None,
        }),
        Ok(CompletionChunk {
            id: "chunk_3".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: None,
                    content: Some(json_response[100..].to_string()),
                },
                finish_reason: None,
            }],
            created: None,
        }),
        Ok(CompletionChunk {
            id: "chunk_4".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: None,
                    content: None,
                },
                finish_reason: Some(FinishReason::Stop),
            }],
            created: None,
        }),
    ];

    // Create JSON schema for validation
    let json_schema = json!({
        "type": "object",
        "properties": {
            "sentiment": {"type": "string"},
            "confidence": {"type": "number"},
            "key_points": {
                "type": "array",
                "items": {"type": "string"}
            }
        },
        "required": ["sentiment", "confidence", "key_points"]
    });

    println!("Streaming chunks and parsing structured output...\n");

    // Create structured stream (without healing for this demo)
    let stream = stream::iter(chunks);
    let mut structured: StructuredStream<_, AnalysisResult> =
        StructuredStream::new(stream, json_schema, None);

    // Process stream events
    let mut event_count = 0;
    while let Some(event) = structured.next().await {
        event_count += 1;

        match event {
            Ok(StructuredEvent::Partial(partial)) => {
                println!("📝 Partial update #{}", event_count);
                println!("   Sentiment: {}", partial.sentiment);
                println!("   Confidence: {:.2}", partial.confidence);
                println!("   Key points: {} items", partial.key_points.len());
                println!();
            }
            Ok(StructuredEvent::Complete {
                value,
                confidence,
                was_healed,
            }) => {
                println!("✅ Complete result (event #{})", event_count);
                println!("   Confidence: {:.2}%", confidence * 100.0);
                println!(
                    "   Healing applied: {}",
                    if was_healed { "Yes" } else { "No" }
                );
                println!();
                println!("Final analysis:");
                println!("   Sentiment: {}", value.sentiment);
                println!("   Confidence: {:.2}", value.confidence);
                println!("   Key points:");
                for (i, point) in value.key_points.iter().enumerate() {
                    println!("      {}. {}", i + 1, point);
                }
            }
            Err(e) => {
                eprintln!("❌ Error: {}", e);
                break;
            }
        }
    }

    println!();
    println!("=== Example Complete ===");
    println!();
    println!("💡 Key Features Demonstrated:");
    println!("   • Streaming JSON accumulation");
    println!("   • Structured output parsing");
    println!("   • Progressive updates (when valid)");
    println!("   • Final complete value with confidence");
    println!();
    println!("🔧 For Production Use:");
    println!("   • Enable healing: HealingIntegration::new(HealingConfig::default())");
    println!("   • Implement ProviderStructuredExt for your provider");
    println!("   • Handle partial updates in your UI");
    println!("   • Monitor confidence scores");
}
