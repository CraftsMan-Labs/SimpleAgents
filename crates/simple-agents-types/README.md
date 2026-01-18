# simple-agents-types

Core types and traits for the SimpleAgents LLM framework.

## Overview

`simple-agents-types` is the foundational crate of SimpleAgents, providing all core types, traits, and error definitions. It has **zero runtime dependencies** - no HTTP client, no I/O, no async runtime - just pure types and traits.

## Features

- **Type Safety**: Strong types prevent common errors
- **Transparency**: All transformations tracked via `CoercionFlag`
- **Security**: API keys never logged (via secure `ApiKey` type)
- **Validation**: Early validation with clear error messages
- **Async**: All traits use `async_trait` for async support
- **Serialization**: Full serde support for all types

## Architecture

SimpleAgents follows a trait-based architecture with three main traits:

1. **`Provider`** - LLM provider implementations (OpenAI, Anthropic, etc.)
2. **`Cache`** - Response caching (in-memory, Redis, etc.)
3. **`RoutingStrategy`** - Provider selection logic (round-robin, latency-based, etc.)

## Quick Start

```rust
use simple_agents_types::prelude::*;

// Create a request
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Hello, world!"))
    .temperature(0.7)
    .build()
    .unwrap();

// Access request properties
assert_eq!(request.model, "gpt-4");
assert_eq!(request.messages.len(), 1);
```

## Core Types

### Messages

```rust
use simple_agents_types::message::{Message, Role};

let user_msg = Message::user("What is 2+2?");
let assistant_msg = Message::assistant("4");
let system_msg = Message::system("You are a helpful assistant.");
```

### Requests

```rust
use simple_agents_types::request::CompletionRequest;
use simple_agents_types::message::Message;

let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Hello!"))
    .temperature(0.7)
    .max_tokens(100)
    .build()?;
```

### Responses

```rust
use simple_agents_types::response::CompletionResponse;

// Get the first completion's content
if let Some(content) = response.content() {
    println!("Response: {}", content);
}
```

### Error Handling

```rust
use simple_agents_types::error::{SimpleAgentsError, ProviderError};

match result {
    Ok(response) => println!("{}", response.content().unwrap()),
    Err(SimpleAgentsError::Provider(ProviderError::RateLimit { retry_after })) => {
        println!("Rate limited, retry after {:?}", retry_after);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

### API Key Security

```rust
use simple_agents_types::validation::ApiKey;

let key = ApiKey::new("sk-...")?;

// Never logged
println!("{:?}", key); // Output: ApiKey([REDACTED])

// Explicit access only
let raw_key = key.expose(); // Use for API calls only
```

### Coercion Tracking

```rust
use simple_agents_types::coercion::{CoercionResult, CoercionFlag};

let result = CoercionResult::new(42)
    .with_flag(CoercionFlag::StrippedMarkdown)
    .set_confidence(0.95);

if result.was_coerced() {
    println!("Applied {} coercions", result.flags.len());
}
```

## Module Structure

- **`message`** - Message types (`Role`, `Message`)
- **`request`** - Request types (`CompletionRequest`, builder)
- **`response`** - Response types (`CompletionResponse`, `Usage`, streaming)
- **`error`** - Error hierarchy (`SimpleAgentsError`, `ProviderError`, `HealingError`)
- **`provider`** - Provider trait and types
- **`cache`** - Cache trait
- **`router`** - Routing strategy trait
- **`config`** - Configuration types (`RetryConfig`, `HealingConfig`, `Capabilities`)
- **`validation`** - Validation types (`ApiKey`)
- **`coercion`** - Coercion tracking (`CoercionFlag`, `CoercionResult`)

## Traits

### Provider Trait

```rust
use simple_agents_types::provider::Provider;
use async_trait::async_trait;

#[async_trait]
impl Provider for MyProvider {
    fn name(&self) -> &str { "my-provider" }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        // Transform to provider format
    }

    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
        // Make HTTP request
    }

    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
        // Transform from provider format
    }
}
```

### Cache Trait

```rust
use simple_agents_types::cache::Cache;
use async_trait::async_trait;

#[async_trait]
impl Cache for MyCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> { /* ... */ }
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()> { /* ... */ }
    async fn delete(&self, key: &str) -> Result<()> { /* ... */ }
    async fn clear(&self) -> Result<()> { /* ... */ }
}
```

### RoutingStrategy Trait

```rust
use simple_agents_types::router::RoutingStrategy;
use async_trait::async_trait;

#[async_trait]
impl RoutingStrategy for MyRouter {
    async fn select_provider(
        &self,
        providers: &[ProviderConfig],
        request: &CompletionRequest,
    ) -> Result<usize> {
        // Select provider index
    }
}
```

## Design Principles

1. **Transparency First** - Every transformation is tracked
2. **Type Safety** - Strong types prevent errors
3. **Performance** - Zero-cost abstractions
4. **Fail Fast** - Validate early, error clearly
5. **Security** - Sensitive data never logged

## Testing

All types include comprehensive unit tests:

```bash
cargo test
```

Run clippy and format checks:

```bash
cargo clippy -- -D warnings
cargo fmt -- --check
```

## Dependencies

Minimal dependencies for maximum compatibility:

- `serde` - Serialization
- `serde_json` - JSON support
- `thiserror` - Error handling
- `async-trait` - Async trait support

## License

MIT OR Apache-2.0

## Next Steps

After `simple-agents-types`, the next crates to implement are:

1. `simple-agents-providers` - OpenAI, Anthropic, etc.
2. `simple-agents-healing` - JSON parser with coercion
3. `simple-agents-router` - Retry and fallback logic
4. `simple-agents-core` - Client and completion API
