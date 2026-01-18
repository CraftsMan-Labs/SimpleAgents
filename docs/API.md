# API Reference

Complete API reference for SimpleAgents.

## Table of Contents

- [simple-agents-types](#simple-agents-types)
  - [Request Types](#request-types)
  - [Response Types](#response-types)
  - [Message Types](#message-types)
  - [Provider Trait](#provider-trait)
  - [Cache Trait](#cache-trait)
  - [Error Types](#error-types)
  - [Validation Types](#validation-types)
- [simple-agents-providers](#simple-agents-providers)
  - [OpenAI Provider](#openai-provider)
  - [Retry Module](#retry-module)
- [simple-agents-cache](#simple-agents-cache)

## simple-agents-types

### Request Types

#### `CompletionRequest`

The main request type for LLM completions.

```rust
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stream: Option<bool>,
    pub n: Option<u32>,
    pub stop: Option<Vec<String>>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub user: Option<String>,
}
```

**Builder Methods:**

```rust
impl CompletionRequest {
    pub fn builder() -> CompletionRequestBuilder;
}

impl CompletionRequestBuilder {
    pub fn model(self, model: impl Into<String>) -> Self;
    pub fn message(self, message: Message) -> Self;
    pub fn messages(self, messages: Vec<Message>) -> Self;
    pub fn max_tokens(self, max_tokens: u32) -> Self;
    pub fn temperature(self, temperature: f32) -> Self;
    pub fn top_p(self, top_p: f32) -> Self;
    pub fn stream(self, stream: bool) -> Self;
    pub fn n(self, n: u32) -> Self;
    pub fn stop(self, stop: Vec<String>) -> Self;
    pub fn presence_penalty(self, penalty: f32) -> Self;
    pub fn frequency_penalty(self, penalty: f32) -> Self;
    pub fn user(self, user: impl Into<String>) -> Self;
    pub fn build(self) -> Result<CompletionRequest>;
}
```

**Validation:**

```rust
impl CompletionRequest {
    pub fn validate(&self) -> Result<()>;
}
```

Checks:
- Messages: 1-1000 items, each < 1MB
- Total size: < 10MB
- Model: alphanumeric + `-_./` only
- Temperature: 0.0-2.0
- Top_p: 0.0-1.0
- Penalties: -2.0 to 2.0

### Response Types

#### `CompletionResponse`

The response from an LLM completion.

```rust
pub struct CompletionResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<CompletionChoice>,
    pub usage: Usage,
    pub created: Option<i64>,
    pub provider: Option<String>,
}

impl CompletionResponse {
    pub fn content(&self) -> Option<&str>;
    pub fn first_choice(&self) -> Option<&CompletionChoice>;
}
```

#### `CompletionChoice`

A single completion option.

```rust
pub struct CompletionChoice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: FinishReason,
    pub logprobs: Option<serde_json::Value>,
}
```

#### `FinishReason`

Why the completion stopped.

```rust
pub enum FinishReason {
    Stop,           // Natural stop point
    Length,         // Max tokens reached
    ContentFilter,  // Filtered by provider
    ToolCalls,      // Function/tool calls generated
}
```

#### `Usage`

Token usage statistics.

```rust
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl Usage {
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self;
}
```

#### Streaming Types

```rust
pub struct CompletionChunk {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChoiceDelta>,
    pub created: Option<i64>,
}

pub struct ChoiceDelta {
    pub index: u32,
    pub delta: MessageDelta,
    pub finish_reason: Option<FinishReason>,
}

pub struct MessageDelta {
    pub role: Option<Role>,
    pub content: Option<String>,
}
```

### Message Types

#### `Message`

A message in a conversation.

```rust
pub struct Message {
    pub role: Role,
    pub content: String,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
}
```

**Constructors:**

```rust
impl Message {
    pub fn user(content: impl Into<String>) -> Self;
    pub fn assistant(content: impl Into<String>) -> Self;
    pub fn system(content: impl Into<String>) -> Self;
    pub fn tool(content: impl Into<String>, tool_call_id: Option<String>) -> Self;
    pub fn with_name(self, name: impl Into<String>) -> Self;
}
```

#### `Role`

The role of a message sender.

```rust
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}
```

### Provider Trait

The core abstraction for LLM providers.

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    fn transform_request(&self, req: &CompletionRequest)
        -> Result<ProviderRequest>;

    async fn execute(&self, req: ProviderRequest)
        -> Result<ProviderResponse>;

    fn transform_response(&self, resp: ProviderResponse)
        -> Result<CompletionResponse>;

    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    async fn execute_stream(&self, req: ProviderRequest)
        -> Result<Box<dyn Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        Err(SimpleAgentsError::Provider(
            ProviderError::UnsupportedFeature("streaming".to_string())
        ))
    }
}
```

#### `ProviderRequest`

HTTP request details for providers.

```rust
pub struct ProviderRequest {
    pub url: String,
    pub headers: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    pub body: serde_json::Value,
    pub timeout: Option<Duration>,
}

impl ProviderRequest {
    pub fn new(url: impl Into<String>) -> Self;
    pub fn with_header(self, name: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn with_static_header(self, name: &'static str, value: &'static str) -> Self;
    pub fn with_body(self, body: serde_json::Value) -> Self;
    pub fn with_timeout(self, timeout: Duration) -> Self;
}
```

**Static Headers:**

```rust
pub mod headers {
    pub const AUTHORIZATION: &str = "Authorization";
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const X_API_KEY: &str = "x-api-key";
}
```

#### `ProviderResponse`

HTTP response from providers.

```rust
pub struct ProviderResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub headers: Option<Vec<(String, String)>>,
}

impl ProviderResponse {
    pub fn new(status: u16, body: serde_json::Value) -> Self;
    pub fn is_success(&self) -> bool;
    pub fn is_client_error(&self) -> bool;
    pub fn is_server_error(&self) -> bool;
    pub fn with_headers(self, headers: Vec<(String, String)>) -> Self;
}
```

### Cache Trait

Async caching interface.

```rust
#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn clear(&self) -> Result<()>;

    fn is_enabled(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "cache"
    }
}
```

#### `CacheKey`

Helper for generating cache keys.

```rust
pub struct CacheKey;

impl CacheKey {
    pub fn from_parts(provider: &str, model: &str, content: &str) -> String;
    pub fn with_namespace(namespace: &str, key: &str) -> String;
}
```

### Error Types

#### `SimpleAgentsError`

The main error type.

```rust
pub enum SimpleAgentsError {
    Validation(ValidationError),
    Provider(ProviderError),
    Network(String),
    Serialization(String),
    Cache(String),
    Config(String),
}
```

#### `ValidationError`

Input validation errors.

```rust
pub enum ValidationError {
    Empty { field: String },
    TooShort { field: String, min: usize },
    TooLong { field: String, max: usize },
    OutOfRange { field: String, min: f64, max: f64 },
    InvalidFormat { field: String, reason: String },
}
```

#### `ProviderError`

Provider-specific errors.

```rust
pub enum ProviderError {
    Authentication(String),
    RateLimit {
        retry_after: Option<Duration>,
        message: String,
    },
    InvalidResponse(String),
    ModelNotFound(String),
    ContextLengthExceeded { max_tokens: u32 },
    Timeout(Duration),
    ContentFiltered { reason: String },
    UnsupportedFeature(String),
}

impl ProviderError {
    pub fn is_retryable(&self) -> bool;
}
```

### Validation Types

#### `ApiKey`

Secure API key type.

```rust
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Result<Self>;
    pub fn expose(&self) -> &str;
    pub fn preview(&self) -> String;
}

// Never logged in Debug
impl Debug for ApiKey { ... }

// Never serialized in plain text
impl Serialize for ApiKey { ... }

// Constant-time comparison
impl PartialEq for ApiKey { ... }
```

Validation rules:
- Not empty
- At least 20 characters
- No null bytes

### Configuration Types

#### `RetryConfig`

Configuration for retry logic.

```rust
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub backoff_multiplier: f32,
    pub jitter: bool,
}

impl RetryConfig {
    pub fn calculate_backoff(&self, attempt: u32) -> Duration;
}

impl Default for RetryConfig {
    // max_attempts: 3
    // initial_backoff: 100ms
    // max_backoff: 10s
    // backoff_multiplier: 2.0
    // jitter: true
}
```

#### `Capabilities`

Provider capabilities.

```rust
pub struct Capabilities {
    pub streaming: bool,
    pub function_calling: bool,
    pub vision: bool,
    pub max_tokens: u32,
}
```

## simple-agents-providers

### OpenAI Provider

```rust
pub struct OpenAIProvider {
    // Private fields
}

impl OpenAIProvider {
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com/v1";

    pub fn new(api_key: ApiKey) -> Result<Self>;
    pub fn with_base_url(api_key: ApiKey, base_url: String) -> Result<Self>;
    pub fn base_url(&self) -> &str;
}

impl Provider for OpenAIProvider { ... }
```

### Retry Module

```rust
pub async fn execute_with_retry<F, Fut, T>(
    config: &RetryConfig,
    error_is_retryable: impl Fn(&SimpleAgentsError) -> bool,
    operation: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
```

## simple-agents-cache

### InMemoryCache

```rust
pub struct InMemoryCache { ... }

impl InMemoryCache {
    pub fn new(max_size: usize, max_entries: usize) -> Self;
}

impl Cache for InMemoryCache { ... }
```

**Features:**
- LRU eviction
- TTL-based expiry
- Thread-safe (Arc<RwLock<>>)
- Configurable size and entry limits

### NoOpCache

```rust
pub struct NoOpCache;

impl Default for NoOpCache { ... }
impl Cache for NoOpCache { ... }
```

**Features:**
- Always returns `None` on `get`
- `set`, `delete`, `clear` do nothing
- `is_enabled()` returns `false`
- Useful for testing and disabling cache

## Prelude

Import commonly used types:

```rust
use simple_agents_types::prelude::*;

// Includes:
// - CompletionRequest, CompletionResponse
// - Message, Role
// - Provider, ProviderRequest, ProviderResponse
// - Cache, CacheKey
// - SimpleAgentsError, Result
// - ApiKey
```

## Type Aliases

```rust
pub type Result<T> = std::result::Result<T, SimpleAgentsError>;
```
