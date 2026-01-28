//! SimpleAgents - Full Example with Coercion Healing
//!
//! This example demonstrates:
//! 1. Making real API calls to OpenAI
//! 2. Requesting structured JSON responses from the LLM
//! 3. Using the healing system to parse malformed JSON
//! 4. Type coercion to ensure correct data types
//! 5. Handling errors and confidence scoring
//! 6. Streaming with progressive JSON healing
//! 7. Streaming structured output with partial parsing
//!
//! # Prerequisites
//!
//! 1. Copy `.env.example` to `.env`
//! 2. Add your OpenAI API key to `.env`
//! 3. Optionally set a base URL or model override
//!
//! ```bash
//! cp .env.example .env
//! # Edit .env and add your API key
//! # Optional:
//! # OPENAI_API_BASE=http://localhost:4000/v1
//! # OPENAI_API_MODEL=gpt-4.1
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example full_api_example
//! ```

use futures_util::StreamExt;
use serde_json::json;
use simple_agents_healing::prelude::*;
use simple_agents_healing::string_utils::jaro_winkler;
use simple_agents_providers::metrics::RequestTimer;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;
use simple_agents_types::prelude::*;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   SimpleAgents - Full API + Coercion Healing Demo        ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Setup provider from environment (optional base URL override)
    let provider = OpenAIProvider::from_env()?;
    let model = std::env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

    println!("✅ API key loaded successfully\n");

    // Example 1: Basic JSON parsing
    println!("{}", "━".repeat(60));
    println!("Example 1: Basic JSON Healing");
    println!("{}", "━".repeat(60));
    example_basic_json(&provider, &model).await?;

    // Example 2: Type coercion
    println!("\n{}", "━".repeat(60));
    println!("Example 2: Type Coercion");
    println!("{}", "━".repeat(60));
    example_type_coercion(&provider, &model).await?;

    // Example 3: Complex structured data with schema
    println!("\n{}", "━".repeat(60));
    println!("Example 3: Complex Schema Validation");
    println!("{}", "━".repeat(60));
    example_schema_validation(&provider, &model).await?;

    // Example 4: Fuzzy field matching
    println!("\n{}", "━".repeat(60));
    println!("Example 4: Fuzzy Field Matching");
    println!("{}", "━".repeat(60));
    example_fuzzy_matching(&provider, &model).await?;

    // Example 5: Streaming with healing
    println!("\n{}", "━".repeat(60));
    println!("Example 5: Streaming + Response Healing");
    println!("{}", "━".repeat(60));
    example_streaming_healing(&provider, &model).await?;

    // Example 6: Streaming structured output
    println!("\n{}", "━".repeat(60));
    println!("Example 6: Streaming Structured Output (Progressive JSON)");
    println!("{}", "━".repeat(60));
    example_streaming_structured(&provider, &model).await?;

    // Example 7: Streaming graph visualization
    println!("\n{}", "━".repeat(60));
    println!("Example 7: Streaming Graph Visualization (Progressive)");
    println!("{}", "━".repeat(60));
    example_streaming_graph(&provider, &model).await?;

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║                    Demo Complete!                         ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}

async fn example_basic_json(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Requesting simple JSON response...\n");

    // Create a request that asks for JSON (LLMs often wrap in markdown)
    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a simple JSON object with name, age, and city for a person named Alice.",
        ))
        .temperature(0.7)
        .max_tokens(150)
        .build()?;

    // Execute with metrics
    let timer = RequestTimer::start("openai", model);
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;
    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Raw response from LLM:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let content = response.content().unwrap_or("No content");
    println!("{}\n", content);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Use healing parser
    let parser = JsonishParser::new();
    let result = parser.parse(content)?;

    println!("✅ Parse Result:");
    println!("  Confidence: {:.2}", result.confidence);
    println!("  Parsed value:");
    println!("  {}", serde_json::to_string_pretty(&result.value)?);

    // Show what transformations were applied
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
    println!("\n📤 Requesting data with numeric values as strings...\n");

    // LLMs often return numbers as strings in JSON
    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON object for a product with fields: id (as string), \
             price (as string with decimal), in_stock (as string), and name.",
        ))
        .temperature(0.5)
        .max_tokens(150)
        .build()?;

    let timer = RequestTimer::start("openai", model);
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;
    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Raw response:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let content = response.content().unwrap_or("No content");
    println!("{}\n", content);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Parse the JSON
    let parser = JsonishParser::new();
    let parse_result = parser.parse(content)?;
    println!(
        "✅ Parsed JSON (confidence: {:.2})",
        parse_result.confidence
    );

    // Now coerce to proper types
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
    if let Some(id) = coerce_result.value.get("id") {
        println!("\n  Type verification:");
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

async fn example_schema_validation(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Requesting complex structured data...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON object representing a person with: \
             first_name, last_name (camelCase), AGE (uppercase, as number string), \
             email_address (snake_case), is_active (as string), and skills (array).",
        ))
        .temperature(0.5)
        .max_tokens(200)
        .build()?;

    let timer = RequestTimer::start("openai", model);
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;
    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Raw response:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let content = response.content().unwrap_or("No content");
    println!("{}\n", content);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Define strict schema with exact field names
    let engine = CoercionEngine::new();
    let schema = Schema::Object(ObjectSchema {
        fields: vec![
            Field::required("firstName", Schema::String),
            Field::required("lastName", Schema::String),
            Field::required("age", Schema::Int),
            Field::required("emailAddress", Schema::String),
            Field::required("isActive", Schema::Bool),
            Field::optional("skills", Schema::Array(Box::new(Schema::String)))
                .with_default(json!([])),
        ],
        allow_additional_fields: false,
    });

    // Parse and coerce
    let parser = JsonishParser::new();
    let parse_result = parser.parse(content)?;
    let coerce_result = engine.coerce(&parse_result.value, &schema)?;

    println!("✅ Coercion Result:");
    println!("  Combined confidence: {:.2}", coerce_result.confidence);
    println!("  Validated value:");
    println!("  {}", serde_json::to_string_pretty(&coerce_result.value)?);

    if !coerce_result.flags.is_empty() {
        println!("\n  🔧 Transformations:");
        for flag in &coerce_result.flags {
            match flag {
                CoercionFlag::FuzzyFieldMatch { expected, found } => {
                    println!("    - Fuzzy match: '{}' → '{}'", found, expected);
                }
                _ => {
                    println!("    - {}", flag.description());
                }
            }
        }
    }

    Ok(())
}

async fn example_fuzzy_matching(provider: &OpenAIProvider, model: &str) -> Result<()> {
    println!("\n📤 Requesting data with case variations...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON object with fields in ALL CAPS: USERNAME, EMAIL, AGE (as string), and IS_VERIFIED.",
        ))
        .temperature(0.5)
        .max_tokens(150)
        .build()?;

    let timer = RequestTimer::start("openai", model);
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;
    timer.complete_success(
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
    );

    println!("📨 Raw response:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let content = response.content().unwrap_or("No content");
    println!("{}\n", content);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Parse
    let parser = JsonishParser::new();
    let parse_result = parser.parse(content)?;

    // Show similarity scores against the expected camelCase field
    let expected_field = "isVerified";
    if let Some(map) = parse_result.value.as_object() {
        println!("🔎 Similarity scores vs '{}':", expected_field);
        for key in map.keys() {
            let score = jaro_winkler(expected_field, key);
            println!("  - {}: {:.4}", key, score);
        }
    }

    // Coerce to camelCase while relying on fuzzy matching for ALL CAPS keys
    let schema = Schema::Object(ObjectSchema::new(vec![
        Field::required("username", Schema::String),
        Field::required("email", Schema::String),
        Field::required("age", Schema::Int),
        Field::required("isVerified", Schema::Bool),
    ]));

    let fuzzy_value = make_fuzzy_variant(&parse_result.value);

    let run_coercion = |label: &str, threshold: f64| -> Result<()> {
        let config = CoercionConfig {
            fuzzy_match_threshold: threshold,
            ..CoercionConfig::default()
        };
        let engine = CoercionEngine::with_config(config);

        println!("\n{} (fuzzy threshold = {:.2})", label, threshold);
        match engine.coerce(&fuzzy_value, &schema) {
            Ok(coerce_result) => {
                println!("✅ Coercion succeeded");
                println!("  Confidence: {:.2}", coerce_result.confidence);
                println!("  Normalized value:");
                println!("  {}", serde_json::to_string_pretty(&coerce_result.value)?);

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
            }
            Err(e) => {
                println!("❌ Coercion failed: {}", e);
            }
        }

        Ok(())
    };

    run_coercion("Example 4A: Default threshold", 0.8)?;
    run_coercion("Example 4B: Lower threshold", 0.6)?;

    Ok(())
}

fn make_fuzzy_variant(value: &serde_json::Value) -> serde_json::Value {
    let mut obj = match value.as_object() {
        Some(map) => map.clone(),
        None => return value.clone(),
    };

    let key_to_replace = obj
        .keys()
        .find(|k| k.eq_ignore_ascii_case("is_verified") || k.eq_ignore_ascii_case("isVerified"))
        .cloned();

    let Some(original_key) = key_to_replace else {
        return serde_json::Value::Object(obj);
    };

    let expected = "isVerified";
    let (fuzzy_key, score) = pick_fuzzy_key(expected);

    if let Some(value) = obj.remove(&original_key) {
        obj.insert(fuzzy_key.clone(), value);
    }

    println!(
        "🔧 Fuzzy demo key: '{}' → '{}' (similarity {:.2})",
        original_key, fuzzy_key, score
    );

    serde_json::Value::Object(obj)
}

fn pick_fuzzy_key(expected: &str) -> (String, f64) {
    let chars: Vec<char> = expected.chars().collect();
    let len = chars.len();
    let mut candidates: Vec<String> = Vec::new();

    // Replace the last N characters with 'x' to tune similarity.
    for n in 1..=len {
        let mut v = chars.clone();
        for item in v.iter_mut().take(len).skip(len - n) {
            *item = 'x';
        }
        candidates.push(v.iter().collect());
    }

    // Drop every third character to reduce similarity further if needed.
    let mut dropped = String::with_capacity(len);
    for (i, ch) in chars.iter().enumerate() {
        if i % 3 != 0 {
            dropped.push(*ch);
        }
    }
    candidates.push(dropped);

    let mut best: Option<(String, f64)> = None;

    for candidate in candidates {
        let score = jaro_winkler(expected, &candidate);
        if (0.6..0.8).contains(&score) {
            return (candidate, score);
        }
        if score < 0.8
            && best
                .as_ref()
                .is_none_or(|(_, best_score)| score > *best_score)
        {
            best = Some((candidate, score));
        }
    }

    if let Some((candidate, score)) = best {
        return (candidate, score);
    }

    let fallback = expected.to_string();
    let score = jaro_winkler(expected, &fallback);
    (fallback, score)
}

async fn example_streaming_healing(provider: &OpenAIProvider, model: &str) -> Result<()> {
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

    let timer = RequestTimer::start("openai", model);
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

    println!("\n📊 Tokens used:");
    println!("  Chunks: {}", chunk_count);
    println!("  Healing operations: {}", heal_count);
    println!("  Total length: {} characters", full_content.len());

    let estimated_tokens = (full_content.len() as f32 / 4.0) as u32;
    timer.complete_success(150, estimated_tokens);

    Ok(())
}

async fn example_streaming_structured(provider: &OpenAIProvider, model: &str) -> Result<()> {
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

    let timer = RequestTimer::start("openai", model);
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

    println!("\n📊 Tokens used:");
    println!("  Chunks: {}", chunk_count);
    println!("  Partial parses: {}", partial_count);
    println!("  Total length: {} characters", full_content.len());

    let estimated_tokens = (full_content.len() as f32 / 4.0) as u32;
    timer.complete_success(100, estimated_tokens);

    Ok(())
}

async fn example_streaming_graph(provider: &OpenAIProvider, model: &str) -> Result<()> {
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

    let timer = RequestTimer::start("openai", model);
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
            println!("    {} [{}]: {}", typ, count, bar);
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

    println!("\n📊 Tokens used:");
    println!("  Chunks: {}", chunk_count);
    println!("  Progressive updates: {}", partial_count);
    println!("  Total length: {} characters", full_content.len());

    let estimated_tokens = (full_content.len() as f32 / 4.0) as u32;
    timer.complete_success(200, estimated_tokens);

    Ok(())
}
