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

use simple_agents_healing::prelude::*;
use simple_agents_providers::metrics::RequestTimer;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agent_type::prelude::*;
use std::io::Write;
use std::time::Duration;

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
    use futures_util::StreamExt;

    println!("\n📤 Streaming JSON response with healing (Large JSON)...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON. Wrap JSON in markdown.",
        ))
        .message(Message::user(
            "Create a comprehensive JSON object for a senior software engineer named Charlie \
             with these fields: name, age, email, phone, address (street, city, state, zip, country), \
             skills (array of at least 5 skills), experience (array of job objects with title, company, years), \
             education (array with degree, school, year), projects (array of 3 project objects with name, description, tech_stack, url), \
             languages (array with language, proficiency), certifications (array), available (boolean), hourly_rate, github, linkedin.",
        ))
        .temperature(0.7)
        .max_tokens(500)
        .stream(true)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);
    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    println!("📝 Streaming with progressive healing:");
    println!("{}", "━".repeat(60));

    let mut full_content = String::new();
    let mut streaming_parser = StreamingParser::new();
    let mut chunk_count = 0;
    let mut heal_count = 0;
    let mut last_parse_size = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                        full_content.push_str(content);

                        streaming_parser.feed(content);

                        if let Some(parse_result) = streaming_parser.try_parse() {
                            if !parse_result.flags.is_empty() {
                                heal_count = parse_result.flags.len();
                            }

                            let current_size = serde_json::to_string(&parse_result.value)
                                .unwrap_or_default()
                                .len();
                            if current_size > last_parse_size + 500
                                || parse_result.value.get("projects").is_some()
                                    && last_parse_size == 0
                            {
                                last_parse_size = current_size;
                                println!(
                                    "\n\n🔍 Progressive parse ({} bytes, {:.2}% complete):",
                                    current_size,
                                    (full_content.len() as f32 / 500.0 * 100.0).min(100.0)
                                );
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

    println!("\n{}", "━".repeat(60));

    let final_result = streaming_parser.finalize()?;

    println!("\n✅ Final Healed Result:");
    println!("  Confidence: {:.2}", final_result.confidence);
    println!(
        "  Total fields: {}",
        final_result.value.as_object().map(|o| o.len()).unwrap_or(0)
    );
    println!(
        "  JSON size: {} bytes",
        serde_json::to_string(&final_result.value)
            .unwrap_or_default()
            .len()
    );

    // Show summary of nested structures
    if let Some(obj) = final_result.value.as_object() {
        if let Some(skills) = obj.get("skills").and_then(|v| v.as_array()) {
            println!("  Skills: {} items", skills.len());
        }
        if let Some(experience) = obj.get("experience").and_then(|v| v.as_array()) {
            println!("  Experience: {} items", experience.len());
        }
        if let Some(projects) = obj.get("projects").and_then(|v| v.as_array()) {
            println!("  Projects: {} items", projects.len());
        }
        if let Some(education) = obj.get("education").and_then(|v| v.as_array()) {
            println!("  Education: {} items", education.len());
        }
    }

    if !final_result.flags.is_empty() {
        println!("\n  🔧 Healing transformations:");
        for flag in &final_result.flags {
            println!("    - {}", flag.description());
        }
    }

    println!("\n📊 Metrics:");
    println!("  Chunks: {}", chunk_count);
    println!("  Healing operations: {}", heal_count);
    println!("  Total length: {} characters", full_content.len());

    let estimated_tokens = (full_content.len() as f32 / 4.0) as u32;
    timer.complete_success(150, estimated_tokens);

    Ok(())
}

async fn example_streaming_structured(provider: &OpenAIProvider, model: &str) -> Result<()> {
    use futures_util::StreamExt;

    println!("\n📤 Streaming structured JSON with progressive parsing (Large Array)...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON array of 8 products with id, name, price, in_stock, category, tags (array), rating, and description fields.",
        ))
        .temperature(0.5)
        .max_tokens(500)
        .stream(true)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);
    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    println!("📝 Progressive structured output:");
    println!("{}", "━".repeat(60));

    let mut full_content = String::new();
    let mut streaming_parser = StreamingParser::new();
    let mut chunk_count = 0;
    let mut partial_count = 0;
    let mut last_item_count = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                        full_content.push_str(content);

                        streaming_parser.feed(content);

                        if let Some(parse_result) = streaming_parser.try_parse() {
                            partial_count += 1;

                            let current_items =
                                parse_result.value.as_array().map(|a| a.len()).unwrap_or(0);

                            if partial_count == 1 || current_items > last_item_count {
                                last_item_count = current_items;
                                println!(
                                    "\n\n🔍 Progressive parse #{} ({} items, {:.2}% complete):",
                                    partial_count,
                                    current_items,
                                    (full_content.len() as f32 / 500.0 * 100.0).min(100.0)
                                );

                                // Show last few items if available
                                if let Some(arr) = parse_result.value.as_array() {
                                    let start = if arr.len() > 3 { arr.len() - 3 } else { 0 };
                                    for (i, item) in arr.iter().enumerate().skip(start) {
                                        println!(
                                            "  [{}] {}",
                                            i,
                                            serde_json::to_string(item).unwrap_or_default()
                                        );
                                    }
                                }
                                println!("{}", "━".repeat(60));
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

    println!("\n{}", "━".repeat(60));

    let final_result = streaming_parser.finalize()?;

    println!("\n✅ Final Structured Output:");
    println!("  Confidence: {:.2}", final_result.confidence);
    println!(
        "  Total items in array: {}",
        final_result.value.as_array().map(|a| a.len()).unwrap_or(0)
    );
    println!(
        "  JSON size: {} bytes",
        serde_json::to_string(&final_result.value)
            .unwrap_or_default()
            .len()
    );

    if !final_result.flags.is_empty() {
        println!("\n  🔧 Healing applied:");
        for flag in &final_result.flags {
            println!("    - {}", flag.description());
        }
    }

    // Show first 2 and last 2 items
    if let Some(arr) = final_result.value.as_array() {
        if arr.len() > 4 {
            println!("\n  Sample items:");
            for i in [0, 1, arr.len() - 2, arr.len() - 1] {
                println!("    [{}]", i);
                for (k, v) in arr[i].as_object().unwrap_or(&serde_json::Map::new()) {
                    println!("      {}: {}", k, v);
                }
            }
        }
    }

    println!("\n📊 Metrics:");
    println!("  Chunks: {}", chunk_count);
    println!("  Partial parses: {}", partial_count);
    println!("  Total length: {} characters", full_content.len());

    let estimated_tokens = (full_content.len() as f32 / 4.0) as u32;
    timer.complete_success(100, estimated_tokens);

    Ok(())
}

async fn example_streaming_graph(provider: &OpenAIProvider, model: &str) -> Result<()> {
    use futures_util::StreamExt;

    println!("\n📤 Streaming graph data with progressive visualization...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON graph representing a software architecture with these fields: \
             nodes (array of objects with id, name, type, group) - include at least 10 nodes \
             representing services, databases, queues, and frontend; \
             edges (array of objects with source, target, type, label) - create connections between nodes; \
             layout (object with type: 'hierarchical', direction: 'top-down').",
        ))
        .temperature(0.6)
        .max_tokens(600)
        .stream(true)
        .build()?;

    let timer = RequestTimer::start("custom-api", model);
    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    println!("📝 Progressive graph visualization:");
    println!("{}", "━".repeat(60));

    let mut full_content = String::new();
    let mut streaming_parser = StreamingParser::new();
    let mut chunk_count = 0;
    let mut partial_count = 0;
    let mut last_node_count = 0;
    let mut last_edge_count = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                        full_content.push_str(content);

                        streaming_parser.feed(content);

                        if let Some(parse_result) = streaming_parser.try_parse() {
                            partial_count += 1;

                            let nodes = parse_result
                                .value
                                .get("nodes")
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);

                            let edges = parse_result
                                .value
                                .get("edges")
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);

                            // Update when new nodes or edges are added
                            if nodes > last_node_count || edges > last_edge_count {
                                last_node_count = nodes;
                                last_edge_count = edges;

                                println!("\n\n🔍 Progressive graph update #{}", partial_count);
                                println!("  📊 Nodes: {} | Edges: {}", nodes, edges);
                                println!(
                                    "  📈 Progress: {:.1}%",
                                    (full_content.len() as f32 / 600.0 * 100.0).min(100.0)
                                );
                                println!("  🔧 Confidence: {:.2}", parse_result.confidence);

                                // Draw ASCII graph representation
                                if let Some(node_arr) =
                                    parse_result.value.get("nodes").and_then(|v| v.as_array())
                                {
                                    println!("\n  🎨 Live Graph Preview:");
                                    println!("  {}", "─".repeat(50));

                                    // Group nodes by type
                                    let mut groups: std::collections::HashMap<&str, Vec<&str>> =
                                        std::collections::HashMap::new();
                                    for node in node_arr.iter() {
                                        if let Some(name) =
                                            node.get("name").and_then(|v| v.as_str())
                                        {
                                            if let Some(typ) =
                                                node.get("type").and_then(|v| v.as_str())
                                            {
                                                groups.entry(typ).or_default().push(name);
                                            }
                                        }
                                    }

                                    // Display groups
                                    for (typ, names) in groups.iter() {
                                        let icon = match *typ {
                                            "service" => "⚙️",
                                            "database" => "🗄️",
                                            "queue" => "📨",
                                            "frontend" => "🖥️",
                                            _ => "📦",
                                        };
                                        println!("  {} [{}] {}:", icon, typ, names.join(", "));
                                    }

                                    println!("  {}", "─".repeat(50));
                                }
                                println!("{}", "━".repeat(60));
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

    println!("\n{}", "━".repeat(60));

    let final_result = streaming_parser.finalize()?;

    println!("\n✅ Final Graph Structure:");
    println!("  Confidence: {:.2}", final_result.confidence);
    println!(
        "  JSON size: {} bytes",
        serde_json::to_string(&final_result.value)
            .unwrap_or_default()
            .len()
    );

    if let Some(nodes) = final_result.value.get("nodes").and_then(|v| v.as_array()) {
        println!("  📊 Total nodes: {}", nodes.len());

        // Node type breakdown
        let mut type_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for node in nodes.iter() {
            if let Some(typ) = node.get("type").and_then(|v| v.as_str()) {
                *type_counts.entry(typ).or_insert(0) += 1;
            }
        }

        println!("\n  🎯 Node Type Distribution:");
        for (typ, count) in type_counts {
            let bar = "█".repeat(count * 2);
            println!("    {} [{}]: {} {}", typ, count, bar, bar.len());
        }

        // Show node list
        println!("\n  📝 Node Details:");
        for (i, node) in nodes.iter().enumerate().take(5) {
            if let Some(obj) = node.as_object() {
                let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                let group = obj.get("group").and_then(|v| v.as_str()).unwrap_or("-");
                println!("    [{}] {} | {} | {} | group: {}", i, id, name, typ, group);
            }
        }
        if nodes.len() > 5 {
            println!("    ... and {} more nodes", nodes.len() - 5);
        }
    }

    if let Some(edges) = final_result.value.get("edges").and_then(|v| v.as_array()) {
        println!("\n  🔗 Total edges: {}", edges.len());

        // Show edge sample
        println!("\n  🔗 Edge Sample:");
        for (i, edge) in edges.iter().enumerate().take(3) {
            if let Some(obj) = edge.as_object() {
                let source = obj.get("source").and_then(|v| v.as_str()).unwrap_or("?");
                let target = obj.get("target").and_then(|v| v.as_str()).unwrap_or("?");
                let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("-");
                let label = obj.get("label").and_then(|v| v.as_str()).unwrap_or("");
                println!(
                    "    [{}] {} --> {} | type: {} | label: '{}'",
                    i, source, target, typ, label
                );
            }
        }
        if edges.len() > 3 {
            println!("    ... and {} more edges", edges.len() - 3);
        }
    }

    if let Some(layout) = final_result.value.get("layout").and_then(|v| v.as_object()) {
        println!("\n  📐 Layout Config:");
        for (k, v) in layout {
            println!("    {}: {}", k, v);
        }
    }

    if !final_result.flags.is_empty() {
        println!("\n  🔧 Healing applied:");
        for flag in &final_result.flags {
            println!("    - {}", flag.description());
        }
    }

    println!("\n📊 Metrics:");
    println!("  Chunks: {}", chunk_count);
    println!("  Progressive updates: {}", partial_count);
    println!("  Total length: {} characters", full_content.len());

    let estimated_tokens = (full_content.len() as f32 / 4.0) as u32;
    timer.complete_success(200, estimated_tokens);

    Ok(())
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &key[..4], &key[key.len() - 4..])
    }
}
