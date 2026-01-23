# Development Guide

This guide is for developers who want to contribute to SimpleAgents or understand its internals.

## Table of Contents

- [Project Structure](#project-structure)
- [Building](#building)
- [Testing](#testing)
- [Contributing](#contributing)
- [Code Style](#code-style)
- [Security Considerations](#security-considerations)
- [Performance Guidelines](#performance-guidelines)
- [Adding a New Provider](#adding-a-new-provider)

## Project Structure

SimpleAgents is a Rust workspace with multiple crates:

```
SimpleAgents/
├── crates/
│   ├── simple-agents-types/       # Core types and traits
│   │   ├── src/
│   │   │   ├── lib.rs              # Crate root
│   │   │   ├── cache.rs            # Cache trait
│   │   │   ├── coercion.rs         # Response coercion
│   │   │   ├── config.rs           # Configuration types
│   │   │   ├── error.rs            # Error types
│   │   │   ├── message.rs          # Message types
│   │   │   ├── provider.rs         # Provider trait
│   │   │   ├── request.rs          # Request types
│   │   │   ├── response.rs         # Response types
│   │   │   ├── router.rs           # Routing types
│   │   │   └── validation.rs       # Validation (ApiKey, etc.)
│   │   └── Cargo.toml
│   │
│   ├── simple-agents-providers/    # Provider implementations
│   │   ├── src/
│   │   │   ├── lib.rs              # Crate root
│   │   │   ├── openai/             # OpenAI provider
│   │   │   │   ├── mod.rs          # Provider implementation
│   │   │   │   ├── models.rs       # Request/response models
│   │   │   │   └── error.rs        # Error mapping
│   │   │   ├── anthropic/          # Anthropic provider (stub)
│   │   │   ├── retry.rs            # Retry logic
│   │   │   └── utils.rs            # Shared utilities
│   │   └── Cargo.toml
│   │
│   └── simple-agents-cache/        # Cache implementations
│       ├── src/
│       │   ├── lib.rs              # Crate root
│       │   ├── memory.rs           # In-memory cache
│       │   └── noop.rs             # No-op cache
│       └── Cargo.toml
│
├── docs/                           # Documentation
├── OPTIMISATION.md                 # Performance tracking
├── Cargo.toml                      # Workspace manifest
└── README.md                       # Project README
```

### Crate Responsibilities

**simple-agents-types**
- Defines all core types and traits
- No implementation-specific code
- No external provider dependencies
- Pure data structures and interfaces

**simple-agents-providers**
- Implements the `Provider` trait for different APIs
- Handles HTTP requests and responses
- Maps provider-specific errors
- Currently supports: OpenAI (full), Anthropic (stub)

**simple-agents-cache**
- Implements the `Cache` trait
- Provides various caching strategies
- Currently supports: InMemory (LRU), NoOp

## Building

### Prerequisites

- Rust 1.75 or later
- Cargo (comes with Rust)

### Build All Crates

```bash
cargo build --all
```

### Build with Release Optimizations

```bash
cargo build --all --release
```

### Build Specific Crate

```bash
cargo build -p simple-agents-types
cargo build -p simple-agents-providers
cargo build -p simple-agents-cache
```

### Check Without Building

```bash
cargo check --all
```

## Testing

### Run All Tests

```bash
cargo test --all
```

**Current Test Count:** 132+ tests

### Run Tests for Specific Crate

```bash
cargo test -p simple-agents-types
cargo test -p simple-agents-providers
cargo test -p simple-agents-cache
```

### Run Specific Test

```bash
cargo test test_api_key_constant_time_comparison
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

### Run Ignored Tests

Integration tests that require API keys or local servers are ignored by default:

```bash
cargo test -- --ignored
```

### Run Doc Tests

```bash
cargo test --doc
```

### Test Coverage

We use various test types:

- **Unit tests**: In each module (`#[cfg(test)] mod tests`)
- **Integration tests**: In `tests/` directory
- **Doc tests**: In documentation comments
- **Property-based tests**: Using fuzzing (future)

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Arrange
        let input = "test";

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_async_functionality() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

## Contributing

### Getting Started

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `cargo test --all`
5. Run clippy: `cargo clippy --all`
6. Run fmt: `cargo fmt --all`
7. Commit: `git commit -m "Add my feature"`
8. Push: `git push origin feature/my-feature`
9. Create a Pull Request

### Commit Message Format

Use conventional commits:

```
feat: add streaming support for OpenAI
fix: correct cache key generation
docs: update usage guide
test: add tests for retry logic
refactor: simplify error handling
perf: optimize message cloning
```

### Pull Request Checklist

- [ ] Tests pass (`cargo test --all`)
- [ ] No clippy warnings (`cargo clippy --all`)
- [ ] Code formatted (`cargo fmt --all`)
- [ ] Documentation updated
- [ ] OPTIMISATION.md updated (if performance-related)
- [ ] Examples added (if new feature)
- [ ] Breaking changes documented

## Code Style

### Formatting

Use `rustfmt` with default settings:

```bash
cargo fmt --all
```

### Linting

Use `clippy` with strict settings:

```bash
cargo clippy --all -- -D warnings
```

### Documentation

- All public items must have doc comments
- Use examples in doc comments
- Include usage examples for complex features
- Document panics, errors, and safety invariants

```rust
/// Calculate the factorial of a number.
///
/// # Arguments
///
/// * `n` - The number to calculate factorial for
///
/// # Returns
///
/// The factorial of `n`
///
/// # Examples
///
/// ```
/// let result = factorial(5);
/// assert_eq!(result, 120);
/// ```
///
/// # Panics
///
/// Panics if `n > 20` (would overflow)
pub fn factorial(n: u64) -> u64 {
    // Implementation
}
```

### Naming Conventions

- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`

### Error Handling

- Use `Result` for recoverable errors
- Use `panic!` only for programming errors
- Provide context in error messages
- Use `thiserror` for error types

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("validation failed: {0}")]
    Validation(String),

    #[error("network error: {0}")]
    Network(#[from] std::io::Error),
}
```

## Security Considerations

### API Key Handling

**NEVER:**
- Log API keys
- Print API keys to stdout
- Store API keys in version control
- Use non-constant-time comparisons

**ALWAYS:**
- Use `ApiKey` type for all keys
- Call `.expose()` only when needed
- Redact keys in debug output
- Use constant-time comparisons

```rust
// Good
let key = ApiKey::new(env::var("API_KEY")?)?;
let header = format!("Bearer {}", key.expose());

// Bad
let key = env::var("API_KEY")?;
println!("Key: {}", key); // NEVER DO THIS
```

### Input Validation

All user input must be validated:

```rust
// Validate before use
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user(user_input))
    .build()?; // Validates automatically
```

### Timing Attacks

Use constant-time operations for security-sensitive comparisons:

```rust
use subtle::ConstantTimeEq;

// Good: constant-time
self.0.as_bytes().ct_eq(other.0.as_bytes()).into()

// Bad: timing attack vulnerable
self.0 == other.0
```

### Random Number Generation

Use cryptographically secure RNG:

```rust
use rand::Rng;

// Good
rand::thread_rng().gen()

// Bad
SystemTime::now() // Predictable!
```

## Performance Guidelines

### Zero-Copy Operations

Prefer borrowing over cloning:

```rust
// Good: borrows data
pub struct Request<'a> {
    pub messages: &'a [Message],
}

// Bad: clones data
pub struct Request {
    pub messages: Vec<Message>,
}
```

### Static vs Dynamic Allocation

Use `Cow` for strings that are often static:

```rust
use std::borrow::Cow;

// Can be static (zero allocation) or owned
pub headers: Vec<(Cow<'static, str>, Cow<'static, str>)>

// Usage
headers.push((Cow::Borrowed("Content-Type"), Cow::Borrowed("application/json")));
```

### Connection Reuse

Configure HTTP clients for connection pooling:

```rust
Client::builder()
    .pool_max_idle_per_host(10)
    .pool_idle_timeout(Duration::from_secs(90))
    .http2_prior_knowledge()
    .build()?
```

### Caching Strategy

Implement caching for expensive operations:

```rust
// Cache key generation (fast)
let key = CacheKey::from_parts(provider, model, content);

// Check cache before API call
if let Some(cached) = cache.get(&key).await? {
    return Ok(cached);
}
```

### Profiling

Use profiling tools to identify bottlenecks:

```bash
# CPU profiling with flamegraph
cargo install flamegraph
cargo flamegraph --bin your_binary

# Memory profiling with valgrind
cargo build --release
valgrind --tool=massif target/release/your_binary
```

## Adding a New Provider

### Step 1: Create Module Structure

```bash
mkdir -p crates/simple-agents-providers/src/newprovider
touch crates/simple-agents-providers/src/newprovider/mod.rs
touch crates/simple-agents-providers/src/newprovider/models.rs
touch crates/simple-agents-providers/src/newprovider/error.rs
```

### Step 2: Define Request/Response Models

```rust
// models.rs
use serde::{Deserialize, Serialize};
use simple_agents_types::message::Message;

#[derive(Debug, Serialize)]
pub struct NewProviderRequest<'a> {
    pub model: &'a str,
    pub messages: &'a [Message],
    // ... provider-specific fields
}

#[derive(Debug, Deserialize)]
pub struct NewProviderResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    // ... provider-specific fields
}
```

### Step 3: Implement Error Mapping

```rust
// error.rs
use simple_agents_types::error::ProviderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NewProviderError {
    #[error("invalid API key")]
    InvalidApiKey,

    #[error("rate limit exceeded")]
    RateLimit,

    // ... other errors
}

impl From<NewProviderError> for ProviderError {
    fn from(err: NewProviderError) -> Self {
        match err {
            NewProviderError::InvalidApiKey => {
                ProviderError::Authentication("Invalid API key".to_string())
            }
            NewProviderError::RateLimit => {
                ProviderError::RateLimit {
                    retry_after: None,
                    message: "Rate limit exceeded".to_string(),
                }
            }
            // ... other mappings
        }
    }
}
```

### Step 4: Implement Provider Trait

```rust
// mod.rs
use async_trait::async_trait;
use simple_agents_types::prelude::*;

pub struct NewProvider {
    api_key: ApiKey,
    client: reqwest::Client,
}

impl NewProvider {
    pub fn new(api_key: ApiKey) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self { api_key, client })
    }
}

#[async_trait]
impl Provider for NewProvider {
    fn name(&self) -> &str {
        "newprovider"
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        // Transform to provider format
        let provider_req = NewProviderRequest {
            model: &req.model,
            messages: &req.messages,
        };

        let body = serde_json::to_value(&provider_req)?;

        Ok(ProviderRequest {
            url: "https://api.newprovider.com/v1/chat".to_string(),
            headers: vec![
                (Cow::Borrowed("Authorization"),
                 Cow::Owned(format!("Bearer {}", self.api_key.expose()))),
            ],
            body,
            timeout: None,
        })
    }

    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
        // Make HTTP request
        let response = self.client
            .post(&req.url)
            .json(&req.body)
            .send()
            .await?;

        // Handle errors
        if !response.status().is_success() {
            // Parse and map errors
        }

        // Parse response
        let body = response.json().await?;

        Ok(ProviderResponse {
            status: 200,
            body,
            headers: None,
        })
    }

    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
        // Transform from provider format
        let provider_resp: NewProviderResponse = serde_json::from_value(resp.body)?;

        // Map to unified format
        Ok(CompletionResponse {
            id: provider_resp.id,
            model: "model-name".to_string(),
            choices: vec![/* ... */],
            usage: Usage::new(0, 0),
            created: None,
            provider: Some(self.name().to_string()),
        })
    }
}
```

### Step 5: Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let key = ApiKey::new("test-key-1234567890").unwrap();
        let provider = NewProvider::new(key).unwrap();
        assert_eq!(provider.name(), "newprovider");
    }

    #[tokio::test]
    async fn test_request_transformation() {
        // Test transform_request
    }

    // Add more tests...
}
```

### Step 6: Update Documentation

1. Add provider to `simple-agents-providers/src/lib.rs`
2. Update `docs/USAGE.md` with examples
3. Add to README.md feature list

## Debugging

### Enable Logging

```rust
use tracing::info;
use tracing_subscriber;

// In your binary
tracing_subscriber::fmt::init();

// In library code
tracing::info!("Processing request for model: {}", model);
tracing::debug!("Request details: {:?}", request);
```

### Run with Logs

```bash
RUST_LOG=debug cargo run
RUST_LOG=simple_agents=trace cargo test
```

### Debugging Tests

```rust
#[test]
fn test_something() {
    let _ = env_logger::builder()
        .is_test(true)
        .try_init();

    // Your test code
}
```

## Release Process

1. Update version in all `Cargo.toml` files
2. Update CHANGELOG.md
3. Run full test suite: `cargo test --all`
4. Build release: `cargo build --all --release`
5. Tag release: `git tag v0.x.0`
6. Push tags: `git push --tags`
7. Publish crates: `cargo publish -p simple-agents-types`, etc.

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Async Book](https://rust-lang.github.io/async-book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Performance Book](https://nnethercote.github.io/perf-book/)
