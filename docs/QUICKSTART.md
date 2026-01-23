# Quick Start Guide

Get up and running with SimpleAgents in 5 minutes.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
simple-agents-types = "0.1.0"
simple-agents-providers = "0.1.0"
simple-agents-cache = "0.1.0"  # Optional
tokio = { version = "1.35", features = ["full"] }
```

## Basic Usage

### 1. Set Your API Key

```bash
export OPENAI_API_KEY="sk-..."
```

### 2. Create Your First Request

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    // Create request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello, AI!"))
        .build()?;

    // Execute
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    // Print result
    println!("{}", response.content().unwrap_or(""));

    Ok(())
}
```

### 3. Run It

```bash
cargo run
```

## Common Tasks

### Ask a Question

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("What is the capital of France?"))
    .build()?;
```

### Have a Conversation

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::system("You are a helpful assistant."))
    .message(Message::user("Hello!"))
    .message(Message::assistant("Hi! How can I help you?"))
    .message(Message::user("Tell me a joke."))
    .build()?;
```

### Control Creativity

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Write a poem"))
    .temperature(0.9)      // More creative
    .max_tokens(200)       // Limit length
    .build()?;
```

### Add Caching

```rust
use simple_agents_cache::InMemoryCache;
use std::time::Duration;

let cache = InMemoryCache::new(10 * 1024 * 1024, 1000);

// Before making request
let cache_key = simple_agents_types::cache::CacheKey::from_parts(
    "openai",
    "gpt-4",
    "What is Rust?"
);

if let Some(cached) = cache.get(&cache_key).await? {
    let response: CompletionResponse = serde_json::from_slice(&cached)?;
    return Ok(response);
}

// ... make request ...

// After getting response
let response_bytes = serde_json::to_vec(&response)?;
cache.set(&cache_key, response_bytes, Duration::from_secs(3600)).await?;
```

### Handle Errors

```rust
match provider.execute(provider_request).await {
    Ok(provider_response) => {
        let response = provider.transform_response(provider_response)?;
        println!("{}", response.content().unwrap_or(""));
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

### Add Retry Logic

```rust
use simple_agents_providers::retry::execute_with_retry;
use simple_agents_types::config::RetryConfig;

let config = RetryConfig::default();

let provider_response = execute_with_retry(
    &config,
    |e| e.is_retryable(),
    || provider.execute(provider_request.clone())
).await?;
```

## Configuration Options

### Temperature (0.0 - 2.0)

Controls randomness:
- `0.0` - Deterministic, focused
- `0.7` - Balanced (default)
- `2.0` - Very creative, random

```rust
.temperature(0.7)
```

### Max Tokens

Limits response length:

```rust
.max_tokens(500)  // Approximately 500 tokens (~375 words)
```

### Top P (0.0 - 1.0)

Alternative to temperature:

```rust
.top_p(0.9)  // Consider top 90% probable tokens
```

### Stop Sequences

Stop at specific text:

```rust
.stop(vec!["END".to_string(), "\n\n".to_string()])
```

## Next Steps

- Read [USAGE.md](USAGE.md) for comprehensive usage guide
- Check [EXAMPLES.md](EXAMPLES.md) for more code examples
- See [API.md](API.md) for complete API reference
- Review [ARCHITECTURE.md](ARCHITECTURE.md) for design details

## Common Issues

### "Invalid API key"

Make sure your API key is set:
```bash
echo $OPENAI_API_KEY
```

### "Validation error: too long"

Reduce message size or max_tokens:
```rust
.max_tokens(100)
```

### "Rate limit exceeded"

Add retry logic or wait before retrying:
```rust
use simple_agents_providers::retry::execute_with_retry;
```

### "Connection timeout"

The default timeout is 30 seconds. For longer requests, this is automatic.

## Getting Help

- **Documentation**: Read the docs in `docs/`
- **Examples**: Check `docs/EXAMPLES.md`
- **Issues**: Open an issue on GitHub
- **API Reference**: See `docs/API.md`
