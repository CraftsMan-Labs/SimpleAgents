# Usage Guide

This guide covers everything you need to know to use SimpleAgents in your applications.

 ## Table of Contents

 - [Installation](#installation)
 - [Basic Usage](#basic-usage)
 - [Providers](#providers)
 - [Messages](#messages)
 - [Request Configuration](#request-configuration)
 - [Caching](#caching)
 - [Retry Logic](#retry-logic)
 - [Error Handling](#error-handling)
 - [Streaming](#streaming)
 - [Response Healing](#response-healing)
 - [Rate Limiting](#rate-limiting)
 - [Metrics](#metrics)
 - [Advanced Features](#advanced-features)

## Installation

Add SimpleAgents to your `Cargo.toml`:

```toml
 [dependencies]
 simple-agents-types = "0.1.0"
 simple-agents-providers = "0.1.0"
 simple-agents-cache = "0.1.0"
 simple-agents-healing = "0.1.0"
 simple-agents-macros = "0.1.0"
 tokio = { version = "1.35", features = ["full"] }
 futures-util = "0.3"
```

## Basic Usage

### Creating a Provider

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

// Create an API key (validated automatically)
let api_key = ApiKey::new("sk-...")?;

// Create the provider
let provider = OpenAIProvider::new(api_key)?;
```

### Building a Request

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .messages(vec![Message::user("What is the capital of France?")])
    .temperature(0.7)
    .max_tokens(100)
    .build()?;
```

### Executing a Request

```rust
// Transform to provider-specific format
let provider_request = provider.transform_request(&request)?;

// Execute the request
let provider_response = provider.execute(provider_request).await?;

// Transform back to unified format
let response = provider.transform_response(provider_response)?;

// Get the content
println!("{}", response.content().unwrap_or("No response"));
```

## Providers

### OpenAI Provider

```rust
use simple_agents_providers::openai::OpenAIProvider;

// Default base URL (api.openai.com)
let provider = OpenAIProvider::new(api_key)?;

// Custom base URL (e.g., for Azure OpenAI)
let provider = OpenAIProvider::with_base_url(
    api_key,
    "https://your-resource.openai.azure.com/".to_string()
)?;
```

**Supported Models:**
- `gpt-4`
- `gpt-4-turbo-preview`
- `gpt-3.5-turbo`
- All OpenAI chat completion models

**Connection Pooling:**
The OpenAI provider automatically uses HTTP/2 with connection pooling:
- 10 idle connections per host
- 90-second keep-alive
- Automatic connection reuse

 ### Anthropic Provider (Stub)

 ```rust
 use simple_agents_providers::anthropic::AnthropicProvider;

 // Note: Currently a stub implementation
 let provider = AnthropicProvider::new(api_key)?;
 ```

### OpenRouter Provider

```rust
 use simple_agents_providers::openrouter::OpenRouterProvider;

 // Create provider
 let provider = OpenRouterProvider::new(api_key)?;

 // Build request with provider prefix
 let request = CompletionRequest::builder()
     .model("openai/gpt-3.5-turbo")  // Provider prefix included
     .message(Message::user("Hello"))
     .build()?;
 ```

**Supported Providers:**

- `openai/*` - All OpenAI models (gpt-4, gpt-3.5-turbo, etc.)
- `anthropic/*` - All Anthropic models (claude-3-opus, claude-3-sonnet, etc.)
- `meta-llama/*` - Meta models (llama-2-70b-chat, llama-2-13b-chat)
- `google/*` - Google models (palm-2-chat-bison, gemini-pro)
- And many more - see [openrouter.ai/models](https://openrouter.ai/models)

**Benefits:**
- Single API key for multiple providers
- Consistent request/response format
- Automatic failover to alternative models

## Messages

SimpleAgents supports different message types:

### User Messages

```rust
let msg = Message::user("Hello, AI!");
```

### Assistant Messages

```rust
let msg = Message::assistant("Hello, human!");
```

### System Messages

```rust
let msg = Message::system("You are a helpful assistant.");
```

### Tool/Function Messages

```rust
let msg = Message::tool("function_result", Some("tool_call_123"));
```

### Named Messages

```rust
let msg = Message::user("Hello!")
    .with_name("John");
```

### Building a Conversation

You can add messages one-by-one with `.message(...)` or pass an ordered list with `.messages(vec![...])`.

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .messages(vec![
        Message::system("You are a helpful assistant."),
        Message::user("What is 2+2?"),
        Message::assistant("4"),
        Message::user("What is 3+3?"),
    ])
    .build()?;
```

## Request Configuration

### Temperature

Controls randomness (0.0 = deterministic, 2.0 = very random):

```rust
.temperature(0.7)
```

### Max Tokens

Limits the response length:

```rust
.max_tokens(500)
```

### Top P (Nucleus Sampling)

Alternative to temperature (0.0-1.0):

```rust
.top_p(0.9)
```

### Stop Sequences

Stop generation at specific strings:

```rust
.stop(vec!["END".to_string(), "\n\n".to_string()])
```

### Number of Completions

Generate multiple responses:

```rust
.n(3)
```

### Frequency and Presence Penalties

Control repetition (-2.0 to 2.0):

```rust
.frequency_penalty(0.5)
.presence_penalty(0.5)
```

### Complete Example

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::system("You are a creative writer."))
    .message(Message::user("Write a haiku about coding."))
    .temperature(0.9)
    .max_tokens(100)
    .top_p(0.95)
    .frequency_penalty(0.3)
    .build()?;
```

## Caching

SimpleAgents provides built-in caching to reduce API calls and costs.

### In-Memory Cache

```rust
use simple_agents_cache::InMemoryCache;
use simple_agents_types::cache::Cache;
use std::time::Duration;

// Create cache with limits
let cache = InMemoryCache::new(
    10 * 1024 * 1024,  // 10MB max size
    1000               // 1000 max entries
);

// Store a response
let key = "request_hash";
let data = serde_json::to_vec(&response)?;
cache.set(key, data, Duration::from_secs(3600)).await?;

// Retrieve from cache
if let Some(cached) = cache.get(key).await? {
    let response: CompletionResponse = serde_json::from_slice(&cached)?;
    println!("Cache hit!");
}
```

### Cache Key Generation

```rust
use simple_agents_types::cache::CacheKey;

let key = CacheKey::from_parts(
    "openai",
    "gpt-4",
    "user: What is the capital of France?"
);
```

### No-Op Cache (for Testing)

```rust
use simple_agents_cache::NoOpCache;

let cache = NoOpCache; // Does nothing, always returns None
```

### LRU Eviction

The `InMemoryCache` automatically evicts:
- Expired entries (based on TTL)
- Least recently used entries (when limits are exceeded)

## Retry Logic

Implement retry with exponential backoff:

```rust
use simple_agents_providers::retry::execute_with_retry;
use simple_agents_types::config::RetryConfig;
use std::time::Duration;

let config = RetryConfig {
    max_attempts: 3,
    initial_backoff: Duration::from_millis(100),
    max_backoff: Duration::from_secs(10),
    backoff_multiplier: 2.0,
    jitter: true,
};

let result = execute_with_retry(
    &config,
    |e| e.is_retryable(), // Only retry retryable errors
    || async {
        // Your operation here
        provider.execute(provider_request.clone()).await
    }
).await?;
```

### Default Retry Configuration

```rust
let config = RetryConfig::default();
// max_attempts: 3
// initial_backoff: 100ms
// max_backoff: 10s
// backoff_multiplier: 2.0
// jitter: true
```

## Error Handling

SimpleAgents uses a comprehensive error type:

```rust
use simple_agents_types::error::{SimpleAgentsError, ProviderError, ValidationError};

match result {
    Ok(response) => {
        // Success
        println!("{}", response.content().unwrap_or(""));
    }
    Err(SimpleAgentsError::Validation(e)) => {
        eprintln!("Validation error: {}", e);
    }
    Err(SimpleAgentsError::Provider(e)) => {
        if e.is_retryable() {
            eprintln!("Retryable provider error: {}", e);
        } else {
            eprintln!("Non-retryable provider error: {}", e);
        }
    }
    Err(SimpleAgentsError::Network(e)) => {
        eprintln!("Network error: {}", e);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

### Retryable Errors

These errors can be safely retried:
- Rate limit errors (429)
- Server errors (5xx)
- Timeout errors
- Temporary network errors

### Non-Retryable Errors

These should not be retried:
- Invalid API key (401)
- Invalid request (400)
- Model not found (404)
- Validation errors

## Advanced Features

### Streaming (Framework Implemented)

```rust
// Note: Full SSE parsing not yet implemented
let stream = provider.execute_stream(provider_request).await?;

// Future API (when fully implemented):
// while let Some(chunk) = stream.next().await {
//     match chunk {
//         Ok(chunk) => {
//             if let Some(content) = chunk.choices.first() {
//                 print!("{}", content.delta.content.as_deref().unwrap_or(""));
//             }
//         }
//         Err(e) => eprintln!("Stream error: {}", e),
//     }
// }
```

### Custom Headers

```rust
use simple_agents_types::provider::{ProviderRequest, headers};
use std::borrow::Cow;

let request = ProviderRequest::new("https://api.example.com/v1/chat")
    .with_header("Authorization", format!("Bearer {}", api_key.expose()))
    .with_static_header(headers::CONTENT_TYPE, "application/json")
    .with_body(body);
```

 ### Request Size Validation

 Requests are automatically validated:
 - Maximum 1MB per message
 - Maximum 10MB total request size
 - Maximum 1000 messages

 ```rust
 // This will fail validation if total size > 10MB
 let large_request = CompletionRequest::builder()
     .model("gpt-4")
     .message(Message::user("x".repeat(11 * 1024 * 1024)))
     .build()?; // Error: TooLong
 ```

 ## Streaming

 SimpleAgents supports streaming responses from OpenAI and Anthropic providers.

### Basic Streaming

```rust
use futures_util::StreamExt;

let request = CompletionRequest::builder()
    .model("gpt-3.5-turbo")
    .message(Message::user("Write a story."))
    .stream(true)  // Enable streaming
    .build()?;

let provider_request = provider.transform_request(&request)?;
let mut stream = provider.execute_stream(provider_request).await?;

while let Some(chunk_result) = stream.next().await {
    match chunk_result {
        Ok(chunk) => {
            if let Some(choice) = chunk.choices.first() {
                if let Some(content) = &choice.delta.content {
                    print!("{}", content);
                    std::io::stdout().flush().unwrap();
                }
            }
        }
        Err(e) => eprintln!("Stream error: {}", e),
    }
}
```

### Stream Aggregation

```rust
let mut full_content = String::new();

while let Some(chunk_result) = stream.next().await {
    if let Ok(chunk) = chunk_result {
        if let Some(choice) = chunk.choices.first() {
            if let Some(content) = &choice.delta.content {
                full_content.push_str(content);
            }
        }
    }
}

println!("Full response: {}", full_content);
```

## Response Healing

 SimpleAgents includes a powerful response healing system that handles malformed LLM outputs.

### JSON Healing

```rust
use simple_agents_healing::prelude::*;

let parser = JsonishParser::new();

// Parse markdown-wrapped JSON with trailing commas
let malformed = r#"```json
{"name": "Alice", "age": 30, "skills": ["Rust", "Python"],}
```"#;

let result = parser.parse(malformed)?;

// Access parsed value
assert_eq!(result.value["name"], "Alice");
assert_eq!(result.value["age"], 30);

// Check confidence and transformations
assert!(result.confidence > 0.85);
assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));
assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
```

### Type Coercion

```rust
let engine = CoercionEngine::new();

// String to number coercion
let input = json!({"age": "25", "score": 98.5});
let schema = Schema::object(vec![
    ("age".into(), Schema::Int, true),
    ("score".into(), Schema::Float, true),
]);

let result = engine.coerce(&input, &schema)?;

// Coerced to proper types
assert_eq!(result.value["age"], 25);  // Was string "25"
assert_eq!(result.value["score"], 98.5);
```

### Fuzzy Field Matching

```rust
let engine = CoercionEngine::new();

let input = json!({
    "FIRSTNAME": "Alice",      // Case mismatch
    "last_name": "Smith",      // snake_case
    "emailAdress": "alice@example.com"  // Typo
});

let schema = Schema::object(vec![
    ("firstName".into(), Schema::String, true),
    ("lastName".into(), Schema::String, true),
    ("emailAddress".into(), Schema::String, true),
]);

let result = engine.coerce(&input, &schema)?;
assert_eq!(result.value["firstName"], "Alice");
```

**Matching order and aliases**

- Matching tries exact name, aliases, case-insensitive match, snake↔camel conversion, then Jaro-Winkler.
- Aliases are checked before fuzzy matching and are recorded as `FuzzyFieldMatch` when used.

```rust
let schema = Schema::Object(ObjectSchema::new(vec![
    Field::required("isVerified", Schema::Bool)
        .with_alias("IS_VERIFIED")
        .with_alias("is_verified"),
]));
```

**Tuning the fuzzy threshold**

```rust
let config = CoercionConfig {
    fuzzy_match_threshold: 0.6, // default is 0.8
    ..CoercionConfig::default()
};
let engine = CoercionEngine::with_config(config);
```

### Default Values

```rust
let schema = Schema::object(vec![
    ("name".into(), Schema::String, true),
    Field::optional("age", Schema::Int).with_default(json!(25)),
    Field::optional("active", Schema::Bool).with_default(json!(true)),
]);

let input = json!({"name": "Alice"});
let result = engine.coerce(&input, &schema)?;

assert_eq!(result.value["age"], 25);
assert_eq!(result.value["active"], true);
```

### Partial Types for Streaming

```rust
use simple_agents_macros::PartialType;

#[derive(Debug, Clone, PartialType, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

// This generates PartialUser with all fields as Option<T>
let mut partial = PartialUser::default();
partial.id = Some(1);
partial.name = Some("Alice".to_string());

// Convert to complete type when ready
let user = User::from_partial(partial)?;
```

## Rate Limiting

 SimpleAgents provides token bucket rate limiting to control request rates.

### Basic Rate Limiting

```rust
use simple_agents_providers::rate_limit::{TokenBucket, RateLimiter};

// Create token bucket (1000 tokens, refills at 100/sec)
let bucket = TokenBucket::new(1000, 100);
let limiter = RateLimiter::new(bucket);

// Check rate limit before each request
limiter.check().await?;

let provider_request = provider.transform_request(&request)?;
let response = provider.execute(provider_request).await?;
```

### Per-Provider Limits

```rust
let openai_limiter = RateLimiter::new(
    TokenBucket::new(1000, 100)  // 1000 req/min, 100/sec
);

let anthropic_limiter = RateLimiter::new(
    TokenBucket::new(500, 50)   // 500 req/min, 50/sec
);

// Use appropriate limiter based on provider
let limiter = match provider.name() {
    "openai" => &openai_limiter,
    "anthropic" => &anthropic_limiter,
    _ => return Err("Unknown provider".into()),
};

limiter.check().await?;
```

## Metrics

 SimpleAgents includes Prometheus-compatible metrics for monitoring.

### Basic Metrics

```rust
use simple_agents_providers::metrics::RequestTimer;

let timer = RequestTimer::start("openai", "gpt-4");

// Execute request
let response = provider.execute(provider_request).await?;

// Record metrics
timer.complete_success(
    response.usage.prompt_tokens,
    response.usage.completion_tokens
);
```

### Error Metrics

```rust
let timer = RequestTimer::start("openai", "gpt-4");

match provider.execute(provider_request).await {
    Ok(response) => {
        timer.complete_success(
            response.usage.prompt_tokens,
            response.usage.completion_tokens
        );
    }
    Err(e) => {
        timer.complete_error(e.to_string());
    }
}
```

### Retry Metrics

```rust
use simple_agents_providers::metrics::record_retry;
use std::time::Duration;

// Record retry with backoff duration
record_retry("openai", 0.5);  // 500ms backoff
```

### Prometheus Exporter

```rust
use simple_agents_providers::metrics::prometheus;

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:9090".parse().unwrap();
    prometheus::init(addr).expect("Failed to start Prometheus exporter");

    // Metrics now available at http://127.0.0.1:9090/metrics
}
```

## Advanced Features

### Secure API Key Handling

```rust
let api_key = ApiKey::new("sk-...")?;

// API keys are:
// - Validated (min 20 chars, no null bytes)
// - Never logged in Debug output
// - Never serialized in plain text
// - Compared in constant time (prevents timing attacks)

// Get a preview (safe for logging)
println!("{}", api_key.preview()); // "sk-1234*** (42 chars)"

// Expose only when needed
let header_value = format!("Bearer {}", api_key.expose());
```

### Performance Tips

1. **Reuse Providers**: Create once, use many times
2. **Enable Caching**: Reduce API calls for repeated requests
3. **Use Connection Pooling**: Automatically enabled for OpenAI
4. **Batch Requests**: Send multiple messages in one request when possible
5. **Set Reasonable Limits**: Use `max_tokens` to control costs

### Example: Complete Application

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_cache::InMemoryCache;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;
    let cache = InMemoryCache::new(10 * 1024 * 1024, 1000);

    // Build request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("What is Rust?"))
        .temperature(0.7)
        .max_tokens(150)
        .build()?;

    // Generate cache key
    let cache_key = simple_agents_types::cache::CacheKey::from_parts(
        "openai",
        &request.model,
        &serde_json::to_string(&request.messages)?
    );

    // Check cache
    let response = if let Some(cached) = cache.get(&cache_key).await? {
        println!("Cache hit!");
        serde_json::from_slice(&cached)?
    } else {
        println!("Cache miss, calling API...");

        // Execute request
        let provider_request = provider.transform_request(&request)?;
        let provider_response = provider.execute(provider_request).await?;
        let response = provider.transform_response(provider_response)?;

        // Cache the response
        let response_bytes = serde_json::to_vec(&response)?;
        cache.set(&cache_key, response_bytes, Duration::from_secs(3600)).await?;

        response
    };

    // Use the response
    println!("Response: {}", response.content().unwrap_or(""));
    println!("Tokens used: {}", response.usage.total_tokens);

    Ok(())
}
```

## Next Steps

- Read [DEVELOPMENT.md](DEVELOPMENT.md) to contribute
- Check [API.md](API.md) for detailed API reference
- See [EXAMPLES.md](EXAMPLES.md) for more code examples
- Review [ARCHITECTURE.md](ARCHITECTURE.md) to understand the design
