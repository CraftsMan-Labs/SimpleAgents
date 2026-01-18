# SimpleAgents Coding Guidelines

**Version**: 1.0
**Last Updated**: 2026-01-16
**Status**: Production-Grade System Standards

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Core Principles](#core-principles)
3. [Rust Best Practices](#rust-best-practices)
4. [Error Handling](#error-handling)
5. [Type Safety & Schema Design](#type-safety--schema-design)
6. [Async Patterns](#async-patterns)
7. [Performance Guidelines](#performance-guidelines)
8. [Testing Standards](#testing-standards)
9. [FFI & Memory Safety](#ffi--memory-safety)
10. [Documentation Requirements](#documentation-requirements)
11. [Security Practices](#security-practices)
12. [Code Organization](#code-organization)
13. [Response Healing System](#response-healing-system)
14. [Streaming Implementation](#streaming-implementation)
15. [Provider Integration](#provider-integration)
16. [Review Checklist](#review-checklist)

---

## Project Overview

**SimpleAgents** is a high-performance Rust LLM gateway that combines:
- **LiteLLM's** multi-provider abstraction, routing, and reliability
- **BAML's** flexible JSON parsing and response healing (SAP - Structured Argument Parsing)
- Simple API with FFI bindings for Python, Go, TypeScript, JavaScript

**Key Differentiators**:
1. Response healing system (handles malformed LLM outputs)
2. Streaming with structured schemas and partial types
3. Production-grade reliability (retry, fallback, caching)
4. Cross-language support via FFI

---

## Core Principles

### 1. **Transparency First**
- Every transformation must be tracked (flag system)
- Users should know when healing/coercion occurs
- Confidence scores required for all transformations

### 2. **Type Safety Without Compromise**
- Leverage Rust's type system fully
- Use derive macros for schemas
- Compile-time guarantees over runtime checks

### 3. **Performance by Default**
- Zero-cost abstractions
- Async I/O for all network operations
- Minimal allocations in hot paths

### 4. **Fail Fast, Recover Smart**
- Validate early, error clearly
- Automatic retries with exponential backoff
- Graceful degradation (fallback chains)

### 5. **FFI Safety is Non-Negotiable**
- Clear ownership rules
- No shared mutable state across boundaries
- Comprehensive contract tests

---

## Rust Best Practices

### Code Style

```rust
// Use Rust 2021 edition
edition = "2021"

// Follow Rust API Guidelines
// https://rust-lang.github.io/api-guidelines/

// Example: Good struct design
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CompletionRequest {
    /// Messages in the conversation
    pub messages: Vec<Message>,

    /// Model identifier (e.g., "gpt-4", "claude-3-opus")
    pub model: String,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature (0.0-2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}
```

### Naming Conventions

```rust
// Types: PascalCase
struct CompletionRequest { }
enum ProviderError { }
trait Provider { }

// Functions/methods: snake_case
fn transform_request() { }
async fn execute_completion() { }

// Constants: SCREAMING_SNAKE_CASE
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_RETRIES: u32 = 3;

// Modules: snake_case
mod response_healing;
mod provider_abstraction;
```

### Attribute Guidelines

```rust
// Always derive Debug for non-FFI types
#[derive(Debug)]

// Use thiserror for error types
#[derive(thiserror::Error, Debug)]
pub enum SimpleAgentsError {
    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}

// Document public APIs
/// Transforms a unified request into provider-specific format.
///
/// # Arguments
///
/// * `request` - The unified completion request
///
/// # Returns
///
/// Provider-specific request object
///
/// # Errors
///
/// Returns `ProviderError::InvalidModel` if model not supported
pub fn transform_request(request: &CompletionRequest) -> Result<ProviderRequest>;
```

### Ownership Patterns

```rust
// Prefer borrowing over cloning
fn process_message(msg: &Message) -> String { }

// Use Arc for shared ownership across threads
struct Client {
    config: Arc<ClientConfig>,
    cache: Arc<dyn Cache>,
}

// Use Cow for conditional cloning
use std::borrow::Cow;

fn normalize_model<'a>(model: &'a str) -> Cow<'a, str> {
    if model.starts_with("openai/") {
        Cow::Owned(model.trim_start_matches("openai/").to_string())
    } else {
        Cow::Borrowed(model)
    }
}
```

---

## Error Handling

### Error Type Hierarchy

```rust
use thiserror::Error;

/// Top-level error type for all SimpleAgents operations
#[derive(Error, Debug)]
pub enum SimpleAgentsError {
    /// Provider-related errors (API failures, invalid responses)
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    /// Response healing/parsing errors
    #[error("Healing error: {0}")]
    Healing(#[from] HealingError),

    /// Network/timeout errors
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Provider-specific errors
#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Rate limit exceeded (retry after {retry_after:?})")]
    RateLimit { retry_after: Option<Duration> },

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}

/// Healing-specific errors
#[derive(Error, Debug)]
pub enum HealingError {
    #[error("Failed to parse JSON: {source}")]
    ParseFailed { source: String, input: String },

    #[error("Type coercion failed: cannot convert {from} to {to}")]
    CoercionFailed { from: String, to: String },

    #[error("Confidence {confidence} below threshold {threshold}")]
    LowConfidence { confidence: f32, threshold: f32 },
}
```

### Error Handling Patterns

```rust
// Use Result for recoverable errors
pub fn parse_response(json: &str) -> Result<CompletionResponse, HealingError> {
    // ...
}

// Use Option for optional values (not errors)
pub fn get_cached_response(&self, key: &str) -> Option<CompletionResponse> {
    // ...
}

// Context with anyhow for application-level code
use anyhow::{Context, Result};

async fn main_workflow() -> Result<()> {
    let config = load_config()
        .context("Failed to load configuration")?;

    let client = SimpleAgentsClient::new(config)
        .context("Failed to initialize client")?;

    Ok(())
}

// Never panic in library code (except for unrecoverable bugs)
// Use expect() only with clear messages for invariants
let value = map.get(key)
    .expect("BUG: key must exist after validation");

// Avoid unwrap() - use ? or match
// BAD:
let response = client.complete(request).await.unwrap();

// GOOD:
let response = client.complete(request).await?;
// OR:
let response = match client.complete(request).await {
    Ok(r) => r,
    Err(e) => {
        log::error!("Completion failed: {}", e);
        return Err(e);
    }
};
```

### Retryable Errors

```rust
/// Determines if an error is retryable
pub trait RetryableError {
    fn is_retryable(&self) -> bool;
    fn retry_after(&self) -> Option<Duration>;
}

impl RetryableError for ProviderError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            ProviderError::RateLimit { .. } |
            ProviderError::Timeout(_) |
            ProviderError::ServerError(_)
        )
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            ProviderError::RateLimit { retry_after } => *retry_after,
            _ => None,
        }
    }
}
```

---

## Type Safety & Schema Design

### Schema Derive Macro

```rust
use simple_agents_macros::Schema;

/// Character schema with validation
#[derive(Schema, Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    /// Character name (required, 1-100 chars)
    #[schema(required, min_length = 1, max_length = 100)]
    pub name: String,

    /// Age in years (required, 0-1000)
    #[schema(required, range(min = 0, max = 1000))]
    pub age: u32,

    /// List of abilities (optional, defaults to empty)
    #[schema(default = "Vec::new")]
    pub abilities: Vec<String>,

    /// Backstory (optional, custom validation)
    #[schema(validate = "validate_backstory")]
    pub backstory: Option<String>,

    /// Status (streaming: only emit when complete)
    #[schema(stream_done)]
    pub status: Status,

    /// ID (streaming: don't emit until non-null)
    #[schema(stream_not_null)]
    pub id: String,
}

fn validate_backstory(s: &str) -> Result<(), ValidationError> {
    if s.len() < 10 {
        Err(ValidationError::new("Backstory must be at least 10 characters"))
    } else {
        Ok(())
    }
}

#[derive(Schema, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Active,
    Inactive,
    Pending,
}
```

### Partial Types for Streaming

```rust
// Derive macro auto-generates partial types
// Original: Character
// Generated: PartialCharacter (all fields as Option<T>)

impl Character {
    /// Validates a partial character and returns confidence
    pub fn from_partial(partial: PartialCharacter) -> Result<Self, ValidationError> {
        Ok(Self {
            name: partial.name
                .ok_or_else(|| ValidationError::missing_field("name"))?,
            age: partial.age
                .ok_or_else(|| ValidationError::missing_field("age"))?,
            abilities: partial.abilities.unwrap_or_default(),
            backstory: partial.backstory,
            status: partial.status
                .ok_or_else(|| ValidationError::missing_field("status"))?,
            id: partial.id
                .ok_or_else(|| ValidationError::missing_field("id"))?,
        })
    }
}
```

### Type Safety Rules

1. **No stringly-typed APIs**: Use enums, not strings
   ```rust
   // BAD:
   fn set_model(model: &str) { }

   // GOOD:
   #[derive(Debug, Clone, Copy)]
   pub enum Model {
       Gpt4,
       Gpt35Turbo,
       Claude3Opus,
   }

   impl Model {
       pub fn as_str(&self) -> &'static str {
           match self {
               Model::Gpt4 => "gpt-4",
               Model::Gpt35Turbo => "gpt-3.5-turbo",
               Model::Claude3Opus => "claude-3-opus-20240229",
           }
       }
   }
   ```

2. **Builder pattern for complex construction**
   ```rust
   pub struct CompletionRequestBuilder {
       messages: Vec<Message>,
       model: Option<String>,
       max_tokens: Option<u32>,
       temperature: Option<f32>,
   }

   impl CompletionRequestBuilder {
       pub fn new() -> Self { Self::default() }

       pub fn model(mut self, model: impl Into<String>) -> Self {
           self.model = Some(model.into());
           self
       }

       pub fn messages(mut self, messages: Vec<Message>) -> Self {
           self.messages = messages;
           self
       }

       pub fn build(self) -> Result<CompletionRequest, ValidationError> {
           Ok(CompletionRequest {
               messages: self.messages,
               model: self.model
                   .ok_or_else(|| ValidationError::missing_field("model"))?,
               max_tokens: self.max_tokens,
               temperature: self.temperature,
           })
       }
   }
   ```

3. **Newtype pattern for validation**
   ```rust
   /// API key (validated, never logged)
   #[derive(Clone)]
   pub struct ApiKey(String);

   impl ApiKey {
       pub fn new(key: impl Into<String>) -> Result<Self, ValidationError> {
           let key = key.into();
           if key.is_empty() {
               return Err(ValidationError::new("API key cannot be empty"));
           }
           if key.len() < 20 {
               return Err(ValidationError::new("API key too short"));
           }
           Ok(Self(key))
       }

       pub fn as_str(&self) -> &str {
           &self.0
       }
   }

   // Never log API keys
   impl fmt::Debug for ApiKey {
       fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
           write!(f, "ApiKey([REDACTED])")
       }
   }
   ```

---

## Async Patterns

### Trait Design

```rust
use async_trait::async_trait;

/// Provider trait for LLM integrations
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g., "openai", "anthropic")
    fn name(&self) -> &str;

    /// Transform unified request to provider format
    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest>;

    /// Execute request against provider API
    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse>;

    /// Transform provider response to unified format
    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse>;

    /// Get retry configuration
    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }

    /// Get provider capabilities
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
}
```

### Async Best Practices

```rust
// Use tokio for async runtime
use tokio;

// Prefer async/await over raw futures
// BAD:
fn complete(req: Request) -> impl Future<Output = Result<Response>> { }

// GOOD:
async fn complete(req: Request) -> Result<Response> { }

// Use tokio::spawn for concurrent tasks
let task1 = tokio::spawn(async { provider1.execute(req).await });
let task2 = tokio::spawn(async { provider2.execute(req).await });

let (result1, result2) = tokio::try_join!(task1, task2)?;

// Use tokio::select for racing operations
use tokio::time::{timeout, Duration};

async fn complete_with_timeout(req: Request) -> Result<Response> {
    let timeout_duration = Duration::from_secs(30);

    tokio::select! {
        result = execute_request(req) => result,
        _ = tokio::time::sleep(timeout_duration) => {
            Err(ProviderError::Timeout(timeout_duration))
        }
    }
}

// Use bounded channels for backpressure
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::channel::<CompletionChunk>(100);

// Streaming with backpressure
async fn stream_response(tx: mpsc::Sender<CompletionChunk>) -> Result<()> {
    let mut stream = api_client.stream().await?;

    while let Some(chunk) = stream.next().await {
        // This will block if buffer is full (backpressure)
        tx.send(chunk?).await
            .map_err(|_| ProviderError::StreamClosed)?;
    }

    Ok(())
}
```

### Cancellation Safety

```rust
// Ensure operations are cancellation-safe
async fn cancellation_safe_operation() -> Result<()> {
    // BAD: Will lose data if cancelled between calls
    let data = fetch_data().await?;
    store_data(data).await?;

    // GOOD: Atomic or with cleanup
    let data = fetch_data().await?;
    let result = store_data(data).await;

    if result.is_err() {
        cleanup().await?;
    }

    result
}

// Use tokio::select with proper cleanup
tokio::select! {
    result = operation() => {
        // Handle result
    }
    _ = cancel_token.cancelled() => {
        // Cleanup before exit
        cleanup().await?;
    }
}
```

---

## Performance Guidelines

### Hot Path Optimization

```rust
// Avoid allocations in hot paths
// BAD:
fn process_chunk(chunk: &str) -> String {
    format!("Processed: {}", chunk) // Allocates every time
}

// GOOD:
fn process_chunk(chunk: &str, buffer: &mut String) {
    buffer.clear();
    buffer.push_str("Processed: ");
    buffer.push_str(chunk);
}

// Use string slicing instead of allocation
// BAD:
let trimmed = json.trim().to_string();

// GOOD:
let trimmed = json.trim(); // &str, no allocation

// Pre-allocate when size is known
let mut results = Vec::with_capacity(expected_size);

// Use Cow for conditional allocation
use std::borrow::Cow;

fn normalize<'a>(input: &'a str) -> Cow<'a, str> {
    if needs_normalization(input) {
        Cow::Owned(normalize_owned(input))
    } else {
        Cow::Borrowed(input)
    }
}
```

### Caching Strategy

```rust
use dashmap::DashMap;
use std::sync::Arc;

/// LRU cache with TTL
pub struct InMemoryCache {
    data: Arc<DashMap<String, CachedValue>>,
    max_size: usize,
}

#[derive(Clone)]
struct CachedValue {
    data: Vec<u8>,
    expires_at: Instant,
    access_count: Arc<AtomicU64>,
}

impl InMemoryCache {
    /// Get value from cache (updates access time)
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let entry = self.data.get(key)?;

        if entry.expires_at < Instant::now() {
            self.data.remove(key);
            return None;
        }

        entry.access_count.fetch_add(1, Ordering::Relaxed);
        Some(entry.data.clone())
    }

    /// Set value in cache with TTL
    pub async fn set(&self, key: String, value: Vec<u8>, ttl: Duration) {
        if self.data.len() >= self.max_size {
            self.evict_lru().await;
        }

        self.data.insert(key, CachedValue {
            data: value,
            expires_at: Instant::now() + ttl,
            access_count: Arc::new(AtomicU64::new(1)),
        });
    }

    async fn evict_lru(&self) {
        // Evict least recently used entries
        // Implementation omitted for brevity
    }
}
```

### Streaming Performance

```rust
use futures::StreamExt;

/// Efficient streaming with chunked processing
pub async fn stream_completion(
    provider: &dyn Provider,
    request: CompletionRequest,
) -> impl Stream<Item = Result<CompletionChunk>> {
    let response = provider.execute_stream(request).await;

    response
        .chunks_timeout(100, Duration::from_millis(50))
        .map(|chunks| {
            // Process batch of chunks together
            process_chunk_batch(chunks)
        })
}

/// Process partial JSON incrementally
pub struct StreamingParser {
    buffer: String,
    state: ParserState,
}

impl StreamingParser {
    pub fn feed(&mut self, chunk: &str) -> Vec<ParsedValue> {
        self.buffer.push_str(chunk);

        // Parse complete objects without re-parsing entire buffer
        let mut results = Vec::new();

        while let Some((value, consumed)) = self.try_parse_next() {
            results.push(value);
            self.buffer.drain(..consumed);
        }

        results
    }
}
```

---

## Testing Standards

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_parser_handles_markdown() {
        let input = r#"```json
        {"name": "test", "age": 25}
        ```"#;

        let parser = JsonishParser::new();
        let result = parser.parse(input).unwrap();

        assert_eq!(result["name"], "test");
        assert_eq!(result["age"], 25);
        assert!(parser.flags().contains(&CoercionFlag::StrippedMarkdown));
    }

    #[test]
    fn test_type_coercion_string_to_int() {
        let engine = CoercionEngine::new(Schema::for_type::<u32>());

        let result = engine.coerce(json!("42")).unwrap();

        assert_eq!(result.value, TypedValue::U32(42));
        assert!(result.flags.contains(&CoercionFlag::TypeCoercion {
            from: "string".into(),
            to: "u32".into(),
        }));
        assert!(result.confidence > 0.9);
    }

    #[tokio::test]
    async fn test_retry_on_timeout() {
        let mut provider = MockProvider::new();
        provider.expect_execute()
            .times(3)
            .returning(|_| Err(ProviderError::Timeout(Duration::from_secs(30))));

        let result = retry_with_backoff(&provider, request, RetryConfig::default()).await;

        assert!(matches!(result, Err(ProviderError::Timeout(_))));
    }
}
```

### Integration Tests

```rust
// tests/integration_test.rs

#[tokio::test]
#[ignore] // Requires API key
async fn test_openai_completion_real() {
    let api_key = env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY not set");

    let client = SimpleAgentsClient::builder()
        .provider("openai")
        .api_key(api_key)
        .build()
        .unwrap();

    let response = client
        .completion()
        .model("gpt-3.5-turbo")
        .messages(vec![Message::user("Say 'test'")])
        .send()
        .await
        .unwrap();

    assert!(!response.choices.is_empty());
    assert!(response.choices[0].message.content.contains("test"));
}

#[tokio::test]
async fn test_fallback_chain() {
    let client = SimpleAgentsClient::builder()
        .fallback_chain(vec![
            "provider-fail".into(),
            "provider-success".into(),
        ])
        .build()
        .unwrap();

    let response = client
        .completion()
        .model("test-model")
        .messages(vec![Message::user("test")])
        .send()
        .await
        .unwrap();

    assert_eq!(response.provider, "provider-success");
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_parser_never_panics(json in "\\PC*") {
        let parser = JsonishParser::new();
        let _ = parser.parse(&json); // Should never panic
    }

    #[test]
    fn test_coercion_confidence_bounded(
        value in any::<serde_json::Value>()
    ) {
        let engine = CoercionEngine::new(Schema::any());
        if let Ok(result) = engine.coerce(value) {
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
        }
    }
}
```

### Benchmark Tests

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_json_parser(c: &mut Criterion) {
    let malformed_json = r#"```json
    {"name": "test", "age": 25,}
    ```"#;

    c.bench_function("parse_malformed_json", |b| {
        b.iter(|| {
            let parser = JsonishParser::new();
            parser.parse(black_box(malformed_json))
        })
    });
}

criterion_group!(benches, benchmark_json_parser);
criterion_main!(benches);
```

---

## FFI & Memory Safety

### C FFI Layer

```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Opaque client pointer
pub struct SAClient {
    inner: SimpleAgentsClient,
}

/// Error codes
pub const SA_OK: i32 = 0;
pub const SA_ERR_INVALID_ARG: i32 = -1;
pub const SA_ERR_PROVIDER: i32 = -2;
pub const SA_ERR_TIMEOUT: i32 = -3;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = RefCell::new(None);
}

fn set_last_error(err: String) {
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(err));
}

/// Create new client
#[no_mangle]
pub unsafe extern "C" fn sa_client_new(
    provider: *const c_char,
    api_key: *const c_char,
) -> *mut SAClient {
    if provider.is_null() || api_key.is_null() {
        set_last_error("provider and api_key cannot be null".into());
        return std::ptr::null_mut();
    }

    let provider = match CStr::from_ptr(provider).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid UTF-8 in provider".into());
            return std::ptr::null_mut();
        }
    };

    let api_key = match CStr::from_ptr(api_key).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("Invalid UTF-8 in api_key".into());
            return std::ptr::null_mut();
        }
    };

    match SimpleAgentsClient::builder()
        .provider(provider)
        .api_key(api_key)
        .build()
    {
        Ok(client) => Box::into_raw(Box::new(SAClient { inner: client })),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Free client
#[no_mangle]
pub unsafe extern "C" fn sa_client_free(client: *mut SAClient) {
    if !client.is_null() {
        drop(Box::from_raw(client));
    }
}

/// Get last error
#[no_mangle]
pub extern "C" fn sa_get_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        match &*e.borrow() {
            Some(err) => {
                CString::new(err.as_str())
                    .unwrap()
                    .into_raw()
            }
            None => std::ptr::null(),
        }
    })
}

/// Free error string
#[no_mangle]
pub unsafe extern "C" fn sa_free_error(error: *mut c_char) {
    if !error.is_null() {
        drop(CString::from_raw(error));
    }
}
```

### FFI Safety Rules

1. **Never return Rust references across FFI**
2. **Always validate pointers before dereferencing**
3. **Use opaque pointers (Box::into_raw/from_raw)**
4. **Every `*_new()` must have `*_free()`**
5. **No panics in FFI functions (catch_unwind)**
6. **Thread-local error storage**
7. **Clear ownership documentation**

```rust
/// FFI-safe wrapper with panic catching
macro_rules! ffi_catch_unwind {
    ($body:block) => {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)) {
            Ok(result) => result,
            Err(_) => {
                set_last_error("Unexpected panic in FFI call".into());
                SA_ERR_INTERNAL
            }
        }
    };
}
```

---

## Documentation Requirements

### Public API Documentation

```rust
/// Client for interacting with LLM providers.
///
/// `SimpleAgentsClient` provides a unified interface for making completion requests
/// to various LLM providers (OpenAI, Anthropic, etc.) with automatic healing,
/// retry logic, and caching.
///
/// # Examples
///
/// ```
/// use simple_agents::prelude::*;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let client = SimpleAgentsClient::builder()
///         .provider("openai")
///         .api_key(std::env::var("OPENAI_API_KEY")?)
///         .build()?;
///
///     let response = client
///         .completion()
///         .model("gpt-4")
///         .messages(vec![Message::user("Hello!")])
///         .send()
///         .await?;
///
///     println!("{}", response.choices[0].message.content);
///     Ok(())
/// }
/// ```
///
/// # Error Handling
///
/// All methods return `Result<T, SimpleAgentsError>`. Common errors include:
/// - `SimpleAgentsError::Provider`: API failures, rate limits
/// - `SimpleAgentsError::Timeout`: Request exceeded timeout
/// - `SimpleAgentsError::Validation`: Invalid request parameters
///
/// # Thread Safety
///
/// `SimpleAgentsClient` is `Send + Sync` and can be shared across threads using `Arc`.
pub struct SimpleAgentsClient {
    // ...
}
```

### Module-Level Documentation

```rust
//! Response healing system for parsing malformed LLM outputs.
//!
//! This module implements BAML-inspired response healing, including:
//! - JSON-ish parser for handling markdown, trailing commas, etc.
//! - Type coercion engine for converting strings to typed values
//! - Confidence scoring and flag system for transparency
//!
//! # Example
//!
//! ```
//! use simple_agents_healing::{JsonishParser, CoercionEngine};
//!
//! let parser = JsonishParser::new();
//! let malformed = r#"```json
//! {"name": "test", "age": "25"}
//! ```"#;
//!
//! let result = parser.parse(malformed).unwrap();
//! assert_eq!(result["name"], "test");
//! ```
//!
//! # Architecture
//!
//! ```text
//! Raw LLM Output → Parser → Coercion → Validation → Typed Result
//!                    ↓         ↓           ↓
//!                  Flags   Confidence   Schema
//! ```
```

### Inline Documentation

```rust
// Explain complex algorithms
/// Implements exponential backoff with jitter.
///
/// Formula: `min(max_backoff, initial_backoff * multiplier^attempt * random(0.7, 1.3))`
///
/// Jitter prevents thundering herd problem when many clients retry simultaneously.
fn calculate_backoff(attempt: u32, config: &RetryConfig) -> Duration {
    // ...
}

// Document non-obvious behavior
/// Returns `None` if cache is disabled, even if value exists.
///
/// This is intentional to avoid cache poisoning attacks where disabled
/// caches still serve stale data.
pub fn get_cached(&self, key: &str) -> Option<Vec<u8>> {
    // ...
}
```

---

## Security Practices

### API Key Handling

```rust
/// API key (never logged or displayed)
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    /// Create API key from string (validated)
    pub fn new(key: impl Into<String>) -> Result<Self, ValidationError> {
        let key = key.into();

        // Validate format
        if key.is_empty() || key.len() < 20 {
            return Err(ValidationError::new("Invalid API key format"));
        }

        Ok(Self(key))
    }

    /// Get key as string (internal use only)
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

// Never log API keys
impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey([REDACTED])")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

// Don't serialize API keys in logs
impl Serialize for ApiKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}
```

### Input Validation

```rust
/// Validate and sanitize user input
pub fn validate_completion_request(req: &CompletionRequest) -> Result<()> {
    // Validate messages
    if req.messages.is_empty() {
        return Err(ValidationError::new("messages cannot be empty"));
    }

    if req.messages.len() > 1000 {
        return Err(ValidationError::new("too many messages (max 1000)"));
    }

    // Validate message content
    for msg in &req.messages {
        if msg.content.len() > 1_000_000 {
            return Err(ValidationError::new("message content too large (max 1MB)"));
        }

        // Prevent injection attacks
        if msg.content.contains('\0') {
            return Err(ValidationError::new("message content contains null bytes"));
        }
    }

    // Validate model name
    if !is_valid_model_name(&req.model) {
        return Err(ValidationError::new("invalid model name"));
    }

    // Validate numeric parameters
    if let Some(temp) = req.temperature {
        if !(0.0..=2.0).contains(&temp) {
            return Err(ValidationError::new("temperature must be 0.0-2.0"));
        }
    }

    Ok(())
}

fn is_valid_model_name(model: &str) -> bool {
    // Only allow alphanumeric, dash, underscore, dot
    model.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
}
```

### Rate Limiting

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    refill_rate: Duration,
}

impl RateLimiter {
    pub fn new(max_concurrent: usize, refill_rate: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            refill_rate,
        }
    }

    /// Acquire permit (blocks if rate limit exceeded)
    pub async fn acquire(&self) -> RateLimitPermit {
        let permit = self.semaphore.acquire().await.unwrap();

        // Refill after duration
        let semaphore = self.semaphore.clone();
        tokio::spawn(async move {
            tokio::time::sleep(self.refill_rate).await;
            drop(permit);
        });

        RateLimitPermit { _inner: () }
    }
}
```

---

## Code Organization

### Workspace Structure

```
simple-agents/
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── simple-agents-core/       # Main client API
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── client.rs
│   │   │   ├── completion.rs
│   │   │   └── streaming/
│   │   └── Cargo.toml
│   ├── simple-agents-providers/  # Provider implementations
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── openai.rs
│   │   │   ├── anthropic.rs
│   │   │   └── openrouter.rs
│   │   └── Cargo.toml
│   ├── simple-agents-healing/    # Response healing
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── parser.rs
│   │   │   ├── coercion.rs
│   │   │   ├── streaming.rs
│   │   │   └── partial_types.rs
│   │   └── Cargo.toml
│   ├── simple-agents-router/     # Routing & reliability
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── router.rs
│   │   │   ├── retry.rs
│   │   │   └── fallback.rs
│   │   └── Cargo.toml
│   ├── simple-agents-cache/      # Caching
│   ├── simple-agents-types/      # Shared types & traits
│   ├── simple-agents-ffi/        # C FFI layer
│   ├── simple-agents-macros/     # Derive macros
│   └── simple-agents-cli/        # CLI tool
└── bindings/
    ├── python/
    ├── node/
    └── go/
```

### Module Organization

```rust
// crates/simple-agents-core/src/lib.rs

//! SimpleAgents: High-performance Rust LLM gateway
//!
//! Main entry point and re-exports.

// Re-export public API
pub use client::SimpleAgentsClient;
pub use completion::{CompletionRequest, CompletionResponse};
pub use error::SimpleAgentsError;

// Prelude for common imports
pub mod prelude {
    pub use crate::{
        SimpleAgentsClient,
        CompletionRequest,
        CompletionResponse,
        SimpleAgentsError,
    };
    pub use simple_agents_types::{Message, Role};
}

// Internal modules
mod client;
mod completion;
mod error;
mod streaming;

// Re-export from other crates
pub use simple_agents_healing as healing;
pub use simple_agents_providers as providers;
pub use simple_agents_router as router;
```

---

## Response Healing System

### Parser Implementation

```rust
/// Three-phase JSON parsing strategy
pub struct JsonishParser {
    config: ParserConfig,
}

impl JsonishParser {
    /// Parse potentially malformed JSON
    ///
    /// Phases:
    /// 1. Strip & Fix: Remove markdown, fix trailing commas, quotes
    /// 2. Standard Parse: Try serde_json (fast path)
    /// 3. Lenient Parse: Character-by-character state machine
    pub fn parse(&self, input: &str) -> Result<(Value, Vec<CoercionFlag>), HealingError> {
        let mut flags = Vec::new();

        // Phase 1: Strip & Fix
        let cleaned = self.strip_and_fix(input, &mut flags)?;

        // Phase 2: Try standard parsing
        if let Ok(value) = serde_json::from_str(&cleaned) {
            return Ok((value, flags));
        }

        // Phase 3: Lenient parsing
        self.lenient_parse(&cleaned, &mut flags)
    }

    fn strip_and_fix(&self, input: &str, flags: &mut Vec<CoercionFlag>) -> Result<String, HealingError> {
        let mut output = input.to_string();

        // Remove markdown code blocks
        if output.trim_start().starts_with("```") {
            output = self.strip_markdown(&output);
            flags.push(CoercionFlag::StrippedMarkdown);
        }

        // Fix trailing commas
        if output.contains(",}") || output.contains(",]") {
            output = output.replace(",}", "}").replace(",]", "]");
            flags.push(CoercionFlag::FixedTrailingComma);
        }

        // Fix single quotes
        if output.contains('\'') && !output.contains('"') {
            output = output.replace('\'', "\"");
            flags.push(CoercionFlag::FixedQuotes);
        }

        Ok(output)
    }

    fn lenient_parse(&self, input: &str, flags: &mut Vec<CoercionFlag>) -> Result<(Value, Vec<CoercionFlag>), HealingError> {
        // State machine parser implementation
        // Handles incomplete JSON, unquoted keys, etc.
        todo!("Implement state machine parser")
    }
}
```

### Coercion Engine

```rust
/// Type coercion with confidence scoring
pub struct CoercionEngine {
    schema: Schema,
    config: CoercionConfig,
}

impl CoercionEngine {
    /// Coerce JSON value to match schema
    pub fn coerce(&self, value: Value) -> Result<CoercionResult> {
        let mut flags = Vec::new();
        let mut confidence = 1.0;

        let typed_value = self.coerce_recursive(&value, &self.schema, &mut flags, &mut confidence)?;

        Ok(CoercionResult {
            value: typed_value,
            flags,
            confidence,
        })
    }

    fn coerce_recursive(
        &self,
        value: &Value,
        schema: &Schema,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<TypedValue> {
        match (value, schema) {
            // Exact match - no coercion needed
            (Value::String(s), Schema::String) => Ok(TypedValue::String(s.clone())),

            // String to number coercion
            (Value::String(s), Schema::U32) => {
                let num = s.parse::<u32>()
                    .map_err(|_| HealingError::CoercionFailed {
                        from: "string".into(),
                        to: "u32".into(),
                    })?;

                flags.push(CoercionFlag::TypeCoercion {
                    from: "string".into(),
                    to: "u32".into(),
                });
                *confidence *= 0.9; // Reduce confidence

                Ok(TypedValue::U32(num))
            }

            // Fuzzy field matching for objects
            (Value::Object(map), Schema::Struct(fields)) => {
                self.coerce_struct(map, fields, flags, confidence)
            }

            // Union resolution (try all variants, pick best)
            (value, Schema::Union(variants)) => {
                self.resolve_union(value, variants, flags, confidence)
            }

            _ => Err(HealingError::CoercionFailed {
                from: format!("{:?}", value),
                to: format!("{:?}", schema),
            }),
        }
    }

    fn coerce_struct(
        &self,
        map: &serde_json::Map<String, Value>,
        fields: &[SchemaField],
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<TypedValue> {
        let mut result = BTreeMap::new();

        for field in fields {
            // Try exact match first
            if let Some(value) = map.get(&field.name) {
                let coerced = self.coerce_recursive(value, &field.schema, flags, confidence)?;
                result.insert(field.name.clone(), coerced);
                continue;
            }

            // Fuzzy field matching (case-insensitive, snake/camel)
            if let Some((found_key, value)) = self.fuzzy_field_match(map, &field.name) {
                flags.push(CoercionFlag::FuzzyFieldMatch {
                    expected: field.name.clone(),
                    found: found_key.clone(),
                });
                *confidence *= 0.95;

                let coerced = self.coerce_recursive(value, &field.schema, flags, confidence)?;
                result.insert(field.name.clone(), coerced);
                continue;
            }

            // Use default if available
            if let Some(default) = &field.default {
                flags.push(CoercionFlag::UsedDefaultValue {
                    field: field.name.clone(),
                });
                *confidence *= 0.9;

                result.insert(field.name.clone(), default.clone());
                continue;
            }

            // Required field missing
            if field.required {
                return Err(HealingError::MissingField {
                    field: field.name.clone(),
                });
            }
        }

        Ok(TypedValue::Struct(result))
    }

    fn fuzzy_field_match<'a>(
        &self,
        map: &'a serde_json::Map<String, Value>,
        expected: &str,
    ) -> Option<(&'a String, &'a Value)> {
        // Try case-insensitive match
        for (key, value) in map {
            if key.eq_ignore_ascii_case(expected) {
                return Some((key, value));
            }
        }

        // Try snake_case <-> camelCase conversion
        let expected_snake = to_snake_case(expected);
        let expected_camel = to_camel_case(expected);

        for (key, value) in map {
            if key == &expected_snake || key == &expected_camel {
                return Some((key, value));
            }
        }

        None
    }
}
```

---

## Streaming Implementation

### Streaming API

```rust
use futures::Stream;
use tokio::sync::mpsc;

/// Streaming completion response
pub struct CompletionStream {
    receiver: mpsc::Receiver<Result<CompletionChunk>>,
    finalizer: Option<Box<dyn FnOnce() -> Result<CompletionResponse>>>,
}

impl CompletionStream {
    /// Get next chunk
    pub async fn next(&mut self) -> Option<Result<CompletionChunk>> {
        self.receiver.recv().await
    }

    /// Finalize stream and get complete response
    pub fn finalize(mut self) -> Result<CompletionResponse> {
        self.finalizer
            .take()
            .ok_or_else(|| SimpleAgentsError::StreamAlreadyFinalized)?()
    }
}

impl Stream for CompletionStream {
    type Item = Result<CompletionChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

/// Stream with partial type extraction
pub struct TypedStream<T: Schema> {
    stream: CompletionStream,
    extractor: StreamingExtractor,
    _phantom: PhantomData<T>,
}

impl<T: Schema> TypedStream<T> {
    /// Get next partial value
    pub async fn next(&mut self) -> Option<Result<T::Partial>> {
        let chunk = self.stream.next().await?;

        match chunk {
            Ok(chunk) => {
                let partial = self.extractor.extract_partial::<T>(&chunk.content);
                Some(Ok(partial))
            }
            Err(e) => Some(Err(e)),
        }
    }

    /// Finalize and get complete typed value
    pub async fn finalize(self) -> Result<T> {
        let response = self.stream.finalize()?;
        T::from_str(&response.choices[0].message.content)
    }
}
```

### Partial Type Extraction

```rust
/// Streaming extractor with annotation support
pub struct StreamingExtractor {
    parser: StreamingParser,
    annotations: HashMap<String, StreamAnnotation>,
}

#[derive(Debug, Clone, Copy)]
pub enum StreamAnnotation {
    /// Emit field as soon as available
    Normal,
    /// Don't emit until non-null
    NotNull,
    /// Only emit when complete
    Done,
}

impl StreamingExtractor {
    /// Extract partial value from streamed JSON
    pub fn extract_partial<T: Schema>(&mut self, chunk: &str) -> T::Partial {
        // Feed chunk to parser
        let parsed = self.parser.feed(chunk);

        // Build partial value respecting annotations
        let mut partial = T::Partial::default();

        for (field, value) in parsed.fields() {
            if self.should_emit_field(field, value) {
                partial.set_field(field, value);
            }
        }

        partial
    }

    fn should_emit_field(&self, field: &str, value: &Value) -> bool {
        match self.annotations.get(field) {
            Some(StreamAnnotation::NotNull) => {
                !matches!(value, Value::Null)
            }
            Some(StreamAnnotation::Done) => {
                value.is_complete()
            }
            _ => true,
        }
    }
}

/// Incremental JSON parser
pub struct StreamingParser {
    buffer: String,
    state: ParserState,
}

impl StreamingParser {
    /// Feed chunk and extract complete values
    pub fn feed(&mut self, chunk: &str) -> PartialValue {
        self.buffer.push_str(chunk);

        // Try to extract complete objects/arrays
        self.parse_incremental()
    }

    fn parse_incremental(&mut self) -> PartialValue {
        // State machine to track depth, quotes, etc.
        // Extract complete values without re-parsing entire buffer
        todo!("Implement incremental parsing")
    }
}
```

---

## Provider Integration

### Provider Trait Implementation

```rust
use async_trait::async_trait;

/// OpenAI provider implementation
pub struct OpenAIProvider {
    api_key: ApiKey,
    base_url: String,
    http_client: reqwest::Client,
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        // Transform unified request to OpenAI format
        Ok(ProviderRequest {
            url: format!("{}/chat/completions", self.base_url),
            headers: vec![
                ("Authorization".into(), format!("Bearer {}", self.api_key.as_str())),
                ("Content-Type".into(), "application/json".into()),
            ],
            body: serde_json::to_value(&OpenAICompletionRequest {
                model: req.model.clone(),
                messages: req.messages.clone(),
                max_tokens: req.max_tokens,
                temperature: req.temperature,
                stream: req.stream.unwrap_or(false),
            })?,
        })
    }

    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in req.headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes())?,
                reqwest::header::HeaderValue::from_str(&value)?,
            );
        }

        let response = self.http_client
            .post(&req.url)
            .headers(headers)
            .json(&req.body)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        // Handle errors
        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await?);
        }

        Ok(ProviderResponse {
            status: response.status().as_u16(),
            headers: response.headers().clone(),
            body: response.json().await?,
        })
    }

    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
        let openai_response: OpenAICompletionResponse = serde_json::from_value(resp.body)?;

        Ok(CompletionResponse {
            id: openai_response.id,
            model: openai_response.model,
            choices: openai_response.choices,
            usage: openai_response.usage,
            provider: self.name().into(),
        })
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
            retryable_errors: vec![
                ErrorType::Timeout,
                ErrorType::RateLimit,
                ErrorType::ServerError,
            ],
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: true,
            function_calling: true,
            vision: true,
            max_tokens: 128_000,
        }
    }
}
```

### Provider Error Handling

```rust
impl OpenAIProvider {
    async fn handle_error_response(&self, response: reqwest::Response) -> Result<ProviderError> {
        let status = response.status();
        let body: OpenAIErrorResponse = response.json().await?;

        match status.as_u16() {
            401 => Err(ProviderError::InvalidApiKey),
            429 => {
                let retry_after = body.error.retry_after
                    .map(Duration::from_secs);
                Err(ProviderError::RateLimit { retry_after })
            }
            404 => Err(ProviderError::ModelNotFound(body.error.message)),
            500..=599 => Err(ProviderError::ServerError(body.error.message)),
            _ => Err(ProviderError::Unknown(body.error.message)),
        }
    }
}
```

---

## Review Checklist

Before submitting code for review, ensure:

### Code Quality
- [ ] Follows Rust API Guidelines
- [ ] No compiler warnings
- [ ] Runs `cargo clippy` with no warnings
- [ ] Formatted with `cargo fmt`
- [ ] No `unwrap()` or `panic!()` in library code
- [ ] Error types implement `thiserror::Error`

### Type Safety
- [ ] Public APIs use strong types (not stringly-typed)
- [ ] Newtype pattern for validated values
- [ ] Builder pattern for complex construction
- [ ] Derive `Debug` for all non-FFI types

### Testing
- [ ] Unit tests for all public functions
- [ ] Integration tests for critical paths
- [ ] Property-based tests for parsers
- [ ] FFI contract tests for C interface
- [ ] Benchmarks for hot paths

### Documentation
- [ ] Public APIs have doc comments
- [ ] Examples in doc comments compile
- [ ] Module-level documentation exists
- [ ] Complex algorithms explained
- [ ] Non-obvious behavior documented

### Performance
- [ ] No allocations in hot paths
- [ ] Async for all I/O operations
- [ ] Bounded channels for backpressure
- [ ] Pre-allocation when size known

### Security
- [ ] API keys never logged
- [ ] Input validation on all user data
- [ ] No injection vulnerabilities
- [ ] Rate limiting implemented
- [ ] Sensitive data redacted in errors

### FFI Safety
- [ ] Opaque pointers used
- [ ] Every `*_new()` has `*_free()`
- [ ] No panics (use `catch_unwind`)
- [ ] Thread-local error storage
- [ ] Clear ownership rules

### Dependencies
- [ ] Minimal dependencies
- [ ] No unnecessary features enabled
- [ ] Versions pinned in `Cargo.lock`
- [ ] No duplicate dependencies

---

## Conclusion

These guidelines ensure **SimpleAgents** is built to production-grade standards, combining:

1. **Rust Best Practices**: Type safety, ownership, async patterns
2. **BAML-Inspired Healing**: Transparent coercion with confidence scoring
3. **Production Reliability**: Retry, fallback, caching, monitoring
4. **Cross-Language Support**: Safe FFI with clear ownership rules

**Key Takeaway**: Every decision prioritizes **transparency, type safety, and performance** while maintaining **simplicity and developer experience**.

---

**Next Steps**:
1. Review and approve these guidelines
2. Begin Week 1-2: Foundation (types, config, HTTP client)
3. Use this document as reference throughout implementation
4. Update guidelines as patterns emerge

**Questions?** Raise them early - better to clarify now than refactor later.
