//! Basic usage example for simple-agents-types.
//!
//! This example demonstrates how to use all the core types.
//!
//! Run with: cargo run --example basic_usage

use simple_agents_types::prelude::*;

fn main() -> Result<()> {
    println!("=== SimpleAgents Types Demo ===\n");

    // 1. Creating messages
    demo_messages();

    // 2. Building requests
    demo_requests()?;

    // 3. Working with responses
    demo_responses();

    // 4. Error handling
    demo_errors();

    // 5. API key security
    demo_api_keys()?;

    // 6. Coercion tracking
    demo_coercion();

    // 7. Configuration
    demo_configuration();

    println!("\n=== All demos completed successfully! ===");
    Ok(())
}

fn demo_messages() {
    println!("📝 Creating Messages:");

    let user_msg = Message::user("What is the capital of France?");
    println!("  User: {:?}", user_msg);

    let assistant_msg = Message::assistant("The capital of France is Paris.");
    println!("  Assistant: {:?}", assistant_msg);

    let system_msg = Message::system("You are a helpful geography assistant.");
    println!("  System: {:?}", system_msg);

    let named_msg = Message::user("Hello!").with_name("Alice");
    println!("  Named: {:?}", named_msg);

    println!();
}

fn demo_requests() -> Result<()> {
    println!("🔨 Building Requests:");

    // Basic request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello!"))
        .build()?;

    println!("  Basic request: model={}, messages={}",
             request.model, request.messages.len());

    // Request with all options
    let full_request = CompletionRequest::builder()
        .model("gpt-4-turbo")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("What is 2+2?"))
        .temperature(0.7)
        .max_tokens(100)
        .top_p(0.9)
        .build()?;

    println!("  Full request: temp={:?}, max_tokens={:?}",
             full_request.temperature, full_request.max_tokens);

    // Serialization
    let json = serde_json::to_string_pretty(&request)?;
    println!("  Serialized:\n{}", json);

    println!();
    Ok(())
}

fn demo_responses() {
    println!("📨 Working with Responses:");

    let response = CompletionResponse {
        id: "resp_abc123".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![
            CompletionChoice {
                index: 0,
                message: Message::assistant("Hello! How can I help you today?"),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }
        ],
        usage: Usage::new(10, 15),
        created: Some(1234567890),
        provider: Some("openai".to_string()),
    };

    println!("  Response ID: {}", response.id);
    println!("  Content: {:?}", response.content());
    println!("  Usage: {} prompt + {} completion = {} total tokens",
             response.usage.prompt_tokens,
             response.usage.completion_tokens,
             response.usage.total_tokens);
    println!("  Finish reason: {:?}", response.choices[0].finish_reason);

    println!();
}

fn demo_errors() {
    println!("⚠️  Error Handling:");

    // Provider errors
    let rate_limit = ProviderError::RateLimit {
        retry_after: Some(std::time::Duration::from_secs(60)),
    };
    println!("  Rate limit error: {}", rate_limit);
    println!("  Is retryable? {}", rate_limit.is_retryable());

    let invalid_key = ProviderError::InvalidApiKey;
    println!("  Invalid key error: {}", invalid_key);
    println!("  Is retryable? {}", invalid_key.is_retryable());

    // Validation errors
    let validation_err = ValidationError::OutOfRange {
        field: "temperature".to_string(),
        min: 0.0,
        max: 2.0,
    };
    println!("  Validation error: {}", validation_err);

    // Healing errors
    let healing_err = HealingError::CoercionFailed {
        from: "string".to_string(),
        to: "number".to_string(),
    };
    println!("  Healing error: {}", healing_err);

    println!();
}

fn demo_api_keys() -> Result<()> {
    println!("🔐 API Key Security:");

    let key = ApiKey::new("sk-1234567890abcdefghijklmnopqrstuvwxyz")?;

    // Debug output is always redacted
    println!("  Debug: {:?}", key);

    // Preview shows partial info
    println!("  Preview: {}", key.preview());

    // Serialization is redacted
    let json = serde_json::to_string(&key)?;
    println!("  Serialized: {}", json);

    // Only expose() gives raw key
    println!("  Raw key length: {} chars", key.expose().len());
    println!("  (Raw key only accessible via expose() method)");

    println!();
    Ok(())
}

fn demo_coercion() {
    println!("🔄 Coercion Tracking:");

    // No coercion
    let perfect = CoercionResult::new(42);
    println!("  Perfect result: value={}, coerced={}",
             perfect.value, perfect.was_coerced());

    // With minor coercion
    let minor = CoercionResult::new("Hello")
        .with_flag(CoercionFlag::StrippedMarkdown)
        .with_flag(CoercionFlag::FixedTrailingComma)
        .set_confidence(0.95);

    println!("  Minor coercion: flags={}, confidence={}, major={}",
             minor.flags.len(), minor.confidence, minor.has_major_coercions());

    for flag in &minor.flags {
        println!("    - {}", flag.description());
    }

    // With major coercion
    let major = CoercionResult::new(100)
        .with_flag(CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "number".to_string(),
        })
        .set_confidence(0.7);

    println!("  Major coercion: major={}, confident={}",
             major.has_major_coercions(),
             major.is_confident(0.8));

    println!();
}

fn demo_configuration() {
    println!("⚙️  Configuration:");

    // Retry config
    let retry = RetryConfig::default();
    println!("  Retry config:");
    println!("    Max attempts: {}", retry.max_attempts);
    println!("    Initial backoff: {:?}", retry.initial_backoff);
    println!("    Backoff multiplier: {}", retry.backoff_multiplier);

    let backoff1 = retry.calculate_backoff(0);
    let backoff2 = retry.calculate_backoff(1);
    println!("    Attempt 0 backoff: {:?}", backoff1);
    println!("    Attempt 1 backoff: {:?}", backoff2);

    // Healing config
    let healing = HealingConfig::default();
    println!("  Healing config:");
    println!("    Enabled: {}", healing.enabled);
    println!("    Strict mode: {}", healing.strict_mode);
    println!("    Min confidence: {}", healing.min_confidence);

    let strict = HealingConfig::strict();
    println!("  Strict healing: min_confidence={}", strict.min_confidence);

    let lenient = HealingConfig::lenient();
    println!("  Lenient healing: min_confidence={}", lenient.min_confidence);

    // Provider config
    let provider = ProviderConfig::new("openai", "https://api.openai.com/v1")
        .with_api_key("sk-test")
        .with_default_model("gpt-4")
        .with_timeout(std::time::Duration::from_secs(30));

    println!("  Provider config:");
    println!("    Name: {}", provider.name);
    println!("    Timeout: {:?}", provider.timeout);

    println!();
}
