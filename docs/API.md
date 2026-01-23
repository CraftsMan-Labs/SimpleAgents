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
   - [Anthropic Provider](#anthropic-provider)
   - [OpenRouter Provider](#openrouter-provider)
   - [Streaming Support](#streaming-support)
   - [Retry Module](#retry-module)
   - [Metrics Module](#metrics-module)
   - [Rate Limiting](#rate-limiting)
 - [simple-agents-cache](#simple-agents-cache)
 - [simple-agents-healing](#simple-agents-healing)
   - [Parser Types](#parser-types)
   - [Coercion Engine](#coercion-engine)
   - [Streaming Parser](#streaming-parser)
   - [Schema Types](#schema-types)
 - [simple-agents-macros](#simple-agents-macros)
   - [PartialType Macro](#partialtype-macro)

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

### Anthropic Provider

```rust
pub struct AnthropicProvider {
    // Private fields
}

impl AnthropicProvider {
    pub const DEFAULT_BASE_URL: &'static str = "https://api.anthropic.com/v1";

    pub fn new(api_key: ApiKey) -> Result<Self>;
    pub fn with_base_url(api_key: ApiKey, base_url: String) -> Result<Self>;
    pub fn base_url(&self) -> &str;
}

impl Provider for AnthropicProvider { ... }
```

### OpenRouter Provider

```rust
pub struct OpenRouterProvider {
    // Private fields
}

impl OpenRouterProvider {
    pub const DEFAULT_BASE_URL: &'static str = "https://openrouter.ai/api/v1";

    pub fn new(api_key: ApiKey) -> Result<Self>;
    pub fn with_base_url(api_key: ApiKey, base_url: String) -> Result<Self>;
    pub fn base_url(&self) -> &str;
}

impl Provider for OpenRouterProvider { ... }
```

**Supported Models:** Uses provider prefixes like `openai/gpt-4`, `anthropic/claude-3-opus`, `meta-llama/llama-2-70b-chat`.

### Streaming Support

```rust
impl Provider {
    async fn execute_stream(&self, req: ProviderRequest)
        -> Result<Box<dyn Stream<Item = Result<CompletionChunk>> + Send + Unpin>>;
}
```

**Streamed Types:**

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

### Metrics Module

```rust
pub use metrics::RequestTimer;

impl RequestTimer {
    pub fn start(provider: impl Into<String>, model: impl Into<String>) -> Self;
    pub fn complete_success(self, prompt_tokens: u32, completion_tokens: u32);
    pub fn complete_error(self, error_type: impl Into<String>);
    pub fn complete_timeout(self);
}

pub mod names {
    pub const REQUESTS_TOTAL: &str = "simple_agents_requests_total";
    pub const REQUEST_DURATION: &str = "simple_agents_request_duration_seconds";
    pub const TOKENS_TOTAL: &str = "simple_agents_tokens_total";
    pub const RETRIES_TOTAL: &str = "simple_agents_retries_total";
    pub const RETRY_BACKOFF: &str = "simple_agents_retry_backoff_seconds";
}

pub mod labels {
    pub const PROVIDER: &str = "provider";
    pub const MODEL: &str = "model";
    pub const STATUS: &str = "status";
    pub const TOKEN_TYPE: &str = "type";
    pub const ERROR_TYPE: &str = "error_type";
}

pub fn record_retry(provider: impl Into<String>, backoff_seconds: f64);
```

### Rate Limiting

```rust
pub use simple_agents_providers::rate_limit::{RateLimiter, TokenBucket, RateLimitConfig};

impl TokenBucket {
    pub fn new(capacity: u64, refill_rate: u64) -> Self;
    pub async fn try_acquire(&self, tokens: u64) -> Result<()>;
    pub fn try_acquire_blocking(&self, tokens: u64) -> Result<()>;
}

impl RateLimiter {
    pub fn new(bucket: TokenBucket) -> Self;
    pub async fn check(&self) -> Result<()>;
}

pub struct RateLimitConfig {
    pub max_requests_per_minute: u32,
    pub max_tokens_per_minute: u32,
}
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

## simple-agents-healing

### Parser Types

```rust
pub struct JsonishParser;

impl JsonishParser {
    pub fn new() -> Self;
    pub fn with_config(config: ParserConfig) -> Self;
    pub fn parse(&self, input: &str) -> Result<CoercionResult<serde_json::Value>>;
}

pub struct ParserConfig {
    pub strip_markdown: bool,
    pub fix_trailing_commas: bool,
    pub fix_quotes: bool,
    pub fix_unquoted_keys: bool,
    pub fix_control_chars: bool,
    pub remove_bom: bool,
    pub min_confidence: f32,
}

impl Default for ParserConfig { ... }

pub struct ParserResult<T> {
    pub value: T,
    pub confidence: f32,
    pub flags: Vec<CoercionFlag>,
}
```

### Coercion Engine

```rust
pub struct CoercionEngine;

impl CoercionEngine {
    pub fn new() -> Self;
    pub fn with_config(config: CoercionConfig) -> Self;
    pub fn coerce(
        &self,
        input: &serde_json::Value,
        schema: &Schema
    ) -> Result<CoercionResult<serde_json::Value>>;
}

pub struct CoercionConfig {
    pub enable_string_number_coercion: bool,
    pub enable_fuzzy_field_matching: bool,
    pub min_confidence: f32,
}

impl Default for CoercionConfig { ... }
```

### Streaming Parser

```rust
pub struct StreamingParser;

impl StreamingParser {
    pub fn new() -> Self;
    pub fn parse_partial(&self, input: &str) -> Result<serde_json::Value>;
    pub fn extract_partial(&self, input: &str, schema: &Schema) -> Result<PartialValue>;
}

pub struct PartialExtractor;

impl PartialExtractor {
    pub fn new(schema: Schema) -> Self;
    pub fn extract(&self, input: &str) -> Result<PartialValue>;
}

pub struct PartialValue {
    pub fields: HashMap<String, serde_json::Value>,
    pub complete: bool,
}
```

### Schema Types

```rust
pub enum Schema {
    String,
    Int,
    Float,
    Bool,
    Array(Box<Schema>),
    Object(ObjectSchema),
    Union(Vec<Schema>),
    Optional(Box<Schema>),
}

pub struct ObjectSchema {
    pub fields: Vec<Field>,
    pub allow_additional_fields: bool,
}

pub struct Field {
    pub name: String,
    pub schema: Schema,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
}

impl Field {
    pub fn required(name: impl Into<String>, schema: Schema) -> Self;
    pub fn optional(name: impl Into<String>, schema: Schema) -> Self;
    pub fn with_default(self, default: serde_json::Value) -> Self;
}

pub enum StreamAnnotation {
    NotNull,
    Done,
    Eager,
}
```

### Coercion Flags

```rust
pub enum CoercionFlag {
    StrippedMarkdown,
    FixedTrailingComma,
    FixedQuotes,
    FixedUnquotedKeys,
    FixedControlCharacters,
    RemovedBom,
    TruncatedJson,
    StringToNumberCoerced,
    FuzzyFieldMatch { expected: String, found: String },
    UsedDefaultValue { field: String },
    UnionTypeSelected { selected: String },
}

impl CoercionFlag {
    pub fn description(&self) -> &'static str;
    pub fn confidence_penalty(&self) -> f32;
}
```

## simple-agents-macros

### PartialType Macro

```rust
#[proc_macro_derive(PartialType, attributes(partial))]
pub fn derive_partial_type(input: TokenStream) -> TokenStream;
```

**Attributes:**

- `#[partial(skip)]` - Exclude field from partial type
- `#[partial(default)]` - Use default value if missing

**Generated Code:**

```rust
// Original struct:
#[derive(PartialType)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

// Generates:
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartialUser {
    pub id: Option<u64>,
    pub name: Option<String>,
    pub email: Option<String>,
}

impl User {
    pub fn from_partial(partial: PartialUser) -> Result<Self, String> { ... }
}

impl PartialUser {
    pub fn merge(&mut self, other: PartialUser) { ... }
}
```

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
