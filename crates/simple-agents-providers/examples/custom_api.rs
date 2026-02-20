//! Custom API Endpoint Example
//!
//! This example demonstrates using SimpleAgents with custom OpenAI-compatible APIs.
//!
//! Features Demonstrated:
//! 1. **Custom API Configuration** - Use any OpenAI-compatible endpoint
//! 2. **Response Healing** - Parse and heal malformed JSON from LLMs
//! 3. **Type Coercion** - Convert values to proper types automatically
//! 4. **Streaming Support** - Real-time streaming from custom APIs
//! 5. **Multi-turn Conversations** - Context-aware dialogues
//! 6. **Metrics Collection** - Track token usage and latency
//!
//! # Use Cases
//!
//! - **Azure OpenAI Service** - Enterprise-grade Azure-hosted OpenAI models
//! - **Local LLM Servers** - Run models locally with OpenAI-compatible APIs
//!   - vLLM (https://github.com/vllm-project/vllm)
//!   - Ollama (with OpenAI-compatible endpoint)
//!   - text-generation-webui
//! - **Custom Proxy Servers** - Company-specific proxy servers
//! - **OpenRouter** - Multi-provider API (already has dedicated provider)
//!
//! # Prerequisites
//!
//! 1. Copy `.env.example` to `.env`
//! 2. Add your custom API base URL and key to `.env`
//!
//! ```bash
//! cp .env.example .env
//! # Edit .env and add your custom API details
//! ```
//!
//! # Setup Examples
//!
//! ## Azure OpenAI Service
//!
//! ```bash
//! # For Azure OpenAI, your base URL looks like:
//! CUSTOM_API_BASE=https://your-resource.openai.azure.com/openai/deployments/your-deployment
//! CUSTOM_API_KEY=your-azure-api-key
//! CUSTOM_API_MODEL=gpt-4
//! ```
//!
//! ## Local vLLM Server
//!
//! First, start a vLLM server:
//! ```bash
//! python -m vllm.entrypoints.openai.api_server --model meta-llama/Llama-2-70b-chat-hf
//! ```
//!
//! Then configure:
//! ```bash
//! CUSTOM_API_BASE=http://localhost:8000/v1
//! CUSTOM_API_KEY=dummy-key  # vLLM doesn't require auth by default
//! CUSTOM_API_MODEL=meta-llama/Llama-2-70b-chat-hf
//! ```
//!
//! ## Ollama with OpenAI Compatibility
//!
//! Start Ollama with OpenAI compatibility:
//! ```bash
//! OLLAMA_ORIGINS="*" ollama serve
//! ```
//!
//! Then configure:
//! ```bash
//! CUSTOM_API_BASE=http://localhost:11434/v1
//! CUSTOM_API_KEY=ollama
//! CUSTOM_API_MODEL=llama2
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example custom_api
//! ```

use simple_agent_type::prelude::*;
use simple_agents_healing::prelude::*;
use simple_agents_providers::metrics::RequestTimer;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use std::io::Write;
use std::time::Duration;

#[path = "../../../examples/shared/healing_showcase.rs"]
mod healing_showcase;
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║     SimpleAgents - Custom API Endpoint Demo              ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Load custom API configuration
    let api_base =
        std::env::var("CUSTOM_API_BASE").expect("CUSTOM_API_BASE environment variable not set");

    let api_key_str =
        std::env::var("CUSTOM_API_KEY").expect("CUSTOM_API_KEY environment variable not set");

    let model =
        std::env::var("CUSTOM_API_MODEL").expect("CUSTOM_API_MODEL environment variable not set");

    println!("📋 Configuration:");
    println!("  Base URL: {}", api_base);
    println!("  Model: {}", model);
    println!(
        "  API Key: {} (hidden for security)\n",
        mask_key(&api_key_str)
    );

    // Create HTTP client without HTTP/2 for local servers
    // (Local servers like vLLM, Ollama, and Grok often only support HTTP/1.1)
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| SimpleAgentsError::Config(format!("Failed to create HTTP client: {}", e)))?;

    // Create provider with custom client
    let api_key = ApiKey::new(api_key_str)?;
    let provider = OpenAIProvider::with_client(api_key, api_base, client)?;

    println!("✅ Provider created successfully\n");

    // Example 1: Simple completion
    println!("{}", "━".repeat(60));
    println!("Example 1: Simple Completion");
    println!("{}", "━".repeat(60));
    example_simple_completion(&provider, &model).await?;

    // Example 2: Streaming response
    println!("\n{}", "━".repeat(60));
    println!("Example 2: Streaming Response");
    println!("{}", "━".repeat(60));
    example_streaming(&provider, &model).await?;

    // Example 3: Multi-turn conversation
    println!("\n{}", "━".repeat(60));
    println!("Example 3: Multi-turn Conversation");
    println!("{}", "━".repeat(60));
    example_conversation(&provider, &model).await?;

    // Example 4: Response healing - JSON parsing
    println!("\n{}", "━".repeat(60));
    println!("Example 4: Response Healing - JSON Parsing");
    println!("{}", "━".repeat(60));
    example_response_healing(&provider, &model).await?;

    // Example 5: Type coercion
    println!("\n{}", "━".repeat(60));
    println!("Example 5: Type Coercion");
    println!("{}", "━".repeat(60));
    example_type_coercion(&provider, &model).await?;

    // Example 6: Fuzzy field matching
    println!("\n{}", "━".repeat(60));
    println!("Example 6: Fuzzy Field Matching");
    println!("{}", "━".repeat(60));
    example_fuzzy_matching(&provider, &model).await?;

    // Example 7: Streaming with healing
    println!("\n{}", "━".repeat(60));
    println!("Example 7: Streaming + Response Healing");
    println!("{}", "━".repeat(60));
    example_streaming_healing(&provider, &model).await?;

    // Example 8: Streaming structured output
    println!("\n{}", "━".repeat(60));
    println!("Example 8: Streaming Structured Output (Progressive JSON)");
    println!("{}", "━".repeat(60));
    example_streaming_structured(&provider, &model).await?;

    // Example 9: Streaming graph visualization
    println!("\n{}", "━".repeat(60));
    println!("Example 9: Streaming Graph Visualization (Progressive)");
    println!("{}", "━".repeat(60));
    example_streaming_graph(&provider, &model).await?;

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║                    Demo Complete!                         ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}

async fn example_simple_completion(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Sending simple completion request...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system("You are a helpful, concise assistant."))
        .message(Message::user(
            "What is Rust programming language? Answer in one sentence.",
        ))
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Response:");
    println!("{}", "━".repeat(60));
    println!("{}", response.content().unwrap_or("No content"));
    println!("{}", "━".repeat(60));

    println!("\n📊 Metrics:");
    println!(
        "  Tokens: {} prompt + {} completion = {} total",
        response.usage.prompt_tokens, response.usage.completion_tokens, response.usage.total_tokens
    );

    Ok(())
}

async fn example_streaming(provider: &OpenAIProvider, model: &str) -> Result<()> {
    use futures_util::StreamExt;

    println!("\n📤 Sending streaming request...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system("You are a creative assistant."))
        .message(Message::user("Write a very short poem about programming."))
        .temperature(0.8)
        .max_tokens(100)
        .stream(true) // Enable streaming
        .build()?;

    let timer = RequestTimer::start("custom-api", model);

    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    println!("📝 Streaming response:");
    println!("{}", "━".repeat(60));

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                        full_content.push_str(content);
                    }
                }
            }
            Err(e) => {
                eprintln!("\n❌ Stream error: {}", e);
                break;
            }
        }
    }

    println!("\n{}", "━".repeat(60));

    println!("\n📊 Metrics:");
    println!("  Chunks received: {}", chunk_count);
    println!("  Total length: {} characters", full_content.len());

    let estimated_tokens = (full_content.len() as f32 / 4.0) as u32;
    timer.complete_success(50, estimated_tokens);

    Ok(())
}

async fn example_conversation(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Starting multi-turn conversation...\n");

    let mut messages = vec![Message::system(
        "You are a math tutor. Be helpful and encouraging.",
    )];

    // First turn
    println!("User: What is 2 + 2?");
    messages.push(Message::user("What is 2 + 2?"));

    let request = CompletionRequest::builder()
        .model(model)
        .messages(messages.clone())
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    let assistant_reply = response.content().unwrap_or("").to_string();
    println!("Assistant: {}\n", assistant_reply);
    messages.push(Message::assistant(assistant_reply));

    // Second turn
    println!("User: Good! Now what is 5 * 3?");
    messages.push(Message::user("Good! Now what is 5 * 3?"));

    let request = CompletionRequest::builder()
        .model(model)
        .messages(messages.clone())
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    let assistant_reply = response.content().unwrap_or("").to_string();
    println!("Assistant: {}", assistant_reply);

    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("\n📊 Conversation Stats:");
    println!("  Total turns: 2");
    println!("  Total tokens: {}", response.usage.total_tokens);

    Ok(())
}

async fn example_response_healing(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Requesting JSON response (with potential formatting issues)...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with valid JSON.",
        ))
        .message(Message::user(
            "Create a simple JSON object with name, age, and city for a person named Bob.",
        ))
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Raw response from LLM:");
    println!("{}", "━".repeat(60));
    let content = response.content().unwrap_or("No content");
    println!("{}\n", content);
    println!("{}", "━".repeat(60));

    // Use healing parser to handle malformed JSON
    let parser = JsonishParser::new();
    let result = parser.parse(content)?;

    println!("✅ Parse Result:");
    println!("  Confidence: {:.2}", result.confidence);
    println!("  Parsed value:");
    println!("  {}", serde_json::to_string_pretty(&result.value)?);

    if !result.flags.is_empty() {
        println!("\n  🔧 Healing applied:");
        for flag in &result.flags {
            println!("    - {}", flag.description());
        }
    } else {
        println!("\n  ✨ No healing needed (perfect JSON)");
    }

    println!("\n📊 Tokens used:");
    println!("  Prompt: {}", response.usage.prompt_tokens);
    println!("  Completion: {}", response.usage.completion_tokens);
    println!("  Total: {}", response.usage.total_tokens);

    Ok(())
}

async fn example_type_coercion(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Requesting data with mixed value types...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON object for a product with: id (as string), \
             price (as string with decimal), in_stock (as string 'true' or 'false'), and name.",
        ))
        .temperature(0.5)
        .max_tokens(150)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Raw response:");
    println!("{}", "━".repeat(60));
    let content = response.content().unwrap_or("No content");
    println!("{}\n", content);
    println!("{}", "━".repeat(60));

    // Parse JSON
    let parser = JsonishParser::new();
    let parse_result = parser.parse(content)?;
    println!(
        "✅ Parsed JSON (confidence: {:.2})",
        parse_result.confidence
    );

    // Coerce to proper types
    let engine = CoercionEngine::new();
    let schema = Schema::object(vec![
        ("id".into(), Schema::String, true),
        ("price".into(), Schema::Float, true),
        ("in_stock".into(), Schema::Bool, true),
        ("name".into(), Schema::String, true),
    ]);

    let coerce_result = engine.coerce(&parse_result.value, &schema)?;

    println!("\n🔧 Coercion Result:");
    println!("  Confidence: {:.2}", coerce_result.confidence);
    println!("  Coerced value:");
    println!("  {}", serde_json::to_string_pretty(&coerce_result.value)?);

    if !coerce_result.flags.is_empty() {
        println!("\n  Coercions applied:");
        for flag in &coerce_result.flags {
            println!("    - {}", flag.description());
        }
    }

    // Verify types
    println!("\n  Type verification:");
    if let Some(id) = coerce_result.value.get("id") {
        println!("    id: {} ({:?})", id, id);
    }
    if let Some(price) = coerce_result.value.get("price") {
        println!("    price: {} ({:?})", price, price);
    }
    if let Some(in_stock) = coerce_result.value.get("in_stock") {
        println!("    in_stock: {} ({:?})", in_stock, in_stock);
    }

    Ok(())
}

async fn example_fuzzy_matching(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Requesting data with inconsistent field naming...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON object with fields in mixed case: \
             Firstname (uppercase), last_name (snake_case), EmailAddress (camelCase), and AGE (uppercase).",
        ))
        .temperature(0.5)
        .max_tokens(150)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Raw response:");
    println!("{}", "━".repeat(60));
    let content = response.content().unwrap_or("No content");
    println!("{}\n", content);
    println!("{}", "━".repeat(60));

    // Parse JSON
    let parser = JsonishParser::new();
    let parse_result = parser.parse(content)?;

    // Define schema with standard field names
    let engine = CoercionEngine::new();
    let schema = Schema::object(vec![
        ("firstName".into(), Schema::String, true),
        ("lastName".into(), Schema::String, true),
        ("emailAddress".into(), Schema::String, true),
        ("age".into(), Schema::Int, true),
    ]);

    let coerce_result = engine.coerce(&parse_result.value, &schema)?;

    println!("✅ Fuzzy Matching Result:");
    println!("  Confidence: {:.2}", coerce_result.confidence);
    println!("  Normalized value:");
    println!("  {}", serde_json::to_string_pretty(&coerce_result.value)?);

    // Show fuzzy matches
    let fuzzy_matches: Vec<_> = coerce_result
        .flags
        .iter()
        .filter_map(|f| {
            if let CoercionFlag::FuzzyFieldMatch { expected, found } = f {
                Some((expected, found))
            } else {
                None
            }
        })
        .collect();

    if !fuzzy_matches.is_empty() {
        println!("\n  🔍 Fuzzy field matches:");
        for (expected, found) in fuzzy_matches {
            println!("    - '{}' matched to '{}'", found, expected);
        }
    }

    if !coerce_result.flags.is_empty() {
        println!("\n  🔧 Other transformations:");
        for flag in &coerce_result.flags {
            if !matches!(flag, CoercionFlag::FuzzyFieldMatch { .. }) {
                println!("    - {}", flag.description());
            }
        }
    }

    Ok(())
}

async fn example_streaming_healing(provider: &OpenAIProvider, model: &str) -> Result<()> {
    healing_showcase::example_streaming_healing(provider, model, "custom-api", "📊 Metrics").await
}

async fn example_streaming_structured(provider: &OpenAIProvider, model: &str) -> Result<()> {
    healing_showcase::example_streaming_structured(provider, model, "custom-api", "📊 Metrics")
        .await
}

async fn example_streaming_graph(provider: &OpenAIProvider, model: &str) -> Result<()> {
    healing_showcase::example_streaming_graph(provider, model, "custom-api", "📊 Metrics").await
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &key[..4], &key[key.len() - 4..])
    }
}
