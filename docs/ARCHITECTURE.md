# Architecture Overview

This document describes the architecture and design decisions behind SimpleAgents.

## Table of Contents

- [Design Philosophy](#design-philosophy)
- [System Architecture](#system-architecture)
- [Core Abstractions](#core-abstractions)
- [Data Flow](#data-flow)
- [Provider System](#provider-system)
- [Caching Layer](#caching-layer)
- [Error Handling](#error-handling)
- [Security Model](#security-model)
- [Performance Optimizations](#performance-optimizations)

## Design Philosophy

SimpleAgents is built on these core principles:

### 1. Type Safety First
- Leverage Rust's type system to prevent errors at compile time
- Use newtypes for domain-specific values (e.g., `ApiKey`)
- Make invalid states unrepresentable

### 2. Zero-Cost Abstractions
- No runtime overhead for abstractions
- Use generics and monomorphization
- Avoid unnecessary allocations

### 3. Security by Default
- Constant-time comparisons for secrets
- Automatic input validation
- No logging of sensitive data

### 4. Extensibility
- Provider trait system for multiple APIs
- Cache trait for different storage backends
- Minimal coupling between components

### 5. Developer Experience
- Clear error messages
- Comprehensive documentation
- Predictable APIs

## System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Application                         │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  simple-agents-types                     │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐ │
│  │   Request   │  │   Response   │  │    Message     │ │
│  └─────────────┘  └──────────────┘  └────────────────┘ │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐ │
│  │  Provider   │  │    Cache     │  │   Validation   │ │
│  │   (trait)   │  │   (trait)    │  │    (ApiKey)    │ │
│  └─────────────┘  └──────────────┘  └────────────────┘ │
└─────────────────────────┬───────────────────────────────┘
                          │
            ┌─────────────┴─────────────┐
            ▼                           ▼
┌───────────────────────┐   ┌──────────────────────┐
│ simple-agents-        │   │ simple-agents-cache  │
│ providers             │   │                      │
│ ┌─────────────────┐   │   │ ┌────────────────┐  │
│ │  OpenAI         │   │   │ │  InMemory      │  │
│ │  Provider       │   │   │ │  (LRU + TTL)   │  │
│ └─────────────────┘   │   │ └────────────────┘  │
│ ┌─────────────────┐   │   │ ┌────────────────┐  │
│ │  Anthropic      │   │   │ │  NoOp          │  │
│ │  Provider       │   │   │ │  (testing)     │  │
│ └─────────────────┘   │   │ └────────────────┘  │
│ ┌─────────────────┐   │   └──────────────────────┘
│ │  Retry Logic    │   │
│ └─────────────────┘   │
└───────────────────────┘
            │
            ▼
    ┌──────────────┐
    │  HTTP/2      │
    │  Connection  │
    │  Pool        │
    └──────────────┘
            │
            ▼
    ┌──────────────┐
    │  LLM API     │
    │  (OpenAI,    │
    │   Anthropic) │
    └──────────────┘
```

## Core Abstractions

### Provider Trait

The `Provider` trait defines a three-phase architecture for LLM interactions:

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    // Phase 1: Transform unified request to provider format
    fn transform_request(&self, req: &CompletionRequest)
        -> Result<ProviderRequest>;

    // Phase 2: Execute HTTP request
    async fn execute(&self, req: ProviderRequest)
        -> Result<ProviderResponse>;

    // Phase 3: Transform provider response to unified format
    fn transform_response(&self, resp: ProviderResponse)
        -> Result<CompletionResponse>;
}
```

**Benefits:**
- Clean separation of concerns
- Easy to test each phase independently
- Provider-agnostic application code
- Simple to add new providers

### Request/Response Types

**Unified Types** (application-facing):
- `CompletionRequest` - Standard request format
- `CompletionResponse` - Standard response format
- `Message` - Conversation messages

**Provider Types** (provider-facing):
- `ProviderRequest` - HTTP request details
- `ProviderResponse` - HTTP response details

This separation allows:
- Applications to be provider-agnostic
- Providers to have full control over HTTP
- Easy migration between providers

### Cache Trait

Simple async trait for caching:

```rust
#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn clear(&self) -> Result<()>;
}
```

**Key Features:**
- Async-first design
- Binary data (allows any serialization format)
- TTL support
- Simple to implement

## Data Flow

### Typical Request Flow

```
1. Application creates CompletionRequest
   ↓
2. transform_request() → ProviderRequest
   ↓
3. execute() → HTTP call → ProviderResponse
   ↓
4. transform_response() → CompletionResponse
   ↓
5. Application uses response
```

### With Caching

```
1. Application creates CompletionRequest
   ↓
2. Generate cache key from request
   ↓
3. Check cache.get(key)
   ├─ Hit → Return cached response
   └─ Miss ↓
4. transform_request() → ProviderRequest
   ↓
5. execute() → HTTP call → ProviderResponse
   ↓
6. transform_response() → CompletionResponse
   ↓
7. cache.set(key, response, ttl)
   ↓
8. Return response to application
```

### With Retry Logic

```
1. Application creates CompletionRequest
   ↓
2. transform_request() → ProviderRequest
   ↓
3. execute_with_retry()
   ├─ Attempt 1 → Fail (retryable error)
   ├─ Backoff (exponential + jitter)
   ├─ Attempt 2 → Fail (retryable error)
   ├─ Backoff (exponential + jitter)
   └─ Attempt 3 → Success ↓
4. transform_response() → CompletionResponse
   ↓
5. Application uses response
```

## Provider System

### OpenAI Provider

**Request Transformation:**
```rust
CompletionRequest → OpenAICompletionRequest → JSON
```

**Key Features:**
- Borrows messages (zero-copy)
- Uses static headers with `Cow`
- HTTP/2 connection pooling
- Structured error mapping

**Error Handling:**
- Maps HTTP status codes to semantic errors
- Extracts retry-after headers
- Logs errors with context

### Adding New Providers

New providers implement:
1. Request/response models
2. Error types and mapping
3. Provider trait implementation
4. Tests for all three phases

**Example Structure:**
```
providers/
└── myprovider/
    ├── mod.rs        # Provider implementation
    ├── models.rs     # Request/response types
    └── error.rs      # Error mapping
```

## Caching Layer

### InMemoryCache

**Architecture:**
```
┌─────────────────────────────────────┐
│         InMemoryCache               │
│                                     │
│  ┌───────────────────────────────┐ │
│  │  Arc<RwLock<HashMap>>         │ │
│  │                               │ │
│  │  CacheEntry {                 │ │
│  │    data: Vec<u8>,             │ │
│  │    expires_at: Instant,       │ │
│  │    last_accessed: Instant     │ │
│  │  }                            │ │
│  └───────────────────────────────┘ │
│                                     │
│  Eviction Strategies:               │
│  • TTL-based (expires_at)          │
│  • LRU (last_accessed)             │
│  • Size-based (max_size)           │
│  • Count-based (max_entries)       │
└─────────────────────────────────────┘
```

**Eviction Algorithm:**
1. On every `get`: Remove expired entries
2. On every `set`: Check limits
3. If over limits: Sort by `last_accessed`, remove oldest

**Thread Safety:**
- Uses `Arc<RwLock<>>` for shared state
- Readers can run concurrently
- Writers block all access

### Cache Key Generation

Uses blake3 for fast, deterministic hashing:

```rust
pub fn from_parts(provider: &str, model: &str, content: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(provider.as_bytes());
    hasher.update(model.as_bytes());
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();
    format!("{}:{}:{}", provider, model, hash.to_hex())
}
```

**Format:** `provider:model:hash`

**Benefits:**
- Deterministic (same input → same key)
- Collision-resistant
- Fast (blake3 is one of the fastest hashes)
- Readable prefix for debugging

## Error Handling

### Error Hierarchy

```
SimpleAgentsError (enum)
├── Validation(ValidationError)
│   ├── Empty { field }
│   ├── TooShort { field, min }
│   ├── TooLong { field, max }
│   ├── OutOfRange { field, min, max }
│   └── InvalidFormat { field, reason }
│
├── Provider(ProviderError)
│   ├── Authentication(String)
│   ├── RateLimit { retry_after, message }
│   ├── InvalidResponse(String)
│   ├── ModelNotFound(String)
│   ├── ContextLengthExceeded { max_tokens }
│   ├── Timeout(Duration)
│   └── UnsupportedFeature(String)
│
├── Network(String)
├── Serialization(String)
├── Cache(String)
└── Config(String)
```

### Error Context

Errors include:
- **What went wrong**: Clear error message
- **Where**: Field names, file locations
- **Why**: Underlying cause (if any)
- **How to fix**: Suggestions in documentation

### Retryable Errors

The system distinguishes between:

**Retryable:**
- Rate limits (429)
- Server errors (5xx)
- Timeouts
- Temporary network issues

**Non-retryable:**
- Invalid API key (401)
- Bad request (400)
- Not found (404)
- Validation errors

## Security Model

### Defense in Depth

**Layer 1: Input Validation**
- All requests validated before processing
- Size limits enforced
- Null byte detection
- Character set validation

**Layer 2: Secrets Handling**
- `ApiKey` type prevents accidental exposure
- Never logged or serialized in plain text
- Constant-time comparisons
- Explicit `.expose()` required

**Layer 3: Cryptographic Security**
- Secure RNG for jitter (prevents predictability)
- Blake3 for cache keys (prevents collisions)
- Constant-time operations for secrets

**Layer 4: Network Security**
- TLS for all API calls (via reqwest)
- Connection pooling reduces attack surface
- Timeout enforcement

### Constant-Time Operations

Critical for preventing timing attacks:

```rust
use subtle::ConstantTimeEq;

// API key comparison
impl PartialEq for ApiKey {
    fn eq(&self, other: &Self) -> bool {
        // Takes same time regardless of where keys differ
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}
```

**Why it matters:**
An attacker could try guessing API keys character by character. With normal comparison, the function returns faster when the first character is wrong vs when many characters are correct. This timing difference leaks information.

## Performance Optimizations

### 1. Zero-Copy Message Passing

```rust
// Before: Clones all messages
pub struct Request {
    pub messages: Vec<Message>,  // Owned
}

// After: Borrows messages
pub struct Request<'a> {
    pub messages: &'a [Message],  // Borrowed
}
```

**Impact:** Eliminates potentially megabytes of allocations per request.

### 2. Static String Allocation

```rust
// Headers use Cow for zero-allocation static strings
pub headers: Vec<(Cow<'static, str>, Cow<'static, str>)>

// Common headers are static
headers.push((
    Cow::Borrowed("Content-Type"),
    Cow::Borrowed("application/json")
));
```

**Impact:** Eliminates heap allocations for common headers.

### 3. Connection Pooling

```rust
Client::builder()
    .pool_max_idle_per_host(10)
    .pool_idle_timeout(Duration::from_secs(90))
    .http2_prior_knowledge()
```

**Impact:**
- Reuses TCP connections (saves ~100ms per request)
- Reuses TLS sessions (saves ~200ms per request)
- HTTP/2 multiplexing (multiple requests on one connection)

### 4. Smart Caching

**LRU Eviction:**
- Keeps hot data in cache
- Automatically removes cold data
- Configurable size/count limits

**TTL-based Expiry:**
- Prevents stale data
- Automatic cleanup

**Blake3 Hashing:**
- ~10x faster than SHA-256
- Parallelizable
- Collision-resistant

### 5. Lazy Validation

Validation happens only when:
- Building a request (`.build()`)
- Not on field setters

**Benefits:**
- Better error messages (all errors at once)
- Faster builder API
- Validation only happens once

## Design Decisions

### Why Traits Over Enums for Providers?

**Considered:**
```rust
enum Provider {
    OpenAI(OpenAIProvider),
    Anthropic(AnthropicProvider),
}
```

**Chose Trait Instead:**
```rust
trait Provider { ... }
```

**Reasons:**
1. Open for extension (users can add providers)
2. No overhead for dispatch (monomorphization)
3. Better encapsulation
4. Easier to test

### Why Three-Phase Provider Architecture?

Separates:
1. **Transform Request** - Pure, testable
2. **Execute** - Side effects, retries
3. **Transform Response** - Pure, testable

**Benefits:**
- Each phase can be tested independently
- Easy to add middleware (logging, metrics)
- Clear separation of concerns

### Why Separate Crates?

**simple-agents-types:**
- Pure interfaces
- No implementation dependencies
- Can be used standalone

**simple-agents-providers:**
- Concrete implementations
- HTTP dependencies
- Provider-specific code

**simple-agents-cache:**
- Optional caching
- Can swap implementations
- Minimal dependencies

**Benefits:**
- Faster compilation (parallel)
- Clearer boundaries
- Optional features

### Why Async?

LLM APIs are:
- Network-bound (not CPU-bound)
- High latency (1-10s responses)
- Benefit from concurrency

Async allows:
- Multiple concurrent requests
- Efficient resource usage
- Natural API for streaming

## Future Architecture

### Planned Improvements

1. **Rate Limiting**
   - Token bucket algorithm
   - Per-provider limits
   - Automatic backoff

2. **Observability**
   - Metrics collection
   - Distributed tracing
   - Performance monitoring

3. **Advanced Caching**
   - Redis backend
   - Semantic caching
   - Cache warming

4. **Streaming**
   - Complete SSE parsing
   - Backpressure handling
   - Chunk aggregation

5. **Advanced Routing**
   - Load balancing
   - Fallback providers
   - Cost optimization

## References

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Async Book](https://rust-lang.github.io/async-book/)
- [Type-Driven Development](https://blog.ploeh.dk/2015/08/10/type-driven-development/)
- [Error Handling in Rust](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
