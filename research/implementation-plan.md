# SimpleAgents: Rust LLM Gateway Implementation Plan

## Overview

**SimpleAgents** is a high-performance Rust-based LLM gateway that combines:
- **LiteLLM's** multi-provider abstraction, routing, and reliability features
- **BAML's** flexible JSON parsing and response healing capabilities
- Simple API with FFI bindings for Python, Go, TypeScript, JavaScript

**Repository**: `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/SimpleAgents`

---

## Architecture Summary

### Workspace Structure

```
simple-agents/
├── crates/
│   ├── simple-agents-core/         # Main API, client, completion
│   ├── simple-agents-providers/    # Provider implementations (OpenAI, Anthropic, etc.)
│   ├── simple-agents-healing/      # JSON parser, coercion engine, scoring
│   ├── simple-agents-router/       # Routing strategies, retry, fallback
│   ├── simple-agents-cache/        # Cache trait + implementations
│   ├── simple-agents-types/        # Shared types, traits, errors
│   ├── simple-agents-ffi/          # C-compatible FFI layer
│   ├── simple-agents-macros/       # Derive macros for Schema
│   └── simple-agents-cli/          # CLI tool
├── bindings/
│   ├── python/                     # PyO3 Python bindings
│   ├── node/                       # napi-rs TypeScript/JS bindings
│   └── go/                         # cgo Go bindings
└── research/                       # Research documentation
```

---

## Core Components

### 1. Provider Abstraction (`simple-agents-providers`)

**Pattern**: Async trait with transformation methods

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest>;
    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse>;
    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse>;
    fn retry_config(&self) -> RetryConfig;
    fn cost_config(&self) -> CostConfig;
    fn capabilities(&self) -> Capabilities;
}
```

**Providers for MVP**:
- OpenAI (gpt-4, gpt-3.5-turbo)
- Anthropic (claude-3-opus, claude-3-sonnet)
- OpenRouter (unified open-source access)

**OpenAI-Compatible Schema**: All requests/responses normalized to OpenAI format for consistency.

**Technology**:
- HTTP client: `reqwest` (async, connection pooling, streaming)
- Runtime: `tokio`

---

### 2. Response Healing System (`simple-agents-healing`)

**The Core Differentiator** - Handles malformed LLM outputs with transparency.

#### JSON-ish Parser

**State Machine Parser** that accepts:
- Markdown-wrapped JSON (```json ... ```)
- Trailing commas
- Single quotes instead of double quotes
- Unquoted keys
- Truncated/incomplete JSON
- Mixed formats

**Three-Phase Approach**:
1. **Strip & Fix**: Remove markdown, fix common issues
2. **Standard Parse**: Try `serde_json` first (fast path)
3. **Lenient Parse**: Character-by-character state machine for malformed input

#### Coercion Engine

**Schema-Aligned Parsing (SAP)**:
```rust
pub struct CoercionEngine {
    schema: Schema,
    config: CoercionConfig,
}

// Process: Parse → Coerce → Score
impl CoercionEngine {
    pub fn coerce(&self, value: JsonValue) -> CoercionResult {
        // - Try exact type match
        // - Fuzzy field matching (case-insensitive, snake/camel)
        // - Type coercion (string "5" → int 5)
        // - Union resolution (try all variants, pick best score)
        // - Apply defaults for missing fields
    }
}
```

#### Flag System

**Transparency**: Every transformation is tracked
```rust
pub enum CoercionFlag {
    StrippedMarkdown,
    FixedTrailingComma,
    FuzzyFieldMatch { expected: String, found: String },
    TypeCoercion { from: String, to: String },
    UsedDefaultValue { field: String },
}

pub struct CoercionResult {
    pub value: TypedValue,
    pub flags: Vec<CoercionFlag>,
    pub confidence: f32,  // 0.0-1.0, based on # of fixes
}
```

#### Streaming Support

**Incremental Parsing**: State machine maintains partial state
- Parse incomplete JSON as it streams
- Extract completed objects/arrays from buffer
- Emit partial results (e.g., first element of array while second is still streaming)

---

### 3. Routing & Reliability (`simple-agents-router`)

#### Retry Logic

**Exponential Backoff with Jitter**:
```rust
pub struct RetryConfig {
    pub max_attempts: u32,               // Default: 3
    pub initial_backoff: Duration,       // Default: 100ms
    pub max_backoff: Duration,           // Default: 10s
    pub backoff_multiplier: f32,         // Default: 2.0
    pub jitter: bool,                    // Default: true (±30%)
    pub retryable_errors: Vec<ErrorType>,
}
```

**Retryable Errors**:
- Timeout
- Rate limit (429)
- Server errors (500-599)
- Network errors

#### Fallback Strategy

**Ordered Provider Chain**:
```rust
pub struct FallbackChain {
    providers: Vec<ProviderConfig>,  // Try in order
}

// Example: [openai-gpt4, anthropic-claude, openai-gpt35]
// If GPT-4 fails → try Claude → try GPT-3.5
```

#### Routing Strategies

**Pluggable via Trait**:
```rust
#[async_trait]
pub trait RoutingStrategy: Send + Sync {
    async fn select_provider(&self, providers: &[ProviderConfig], req: &CompletionRequest)
        -> Result<&ProviderConfig>;
}
```

**MVP Strategy**: Round-robin
**Post-MVP**: Latency-based, cost-based, least-busy

**Configuration** (TOML):
```toml
[router]
strategy = "round-robin"
fallback_enabled = true

[[providers]]
name = "openai-gpt4"
type = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
timeout = "30s"
max_retries = 3
```

---

### 4. Caching (`simple-agents-cache`)

#### Trait Abstraction

```rust
#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
}
```

#### Implementations

**MVP**: `InMemoryCache` (using `DashMap` for concurrency)
**Post-MVP**: `RedisCache` (using `redis-rs`)

#### Cache Key Generation

SHA256 hash of deterministic request fields:
- Messages
- Model
- Temperature
- Max tokens
- (Excludes: stream, user, logprobs)

**Automatic via Interceptor**:
- Check cache before request
- Store response after successful completion
- Skip caching for streaming requests

---

### 5. Type System & Schema (`simple-agents-types` + `simple-agents-macros`)

#### Derive Macro for Schemas

```rust
use simple_agents::prelude::*;

#[derive(Schema, Debug, Serialize, Deserialize)]
pub struct Character {
    #[schema(required)]
    pub name: String,

    #[schema(required, min_length = 1)]
    pub age: u32,

    #[schema(default = vec![])]
    pub abilities: Vec<String>,

    #[schema(validate = "validate_backstory")]
    pub backstory: Option<String>,
}

fn validate_backstory(s: &str) -> Result<(), ValidationError> {
    if s.len() < 10 {
        Err(ValidationError::new("Backstory too short"))
    } else {
        Ok(())
    }
}
```

#### Partial Types for Streaming (MVP)

**Auto-generated partial versions**:
```rust
// Original type
#[derive(Schema)]
pub struct Character {
    pub name: String,
    pub age: u32,
    pub abilities: Vec<String>,
}

// Auto-generated partial type (all fields Optional)
#[derive(Debug)]
pub struct PartialCharacter {
    pub name: Option<String>,
    pub age: Option<u32>,
    pub abilities: Option<Vec<String>>,
}
```

#### Streaming Annotations (MVP)

```rust
#[derive(Schema)]
pub struct StreamedResponse {
    #[schema(stream_not_null)]  // Don't emit until non-null
    pub id: String,

    #[schema(stream_done)]       // Only emit when complete
    pub status: String,

    pub items: Vec<Item>,        // Emit as items arrive
}
```

#### Type-Safe API

```rust
let client = SimpleAgentsClient::builder()
    .provider("openai")
    .api_key(env::var("OPENAI_API_KEY")?)
    .build()?;

// Structured output with automatic healing
let character: Character = client
    .completion()
    .model("gpt-4")
    .messages(vec![
        Message::user("Generate a fantasy character")
    ])
    .response_format::<Character>()
    .healing(HealingConfig::default())
    .send()
    .await?;

println!("{:?}", character);  // Type-safe!
```

#### Streaming with Partial Types (MVP)

```rust
// Streaming structured output
let mut stream = client
    .completion()
    .model("gpt-4")
    .messages(vec![
        Message::user("Generate a fantasy character")
    ])
    .response_format::<Character>()
    .stream()
    .await?;

// Receive partial results as they stream in
while let Some(partial) = stream.next().await {
    let partial: PartialCharacter = partial?;

    // Fields are optional until fully streamed
    if let Some(name) = &partial.name {
        println!("Name: {}", name);
    }

    if let Some(abilities) = &partial.abilities {
        println!("Abilities so far: {:?}", abilities);
    }
}

// Get final complete result
let final_character: Character = stream.finalize()?;
println!("Final: {:?}", final_character);
```

---

### 6. FFI Layer (`simple-agents-ffi`)

#### C-Compatible Interface

**Opaque Pointers + Error Codes**:

```c
// simple_agents.h

typedef struct SAClient SAClient;
typedef struct SARequest SARequest;
typedef struct SAResponse SAResponse;

#define SA_OK 0
#define SA_ERR_INVALID_ARG -1
#define SA_ERR_PROVIDER -2
#define SA_ERR_TIMEOUT -3

// Client lifecycle
SAClient* sa_client_new(const char* provider, const char* api_key);
void sa_client_free(SAClient* client);

// Request building
SARequest* sa_request_new(void);
int sa_request_add_message(SARequest* req, const char* role, const char* content);
int sa_request_set_model(SARequest* req, const char* model);
void sa_request_free(SARequest* req);

// Completion (blocking)
int sa_complete(SAClient* client, const SARequest* req, SAResponse** response_out);

// Response accessors
const char* sa_response_get_content(const SAResponse* resp);
void sa_response_free(SAResponse* resp);

// Error handling
const char* sa_get_last_error(void);
```

#### Memory Management

**Rules**:
1. Rust owns all memory
2. Each `*_new()` must have `*_free()`
3. Returned strings valid until parent object freed
4. Thread-local error storage
5. Async hidden via `tokio::block_on`

---

### 7. Language Bindings

#### Python (`bindings/python`) - PyO3

**Native Extension with Async Support**:

```rust
#[pyclass]
struct SimpleAgentsClient {
    inner: simple_agents_core::SimpleAgentsClient,
}

#[pymethods]
impl SimpleAgentsClient {
    #[new]
    fn new(provider: String, api_key: String) -> PyResult<Self> { /* ... */ }

    fn complete(&self, messages: Vec<(String, String)>, model: String) -> PyResult<String> {
        // Blocking version
    }

    fn complete_async<'py>(&self, py: Python<'py>, messages: Vec<(String, String)>, model: String)
        -> PyResult<&'py PyAny> {
        // Async version (returns coroutine)
    }
}
```

**Python Usage**:
```python
import simple_agents

client = simple_agents.SimpleAgentsClient("openai", api_key)
response = client.complete([("user", "Hello!")], model="gpt-4")

# Async
async def main():
    response = await client.complete_async([("user", "Hello!")], model="gpt-4")
```

**Distribution**: Python wheels (maturin)

#### Go (`bindings/go`) - cgo

Uses C FFI layer with Go wrapper:

```go
package simpleagents

import "C"

type Client struct {
    ptr *C.SAClient
}

func NewClient(provider, apiKey string) (*Client, error) {
    // ... cgo wrapper
}

func (c *Client) Complete(messages []Message, model string) (string, error) {
    // ... cgo calls
}
```

#### TypeScript/JavaScript (`bindings/node`) - napi-rs

**Native Node.js Addon**:

```rust
#[napi]
pub struct SimpleAgentsClient { /* ... */ }

#[napi]
impl SimpleAgentsClient {
    #[napi(constructor)]
    pub fn new(provider: String, api_key: String) -> Result<Self> { /* ... */ }

    #[napi]
    pub async fn complete(&self, messages: Vec<JsMessage>, model: String) -> Result<String> {
        // Native async/await
    }
}
```

**TypeScript Usage**:
```typescript
import { SimpleAgentsClient } from 'simple-agents';

const client = new SimpleAgentsClient('openai', process.env.OPENAI_API_KEY!);
const response = await client.complete([
    { role: 'user', content: 'Hello!' }
], 'gpt-4');
```

---

### 8. CLI Tool (`simple-agents-cli`)

#### Commands

```bash
simple-agents [OPTIONS] <COMMAND>

Commands:
  complete     Send a completion request
  chat         Interactive chat session
  providers    List available providers
  test         Test provider configuration
  config       Manage configuration
  cache        Cache management operations
```

#### Examples

**Single Request**:
```bash
simple-agents complete -m gpt-4 --message "Explain Rust" --output json
```

**Interactive Chat**:
```bash
simple-agents chat -m gpt-4 --system "You are a helpful assistant"
```

**With Streaming**:
```bash
simple-agents complete -m gpt-4 --message "Count to 10" --stream
```

**Technology**:
- `clap` for CLI parsing
- `rustyline` for interactive chat
- `tokio` for async runtime

---

## Configuration System

### Three-Tier Configuration

1. **Defaults** (hardcoded in code)
2. **Config File** (`simple-agents.toml`)
3. **Programmatic** (builder pattern)

### Example Config File

```toml
# simple-agents.toml

[client]
default_timeout = "30s"

[router]
strategy = "round-robin"
fallback_enabled = true

[[providers]]
name = "openai-gpt4"
type = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
timeout = "30s"
max_retries = 3

[cache]
enabled = true
backend = "memory"
ttl = "1h"

[healing]
enabled = true
strict_mode = false
allow_type_coercion = true
min_confidence = 0.7
```

### Builder Pattern

```rust
let client = SimpleAgentsClient::builder()
    .from_config_file("simple-agents.toml")?
    .provider("openai")
    .api_key(env::var("OPENAI_API_KEY")?)
    .timeout(Duration::from_secs(60))
    .healing(HealingConfig { min_confidence: 0.8, ..Default::default() })
    .cache(Arc::new(InMemoryCache::new()))
    .build()?;
```

---

## Key Dependencies

```toml
# Async runtime
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"

# HTTP client
reqwest = { version = "0.11", features = ["json", "stream"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Concurrency
dashmap = "5.5"
parking_lot = "0.12"

# CLI
clap = { version = "4.4", features = ["derive"] }
rustyline = "13.0"

# Language bindings
pyo3 = { version = "0.20", features = ["extension-module"] }
napi = "2.14"
napi-derive = "2.14"

# Cache (optional)
redis = { version = "0.24", features = ["tokio-comp"] }
```

---

## MVP Scope

### Must Have (First Release)

**Core**:
- [x] Provider trait + OpenAI, Anthropic, OpenRouter implementations
- [x] Basic JSON healing (markdown stripping, trailing commas, basic type coercion)
- [x] Retry logic with exponential backoff
- [x] Round-robin routing
- [x] In-memory LRU cache
- [x] Simple completion API (sync + async, streaming)
- [x] Streaming with partial types and annotations
- [x] Configuration via TOML + builder pattern

**Bindings**:
- [x] C FFI layer
- [x] Python bindings (PyO3, sync + async)

**CLI**:
- [x] `complete` command (single request)
- [x] `chat` command (interactive session)

### Post-MVP Enhancements

**Phase 2**:
- Full BAML coercion (fuzzy field matching, union resolution, confidence scoring)
- Redis cache backend
- Latency-based and cost-based routing
- Additional providers (Google Vertex AI, AWS Bedrock, Azure)
- Go and TypeScript/JavaScript bindings
- Advanced schema derive macros with custom validation

**Phase 3**:
- Metrics/observability (Prometheus, OpenTelemetry)
- Circuit breaker pattern
- Function calling support
- Multimodal support (images, audio)
- Web UI for testing

**Phase 4**:
- Plugin system for custom providers
- Proxy mode (HTTP server)
- Kubernetes operator

---

## Implementation Sequence

### Week 1-2: Foundation
1. Setup workspace structure
2. Define core types (`Message`, `CompletionRequest/Response`)
3. Implement error types (`SimpleAgentsError`)
4. Create HTTP client wrapper
5. Build configuration system (TOML loading, builder pattern)

**Critical Files**:
- `crates/simple-agents-types/src/lib.rs`
- `crates/simple-agents-core/src/config.rs`
- `crates/simple-agents-core/src/http.rs`

### Week 3-4: Provider Abstraction
1. Implement `Provider` trait
2. Create OpenAI provider (request/response transformation)
3. Create Anthropic provider
4. Add OpenRouter provider
5. Basic retry logic
6. Integration tests with real APIs

**Critical Files**:
- `crates/simple-agents-providers/src/lib.rs`
- `crates/simple-agents-providers/src/openai.rs`
- `crates/simple-agents-providers/src/anthropic.rs`

### Week 5-6: Response Healing
1. Build JSON-ish parser (state machine)
2. Implement coercion engine (type coercion, fuzzy matching)
3. Add flag system and scoring
4. Create streaming parser with partial value extraction
5. Implement partial types (all fields as `Option<T>`)
6. Add streaming annotations (`@@stream.not_null`, `@@stream.done`)
7. Comprehensive test corpus (malformed JSON examples)

**Critical Files**:
- `crates/simple-agents-healing/src/parser.rs`
- `crates/simple-agents-healing/src/coercion.rs`
- `crates/simple-agents-healing/src/streaming.rs`
- `crates/simple-agents-healing/src/partial_types.rs`

### Week 7: Routing & Reliability
1. Implement round-robin router
2. Add fallback chain
3. Enhance retry logic (backoff, jitter)
4. Create in-memory cache with LRU eviction

**Critical Files**:
- `crates/simple-agents-router/src/router.rs`
- `crates/simple-agents-router/src/retry.rs`
- `crates/simple-agents-cache/src/memory.rs`

### Week 8: Core API
1. Build `SimpleAgentsClient` with builder pattern
2. Implement completion API (sync/async)
3. Add streaming support
4. Pipeline with interceptors (cache, logging, retry)
5. End-to-end tests

**Critical Files**:
- `crates/simple-agents-core/src/client.rs`
- `crates/simple-agents-core/src/completion.rs`

### Week 9-10: FFI & Python Bindings
1. Create C-compatible FFI layer (opaque pointers, error codes)
2. Build Python bindings (PyO3)
3. FFI contract tests
4. Python examples and tests

**Critical Files**:
- `crates/simple-agents-ffi/src/lib.rs`
- `crates/simple-agents-ffi/simple_agents.h`
- `bindings/python/src/lib.rs`

### Week 11: CLI
1. Implement CLI structure (`clap`)
2. Add `complete` command
3. Add `chat` command with `rustyline`
4. Configuration management commands
5. Polish UX (colors, progress)

**Critical Files**:
- `crates/simple-agents-cli/src/main.rs`

### Week 12: Documentation & Release
1. Write comprehensive README
2. Create examples for common use cases
3. API documentation (rustdoc)
4. Setup CI/CD (GitHub Actions: test, lint, build)
5. Prepare release (versioning, changelog, crates.io)

---

## Research Documentation

Store all research in `/Users/rishub/Desktop/projects/enterprise/craftsmanlabs/SimpleAgents/research/`:

### Files to Create

1. **`research/litellm-analysis.md`**
   - Architecture overview
   - Provider implementations
   - Routing strategies
   - Key learnings

2. **`research/baml-analysis.md`**
   - Response healing mechanisms
   - JSON parser implementation
   - Coercion engine design
   - Flag system details

3. **`research/provider-comparison.md`**
   - OpenAI vs Anthropic vs others
   - API differences
   - Error handling patterns
   - Rate limits

4. **`research/ffi-patterns.md`**
   - Memory management strategies
   - Error passing across boundaries
   - Best practices for each language

5. **`research/architecture-decisions.md`**
   - Trade-offs made
   - Technology choices rationale
   - Alternative approaches considered

---

## Testing Strategy

### Unit Tests

**Per Crate**:
- `simple-agents-healing`: Parser, coercion, streaming
- `simple-agents-router`: Retry logic, routing strategies
- `simple-agents-cache`: Cache implementations
- `simple-agents-providers`: Request/response transformation

### Integration Tests

**Real API Calls** (requires API keys):
```rust
#[tokio::test]
async fn test_openai_completion() {
    let api_key = env::var("OPENAI_API_KEY").unwrap();
    let provider = OpenAIProvider::new(api_key);

    let request = CompletionRequest {
        messages: vec![Message::user("Say 'test'")],
        model: "gpt-3.5-turbo".into(),
        max_tokens: Some(10),
        ..Default::default()
    };

    let response = provider.complete(&request).await.unwrap();
    assert!(!response.choices.is_empty());
}
```

### FFI Contract Tests

**C Interface Validation**:
```rust
#[test]
fn test_client_lifecycle() {
    let provider = CString::new("openai").unwrap();
    let api_key = CString::new("test-key").unwrap();

    unsafe {
        let client = sa_client_new(provider.as_ptr(), api_key.as_ptr());
        assert!(!client.is_null());
        sa_client_free(client);
    }
}
```

### Mocking for LLM Calls

**MockProvider for Tests**:
```rust
pub struct MockProvider {
    responses: Vec<CompletionResponse>,
    call_count: AtomicUsize,
}

// Use for testing retry logic, fallbacks, etc.
```

---

## Verification Plan

### End-to-End Testing

1. **Basic Completion**
   ```bash
   cargo run --bin simple-agents -- complete -m gpt-4 --message "Hello" --output json
   ```
   - Verify response structure
   - Check latency
   - Confirm provider called

2. **Healing Test**
   ```rust
   let malformed = r#"```json
   {"name": "test", "age": 25,}
   ```"#;

   let parsed = parser.parse(malformed).unwrap();
   assert_eq!(parsed["name"], "test");
   assert!(parser.flags.stripped_markdown);
   assert!(parser.flags.fixed_trailing_comma);
   ```

3. **Retry Test**
   - Mock provider returns timeout twice, then success
   - Verify 3 calls made
   - Check exponential backoff timing

4. **Fallback Test**
   - Configure fallback chain: [provider-fail, provider-success]
   - Verify fallback triggered
   - Check final response from second provider

5. **Streaming Test**
   ```rust
   let mut stream = client.completion()
       .model("gpt-4")
       .messages(vec![Message::user("Count to 5")])
       .stream()
       .await?;

   while let Some(chunk) = stream.next().await {
       println!("{}", chunk?.choices[0].delta.content);
   }
   ```

6. **Cache Test**
   - Make same request twice
   - Verify second request served from cache
   - Check cache hit metrics

7. **Python Binding Test**
   ```python
   import simple_agents

   client = simple_agents.SimpleAgentsClient("openai", api_key)
   response = client.complete([("user", "Hello")], model="gpt-4")
   assert len(response) > 0
   ```

8. **CLI Interactive Test**
   ```bash
   simple-agents chat -m gpt-4 --system "You are helpful"
   >> Hello
   # Verify response appears
   >> exit
   ```

---

## Success Criteria

### MVP Complete When:

- ✅ Can make completions to OpenAI, Anthropic, OpenRouter
- ✅ Handles malformed JSON responses (at least 90% of common cases)
- ✅ Retry logic works with exponential backoff
- ✅ Fallback chain successfully fails over
- ✅ In-memory cache reduces duplicate calls
- ✅ Python bindings work (sync + async)
- ✅ CLI can complete single requests and run interactive chat
- ✅ All tests pass (unit, integration, FFI)
- ✅ Documentation complete (README, examples, API docs)
- ✅ CI/CD pipeline green

---

## Challenges & Mitigations

### Challenge 1: Provider API Changes

**Risk**: Providers update APIs, breaking integrations

**Mitigation**:
- Version provider implementations
- Comprehensive integration tests (run in CI)
- Monitor provider changelog/announcements
- Graceful degradation when features unsupported

### Challenge 2: Response Healing Over-Aggressiveness

**Risk**: Coercion produces incorrect data

**Mitigation**:
- Confidence scoring (users set threshold)
- Strict mode option (fail on any coercion)
- Flag system for transparency
- Extensive test corpus
- User feedback loop

### Challenge 3: FFI Memory Safety

**Risk**: Memory leaks, use-after-free in bindings

**Mitigation**:
- Clear ownership rules documented
- Opaque pointer pattern (no shared state)
- Comprehensive contract tests
- Valgrind/ASan in CI
- Fuzz testing

### Challenge 4: Streaming Complexity

**Risk**: Backpressure, timeout, partial data handling

**Mitigation**:
- Tokio channels for backpressure
- Per-chunk timeout
- State machine tracks completion
- Extensive streaming tests
- Mock SSE data

---

## Trade-offs

1. **Async Trait vs Manual Futures**
   - **Decision**: Use `async_trait` macro
   - **Rationale**: Readability > minimal overhead for I/O-bound ops

2. **PyO3 vs ctypes for Python**
   - **Decision**: PyO3
   - **Rationale**: Better DX, type safety, async support, wheel distribution

3. **Lenient vs Strict Parsing Default**
   - **Decision**: Lenient default, strict mode optional
   - **Rationale**: Core value prop is handling malformed responses

4. **Single Binary vs Modular**
   - **Decision**: Single binary with feature flags
   - **Rationale**: Simple distribution, Cargo handles features

5. **TOML vs YAML for Config**
   - **Decision**: TOML primary
   - **Rationale**: Rust-native, clear structure, Cargo convention

---

## Critical Files (Priority Order)

1. **`crates/simple-agents-types/src/lib.rs`**
   - Core types, traits, errors
   - Foundation for entire project

2. **`crates/simple-agents-providers/src/openai.rs`**
   - First provider implementation
   - Validates provider abstraction

3. **`crates/simple-agents-healing/src/parser.rs`**
   - JSON-ish parser
   - Core differentiator

4. **`crates/simple-agents-core/src/client.rs`**
   - Main client API
   - User-facing interface

5. **`Cargo.toml`**
   - Workspace setup
   - Dependency management

---

## Next Steps After Approval

1. Create workspace structure
2. Initialize all crates with `Cargo.toml`
3. Define core types and traits
4. Implement OpenAI provider (validates architecture)
5. Build JSON-ish parser (proves healing concept)
6. Iterate based on learnings

**Estimated Timeline**: 12 weeks to MVP

**Success Metric**: Developer can make a single API call with healing in <10 lines of Rust code:

```rust
use simple_agents::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let client = SimpleAgentsClient::builder()
        .provider("openai")
        .api_key(env::var("OPENAI_API_KEY")?)
        .build()?;

    let response = client.completion()
        .model("gpt-4")
        .messages(vec![Message::user("Hello!")])
        .send()
        .await?;

    println!("{}", response.choices[0].message.content);
    Ok(())
}
```
