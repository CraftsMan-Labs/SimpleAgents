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
- [Advanced Features](#advanced-features)

## Installation

Add SimpleAgents to your `Cargo.toml`:

```toml
[dependencies]
simple-agents-types = "0.1.0"
simple-agents-providers = "0.1.0"
simple-agents-cache = "0.1.0"
tokio = { version = "1.35", features = ["full"] }
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
    .message(Message::user("What is the capital of France?"))
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

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::system("You are a helpful assistant."))
    .message(Message::user("What is 2+2?"))
    .message(Message::assistant("4"))
    .message(Message::user("What is 3+3?"))
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
