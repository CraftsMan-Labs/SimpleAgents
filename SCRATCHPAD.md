# SimpleAgents Development Scratchpad

> Developer notes, implementation details, and future considerations
>
> **Last Updated**: 2026-01-16
> **Current Phase**: Foundation Complete, Planning Phase 2

---

## 📝 Phase 1 Implementation Notes

### What I Built (Week 1-2)

#### ✅ Core Type System (`simple-agents-types`)

**Key Decisions Made**:

1. **Pure types crate - NO runtime dependencies**
   - Only serde, thiserror, async-trait
   - No tokio, no reqwest, no HTTP client
   - Rationale: Keep foundation reusable and lightweight
   - Future benefit: Can be used in WASM, embedded, etc.

2. **Builder pattern everywhere**
   - `CompletionRequest::builder()` - fluent API
   - `ProviderConfig::new().with_api_key()` - chainable
   - Rationale: Ergonomic + compile-time validation
   - Works well: Users get clear error messages

3. **Trait-based architecture**
   ```rust
   Provider trait  -> Implement for OpenAI, Anthropic, etc.
   Cache trait     -> Implement for Redis, in-memory, etc.
   RoutingStrategy -> Implement for different algorithms
   ```
   - Rationale: Maximum extensibility
   - Verified: All traits are object-safe
   - Future: Can add providers without breaking changes

4. **Security-first for API keys**
   - Never logged (Debug shows `[REDACTED]`)
   - Never serialized (JSON shows `[REDACTED]`)
   - Only `expose()` gives raw key
   - Rationale: Production security from day 1
   - Works perfectly: Passed all security tests

5. **Transparency through CoercionFlag**
   - Every transformation tracked
   - Confidence scoring (0.0-1.0)
   - Major vs minor categorization
   - Rationale: Users must know what changed
   - Future: Will be critical for JSON healing

#### 🔧 Technical Gotchas Encountered

1. **Thiserror `source` field name conflict**
   - Problem: Can't use field named `source` in error structs
   - Solution: Renamed to `error_message` in `HealingError::ParseFailed`
   - Lesson: Read thiserror docs carefully!

2. **Duration serialization**
   - Problem: Duration doesn't implement Serialize by default
   - Solution: Custom `duration_millis` module with serde helpers
   - Location: `config.rs:218-231`

3. **Float equality in tests**
   - Problem: `0.95 != 0.950000012` due to float precision
   - Solution: Use `abs() < 0.001` for comparisons
   - Location: `router.rs:220-221`

4. **Clippy warnings**
   - `manual_hash_one`: Use `hash_one()` instead of manual hasher
   - `needless_borrows_for_generic_args`: Remove unnecessary `&`
   - All fixed in final pass

5. **Test string lengths**
   - Problem: Counted wrong length for test API key
   - Solution: Always verify with `python3 -c "print(len('string'))"`
   - Lesson: Don't assume, measure!

#### 📊 Code Organization Patterns

**Module structure that works well**:
```rust
// Each module:
// 1. Types and structs
// 2. Implementations
// 3. Helper functions
// 4. Tests at bottom with #[cfg(test)]

pub struct MyType { ... }

impl MyType {
    pub fn new() -> Self { ... }
    pub fn helper() -> Result<()> { ... }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thing() { ... }
}
```

**What to test**:
- ✅ Happy path
- ✅ Validation failures
- ✅ Serialization round-trips
- ✅ Edge cases (empty, max, boundaries)
- ✅ Security (API key redaction)
- ✅ Object safety for traits

#### 🎨 Design Patterns Established

1. **Builder Pattern**
   ```rust
   Request::builder()
       .field(value)
       .field2(value2)
       .build()?  // Validates here
   ```
   - Validation happens in `build()`, not during field setting
   - Returns `Result` for clear error handling

2. **Newtype Pattern for Security**
   ```rust
   pub struct ApiKey(String);  // Never public
   impl ApiKey {
       pub fn expose(&self) -> &str  // Explicit access
   }
   ```

3. **Opaque Types for Provider Flexibility**
   ```rust
   pub struct ProviderRequest { url, headers, body }
   // Provider can transform however it needs
   ```

4. **Error Hierarchy**
   ```rust
   SimpleAgentsError        // Top-level
   ├── Provider(ProviderError)
   ├── Healing(HealingError)
   ├── Validation(ValidationError)
   └── ...
   ```
   - Auto-conversions with `From` trait
   - Specific error types for specific domains

---

## 🚀 Phase 2 Planning: Providers (`simple-agents-providers`)

### Goals
- Implement OpenAI provider
- Implement Anthropic provider
- Real HTTP client integration
- Streaming support
- Error mapping from provider APIs

### Technical Considerations

#### HTTP Client Choice
**Decision**: Use `reqwest`
- Pros:
  - Industry standard
  - Excellent async support
  - Built-in JSON support
  - Streaming support
  - Good error messages
- Cons:
  - Heavy dependency (but needed anyway)
- Alternative considered: `ureq` (sync), rejected because we need async

#### Provider Architecture
```rust
pub struct OpenAIProvider {
    api_key: ApiKey,
    base_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn transform_request(&self, req: &CompletionRequest)
        -> Result<ProviderRequest>
    {
        // 1. Map our Message format to OpenAI format
        // 2. Handle model name mapping
        // 3. Add API version headers
        // 4. Create ProviderRequest
    }

    async fn execute(&self, req: ProviderRequest)
        -> Result<ProviderResponse>
    {
        // 1. Make actual HTTP request with reqwest
        // 2. Handle timeouts
        // 3. Handle rate limits (429)
        // 4. Return ProviderResponse
    }

    fn transform_response(&self, resp: ProviderResponse)
        -> Result<CompletionResponse>
    {
        // 1. Parse OpenAI JSON
        // 2. Map to our CompletionResponse
        // 3. Handle errors
    }
}
```

#### OpenAI-Specific Considerations

**Rate Limiting**:
- Header: `x-ratelimit-remaining-requests`
- Header: `x-ratelimit-reset-requests`
- Error code: 429
- Action: Parse `retry-after` header, return `ProviderError::RateLimit`

**Error Codes**:
```rust
match status {
    401 => ProviderError::InvalidApiKey,
    404 => ProviderError::ModelNotFound(model),
    429 => ProviderError::RateLimit { retry_after },
    500..=599 => ProviderError::ServerError(body),
    _ => ProviderError::BadRequest(body),
}
```

**Streaming**:
- OpenAI uses Server-Sent Events (SSE)
- Each chunk: `data: {"choices": [{"delta": ...}]}`
- Need to parse line by line
- Return `CompletionChunk` instead of `CompletionResponse`

**Model Mapping**:
```rust
// User requests: "gpt-4"
// OpenAI expects: "gpt-4-0613" or latest
// Need model alias resolution
const MODEL_ALIASES: &[(&str, &str)] = &[
    ("gpt-4", "gpt-4-0613"),
    ("gpt-3.5-turbo", "gpt-3.5-turbo-0613"),
];
```

#### Anthropic-Specific Considerations

**Different Message Format**:
- Anthropic requires `system` separate from `messages`
- Our format: `[Message{role: System, ...}, Message{role: User, ...}]`
- Their format: `{system: "...", messages: [{role: "user", ...}]}`
- Need to extract system messages in transform

**Model Names**:
- `claude-3-opus-20240229`
- `claude-3-sonnet-20240229`
- `claude-3-haiku-20240307`

**Streaming**:
- Similar SSE format but different schema
- Events: `message_start`, `content_block_delta`, `message_stop`

**Headers**:
```rust
headers.insert("anthropic-version", "2023-06-01");
headers.insert("anthropic-beta", "messages-2023-12-15");  // If using beta features
```

#### Testing Strategy

**Mock Tests** (always run):
```rust
#[tokio::test]
async fn test_openai_request_transformation() {
    let provider = OpenAIProvider::new_mock();
    let request = CompletionRequest::builder()...;
    let provider_req = provider.transform_request(&request)?;
    // Assert correct format
}
```

**Integration Tests** (opt-in with env var):
```rust
#[tokio::test]
#[ignore] // Don't run by default
async fn test_openai_real_api() {
    let api_key = env::var("OPENAI_API_KEY").ok()?;
    let provider = OpenAIProvider::new(api_key);
    // Make real request
}
```

**Mock Server** (for CI):
- Use `wiremock` or `mockito`
- Simulate rate limits, timeouts, errors
- Verify retry behavior

#### Dependencies for Phase 2
```toml
[dependencies]
simple-agents-types = { path = "../simple-agents-types" }
reqwest = { version = "0.11", features = ["json", "stream"] }
tokio = { version = "1.35", features = ["full"] }
futures = "0.3"  # For stream processing
serde_json = "1.0"

[dev-dependencies]
wiremock = "0.5"  # Mock HTTP server
tokio-test = "0.4"
```

#### File Structure
```
crates/simple-agents-providers/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── openai/
│   │   ├── mod.rs       # Provider impl
│   │   ├── models.rs    # OpenAI-specific types
│   │   ├── streaming.rs # SSE parsing
│   │   └── error.rs     # Error mapping
│   ├── anthropic/
│   │   ├── mod.rs
│   │   ├── models.rs
│   │   ├── streaming.rs
│   │   └── error.rs
│   └── utils.rs         # Shared utilities
├── tests/
│   ├── openai_integration.rs
│   └── anthropic_integration.rs
└── examples/
    ├── openai_basic.rs
    └── anthropic_basic.rs
```

#### Implementation Order
1. ✅ Set up crate structure
2. ✅ OpenAI non-streaming implementation
3. ✅ OpenAI error mapping
4. ✅ OpenAI streaming
5. ✅ Anthropic non-streaming
6. ✅ Anthropic error mapping
7. ✅ Anthropic streaming
8. ✅ Integration tests
9. ✅ Examples
10. ✅ Documentation

---

## 🔮 Phase 3 Planning: Healing (`simple-agents-healing`)

### Goals
- Parse malformed JSON from LLMs
- Fix common issues automatically
- Track all fixes via `CoercionFlag`
- Score confidence

### Real-World Issues to Handle

**From actual LLM outputs**:

1. **Markdown code fences**:
   ```
   ```json
   {"key": "value"}
   ```
   ```
   → Strip the ` ```json` and ` ``` ` parts

2. **Trailing commas**:
   ```json
   {"key": "value",}
   ```
   → Remove trailing comma before `}`

3. **Unquoted keys**:
   ```json
   {key: "value"}
   ```
   → Add quotes: `{"key": "value"}`

4. **Mismatched quotes**:
   ```json
   {"key': "value"}
   ```
   → Fix to: `{"key": "value"}`

5. **Truncated JSON**:
   ```json
   {"key": "val
   ```
   → Try to recover or error with context

6. **Type coercion**:
   ```json
   {"age": "25"}  // string
   // Expected: {"age": 25}  // number
   ```
   → Coerce "25" → 25, flag it

7. **Fuzzy field names**:
   ```json
   {"user_name": "Alice"}  // LLM output
   // Expected: {"username": "Alice"}
   ```
   → Match with Levenshtein distance

### Healing Strategy

```rust
pub struct JsonHealer {
    config: HealingConfig,
    strategies: Vec<Box<dyn HealingStrategy>>,
}

impl JsonHealer {
    pub fn heal<T: DeserializeOwned>(&self, input: &str)
        -> Result<CoercionResult<T>>
    {
        let mut flags = Vec::new();
        let mut confidence = 1.0;

        // 1. Strip markdown
        let (cleaned, flag) = self.strip_markdown(input);
        if let Some(f) = flag {
            flags.push(f);
            confidence *= 0.98;
        }

        // 2. Fix trailing commas
        let (cleaned, flag) = self.fix_commas(&cleaned);
        if let Some(f) = flag {
            flags.push(f);
            confidence *= 0.95;
        }

        // 3. Try parse
        match serde_json::from_str::<T>(&cleaned) {
            Ok(value) => {
                Ok(CoercionResult { value, flags, confidence })
            }
            Err(e) => {
                // Try more aggressive healing
                self.aggressive_heal(&cleaned, flags, confidence)
            }
        }
    }
}
```

### Confidence Scoring Rules

```rust
Base confidence: 1.0

Deductions:
- Stripped markdown:        -0.02
- Fixed trailing comma:     -0.05
- Fixed quotes:             -0.10
- Type coercion:            -0.15
- Fuzzy field match:        -0.20
- Truncated JSON:           -0.30
- Multiple major fixes:     -0.40

Minimum: 0.0
```

### Dependencies
```toml
[dependencies]
simple-agents-types = { path = "../simple-agents-types" }
serde = "1.0"
serde_json = "1.0"
regex = "1.0"
levenshtein = "1.0"  # For fuzzy matching
```

### Testing Strategy

**Real LLM outputs** (collect from GPT-4, Claude):
```rust
#[test]
fn test_gpt4_markdown_wrapped() {
    let input = r#"```json
    {"result": "success"}
    ```"#;

    let healed: CoercionResult<MyType> = healer.heal(input)?;
    assert!(healed.was_coerced());
    assert!(healed.flags.contains(&CoercionFlag::StrippedMarkdown));
}
```

**Confidence threshold tests**:
```rust
#[test]
fn test_strict_mode_rejects_low_confidence() {
    let healer = JsonHealer::new(HealingConfig::strict());
    let result = healer.heal::<MyType>(malformed_input);

    // Strict mode: min_confidence = 0.95
    // If confidence < 0.95, should error
    assert!(result.is_err());
}
```

---

## 🔄 Phase 4 Planning: Router (`simple-agents-router`)

### Goals
- Implement routing strategies
- Exponential backoff retry
- Provider fallback chains
- Circuit breaker

### Routing Strategies to Implement

#### 1. Priority Routing (Simplest)
```rust
pub struct PriorityRouter;

#[async_trait]
impl RoutingStrategy for PriorityRouter {
    async fn select_provider(&self, providers: &[ProviderConfig], _req: &CompletionRequest) -> Result<usize> {
        // Always try first provider first
        Ok(0)
    }
}
```
- Use case: Primary/backup setup
- Behavior: Try providers in order until success

#### 2. Round-Robin
```rust
pub struct RoundRobinRouter {
    counter: Arc<AtomicUsize>,
}

#[async_trait]
impl RoutingStrategy for RoundRobinRouter {
    async fn select_provider(&self, providers: &[ProviderConfig], _req: &CompletionRequest) -> Result<usize> {
        let count = self.counter.fetch_add(1, Ordering::Relaxed);
        Ok(count % providers.len())
    }
}
```
- Use case: Load balancing
- Behavior: Distribute evenly

#### 3. Latency-Based (Most Complex)
```rust
pub struct LatencyRouter {
    metrics: Arc<RwLock<HashMap<usize, ProviderMetrics>>>,
}

#[async_trait]
impl RoutingStrategy for LatencyRouter {
    async fn select_provider(&self, providers: &[ProviderConfig], _req: &CompletionRequest) -> Result<usize> {
        let metrics = self.metrics.read().await;

        // Find provider with lowest average latency
        let best = metrics.iter()
            .min_by_key(|(_, m)| m.avg_latency)
            .map(|(idx, _)| *idx)
            .unwrap_or(0);

        Ok(best)
    }

    async fn report_success(&self, idx: usize, latency: Duration) {
        let mut metrics = self.metrics.write().await;
        let m = metrics.entry(idx).or_default();

        // Exponential moving average
        m.avg_latency = Duration::from_millis(
            (m.avg_latency.as_millis() as f32 * 0.9
             + latency.as_millis() as f32 * 0.1) as u64
        );
    }
}
```

### Retry Logic

```rust
pub struct RetryExecutor {
    config: RetryConfig,
}

impl RetryExecutor {
    pub async fn execute_with_retry<F, Fut, T>(
        &self,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut attempt = 0;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_retryable() && attempt < self.config.max_attempts => {
                    attempt += 1;
                    let backoff = self.config.calculate_backoff(attempt);
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

### Circuit Breaker Pattern

```rust
pub struct CircuitBreaker {
    state: Arc<RwLock<State>>,
    failure_threshold: u32,
    timeout: Duration,
}

enum State {
    Closed,  // Normal operation
    Open { until: Instant },  // Failing, don't try
    HalfOpen,  // Testing if recovered
}

impl CircuitBreaker {
    pub async fn call<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let state = self.state.read().await.clone();

        match state {
            State::Open { until } if Instant::now() < until => {
                Err(SimpleAgentsError::Routing("Circuit breaker open".into()))
            }
            State::Open { .. } => {
                // Try to recover
                *self.state.write().await = State::HalfOpen;
                self.try_operation(operation).await
            }
            State::HalfOpen | State::Closed => {
                self.try_operation(operation).await
            }
        }
    }
}
```

### Fallback Chain

```rust
pub struct FallbackExecutor {
    providers: Vec<Box<dyn Provider>>,
    router: Box<dyn RoutingStrategy>,
    retry: RetryExecutor,
}

impl FallbackExecutor {
    pub async fn execute(&self, request: CompletionRequest)
        -> Result<CompletionResponse>
    {
        let mut last_error = None;

        // Try each provider
        for (idx, provider) in self.providers.iter().enumerate() {
            match self.try_provider(provider, &request).await {
                Ok(response) => {
                    self.router.report_success(idx, response.latency).await;
                    return Ok(response);
                }
                Err(e) => {
                    self.router.report_failure(idx).await;
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            SimpleAgentsError::Routing("All providers failed".into())
        }))
    }
}
```

---

## 🎯 Phase 5 Planning: Core (`simple-agents-core`)

### Goals
- Unified client API
- Bring everything together
- Simple API for common use cases
- Advanced API for power users

### Client Design

```rust
pub struct SimpleAgentsClient {
    providers: Vec<Box<dyn Provider>>,
    router: Box<dyn RoutingStrategy>,
    cache: Option<Box<dyn Cache>>,
    healer: Option<JsonHealer>,
    retry: RetryExecutor,
    middleware: Vec<Box<dyn Middleware>>,
}

impl SimpleAgentsClient {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    // Simple API
    pub async fn complete(&self, prompt: &str) -> Result<String> {
        let request = CompletionRequest::builder()
            .model("gpt-4")  // default
            .message(Message::user(prompt))
            .build()?;

        let response = self.execute(request).await?;
        Ok(response.content().unwrap_or("").to_string())
    }

    // Advanced API
    pub async fn execute(&self, request: CompletionRequest)
        -> Result<CompletionResponse>
    {
        // 1. Check cache
        if let Some(cached) = self.check_cache(&request).await? {
            return Ok(cached);
        }

        // 2. Select provider
        let provider_idx = self.router.select_provider(&self.providers, &request).await?;
        let provider = &self.providers[provider_idx];

        // 3. Execute with retry
        let response = self.retry.execute_with_retry(|| async {
            // Transform request
            let provider_req = provider.transform_request(&request)?;

            // Execute
            let provider_resp = provider.execute(provider_req).await?;

            // Transform response
            provider.transform_response(provider_resp)
        }).await?;

        // 4. Cache result
        self.cache_response(&request, &response).await?;

        Ok(response)
    }

    // Streaming API
    pub async fn stream(&self, request: CompletionRequest)
        -> Result<impl Stream<Item = Result<CompletionChunk>>>
    {
        // Similar but return stream
    }
}
```

### Builder API

```rust
let client = SimpleAgentsClient::builder()
    .add_provider(OpenAIProvider::new(openai_key)?)
    .add_provider(AnthropicProvider::new(anthropic_key)?)
    .with_routing(RoutingStrategy::Priority)
    .with_retry(RetryConfig::default())
    .with_cache(InMemoryCache::new())
    .with_healing(HealingConfig::lenient())
    .build()?;
```

### Middleware System

```rust
#[async_trait]
pub trait Middleware: Send + Sync {
    async fn before_request(&self, req: &mut CompletionRequest) -> Result<()>;
    async fn after_response(&self, resp: &mut CompletionResponse) -> Result<()>;
}

// Example: Logging middleware
pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before_request(&self, req: &mut CompletionRequest) -> Result<()> {
        info!("Request: model={}, messages={}", req.model, req.messages.len());
        Ok(())
    }

    async fn after_response(&self, resp: &mut CompletionResponse) -> Result<()> {
        info!("Response: tokens={}", resp.usage.total_tokens);
        Ok(())
    }
}
```

---

## 💡 Ideas & Future Considerations

### Performance Optimizations

1. **Connection Pooling**
   - Reuse HTTP connections
   - Configure in `reqwest::Client::builder()`

2. **Parallel Requests**
   - For `n > 1` in request
   - Use `tokio::spawn` for parallel execution

3. **Request Batching**
   - Batch multiple small requests
   - Some providers support this

### Observability

1. **OpenTelemetry Integration**
   ```rust
   #[instrument]
   async fn execute(&self, request: CompletionRequest) -> Result<...> {
       // Auto-traced
   }
   ```

2. **Metrics Export**
   - Prometheus metrics
   - Request counts, latencies, error rates
   - Per-provider metrics

3. **Structured Logging**
   - Use `tracing` instead of `log`
   - Contextual logs with spans

### Advanced Features

1. **Semantic Caching**
   - Cache based on embedding similarity
   - Not just exact matches
   - Use vector database

2. **Function Calling**
   - Define tools/functions
   - LLM decides when to call
   - Execute and return results
   - Continue conversation

3. **Vision Support**
   - Image inputs
   - Base64 or URL
   - GPT-4V, Claude 3 support

4. **Embeddings**
   - Separate API
   - Different request/response types
   - Batch support important

### Deployment Considerations

1. **Docker**
   - Provide Dockerfile
   - Multi-stage build for small image
   - Health check endpoint

2. **Kubernetes**
   - Helm chart
   - ConfigMap for settings
   - Secret for API keys

3. **Serverless**
   - Cold start optimization
   - Connection reuse critical
   - Consider Lambda/Cloud Functions

---

## ⚠️ Watch Out For

### Common Pitfalls

1. **Rate Limits**
   - Always respect `retry-after` headers
   - Don't retry 429s immediately
   - Consider token bucket algorithm

2. **Timeout Handling**
   - Set reasonable defaults (30s)
   - Allow configuration
   - Distinguish network timeout vs provider timeout

3. **Large Responses**
   - Some models generate VERY long responses
   - Stream instead of buffer
   - Consider max token limits

4. **API Key Security**
   - Never log in plaintext
   - Never commit to git
   - Use environment variables
   - Consider key rotation

5. **Cost Control**
   - Track token usage
   - Set limits
   - Alert on unexpected usage
   - Different models have VERY different costs

### Testing Challenges

1. **Flaky Tests**
   - Real API calls can fail randomly
   - Use mocks for CI
   - Real tests opt-in only

2. **Rate Limiting in Tests**
   - Don't spam APIs in tests
   - Use test API keys with limits
   - Implement backoff even in tests

3. **Cost of Testing**
   - Real API calls cost money
   - Use cheaper models for tests
   - Cache test responses

---

## 📚 Resources & References

### Rust Libraries
- `reqwest` - HTTP client
- `tokio` - Async runtime
- `serde` - Serialization
- `thiserror` - Error handling
- `async-trait` - Async traits
- `tracing` - Observability
- `tower` - Middleware patterns

### LLM APIs
- [OpenAI API Docs](https://platform.openai.com/docs)
- [Anthropic API Docs](https://docs.anthropic.com/)
- [OpenAI Rate Limits](https://platform.openai.com/docs/guides/rate-limits)
- [Anthropic Rate Limits](https://docs.anthropic.com/claude/reference/rate-limits)

### Design Patterns
- Builder pattern for configuration
- Strategy pattern for routing
- Middleware pattern for extensibility
- Circuit breaker for resilience

### Similar Projects (for inspiration)
- `openai-rust` - Async OpenAI client
- `anthropic-sdk-rust` - Anthropic official SDK
- `langchain-rust` - LangChain port
- `litellm` - Multi-provider Python library

---

## ✅ Quick Reference

### Testing Commands
```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p simple-agents-types

# Run ignored tests (integration)
cargo test -- --ignored

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Check without building
cargo check

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Build docs
cargo doc --no-deps --open
```

### Git Workflow
```bash
# Create feature branch
git checkout -b feature/phase-2-providers

# Commit changes
git add .
git commit -m "feat(providers): implement OpenAI provider"

# Push
git push origin feature/phase-2-providers
```

### Debug Tips
```rust
// Print debug
println!("{:#?}", value);

// Use dbg! macro
let result = dbg!(some_function());

// Conditional compilation
#[cfg(debug_assertions)]
println!("Debug only");
```

---

**Last Updated**: 2026-01-16
**Next Update**: When starting Phase 2 implementation
