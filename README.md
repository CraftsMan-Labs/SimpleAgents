# SimpleAgents

A high-performance, type-safe Rust framework for building LLM-powered applications with **response healing** and **multi-provider abstraction**.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-203%20passing-success.svg)](./TODO.md)
[![Clippy](https://img.shields.io/badge/clippy-0%20warnings-success.svg)](./TODO.md)
[![Docs](https://img.shields.io/badge/docs-150%2B%20pages-blue.svg)](./research/)

---

## Overview

SimpleAgents combines the best of **LiteLLM's multi-provider capabilities** with **BAML's response healing system** to provide a production-ready Rust framework for LLM applications. Built on Rust's zero-cost abstractions, it offers:

- **🔧 Multi-Provider Support**: Unified interface for OpenAI, Anthropic, OpenRouter, and 100+ providers
- **🩹 Response Healing**: Parse malformed JSON from LLMs with confidence scoring and transparency *(Coming in Phase 3)*
- **🔒 Type Safety**: Comprehensive compile-time guarantees through Rust's type system
- **⚡ Performance**: Zero-copy operations, HTTP/2 connection pooling, Blake3 caching (10x faster than SHA-256)
- **🛡️ Security**: Constant-time API key comparison, automatic input validation, keys never logged
- **🔄 Reliability**: Exponential backoff with jitter, rate limiting, fallback chains
- **📦 Extensibility**: Provider trait system with pluggable routing strategies

---

## Quick Start

### Installation

#### Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
simple-agents-types = "0.1.0"
simple-agents-providers = "0.1.0"
simple-agents-cache = "0.1.0"  # Optional
tokio = { version = "1.35", features = ["full"] }
```

#### Python

[![PyPI](https://img.shields.io/pypi/v/simple-agents-py)](https://pypi.org/project/simple-agents-py/)
[![PyPI - Downloads](https://img.shields.io/pypi/dm/simple-agents-py)](https://pypi.org/project/simple-agents-py/)

Install from [PyPI](https://pypi.org/project/simple-agents-py/):

```sh
pip install simple-agents-py
```

See [crates/simple-agents-py/README.md](crates/simple-agents-py/README.md) for Python usage examples.

### Basic Example (Rust)

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::Provider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create provider
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    // Build request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Explain Rust ownership in one sentence"))
        .temperature(0.7)
        .build()?;

    // Execute (3-phase pattern)
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    // Print response
    println!("{}", response.content().unwrap_or("No response"));
    Ok(())
}
```

---

## Features

### ✅ Available Now (Phase 1 & 2)

#### Multi-Provider Support
- **OpenAI**: GPT-3.5, GPT-4, GPT-4-turbo with streaming
- **Anthropic**: Claude-3 (Opus, Sonnet, Haiku) with system message extraction
- **OpenRouter**: 100+ models via unified interface with provider prefixes

#### Reliability & Performance
- **Retry Logic**: Exponential backoff with jitter (prevents thundering herd)
- **Rate Limiting**: Token bucket algorithm (per-instance and shared)
- **Caching**: InMemoryCache with LRU eviction and TTL support
- **Connection Pooling**: HTTP/2 multiplexing (10 idle connections per host, 90s timeout)
- **Streaming**: Server-Sent Events (SSE) support for both OpenAI and Anthropic

#### Security
- **API Keys**: Never logged, constant-time comparison, automatic redaction
- **Input Validation**: Automatic request validation
- **Type Safety**: Compile-time guarantees for all operations

### 🚧 Coming Soon (Phase 3+)

- **Response Healing**: BAML-inspired JSON parser for malformed LLM outputs
- **Routing Strategies**: Round-robin, latency-based, cost-based routing
- **Fallback Chains**: Automatic provider failover
- **Observability**: Metrics, tracing, and monitoring
- **CLI Tool**: Command-line interface for testing
- **Language Bindings**: Python, TypeScript, Go

---

## Architecture

### Three-Phase Provider Pattern

```rust
// Phase 1: Transform unified request to provider format
fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest>;

// Phase 2: Execute HTTP request
async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse>;

// Phase 3: Transform provider response to unified format
fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse>;
```

**Benefits**:
- Clean separation of concerns
- Easy testing of each phase independently
- Provider-agnostic application code
- Simple addition of new providers

### Crate Structure

```
SimpleAgents/
├── simple-agents-types/         # Core types and traits ✅
├── simple-agents-providers/     # Provider implementations ✅
├── simple-agents-cache/         # Caching strategies ✅
├── simple-agents-healing/       # Response healing 📅 Phase 3
├── simple-agents-router/        # Routing strategies 📅 Phase 4
├── simple-agents-core/          # Unified client API 📅 Phase 5
├── simple-agents-cli/           # CLI tool 📅 Phase 6
└── simple-agents-ffi/           # FFI bindings 📅 Phase 7
```

---

## Examples

### Multi-Turn Conversation

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::system("You are a helpful Rust expert."))
    .message(Message::user("What is a lifetime?"))
    .message(Message::assistant("A lifetime is a Rust construct that ensures references are valid."))
    .message(Message::user("Can you give an example?"))
    .build()?;
```

### Streaming Completion

```rust
use simple_agents_providers::openai::OpenAIProvider;
use futures::StreamExt;

let provider = OpenAIProvider::new(api_key)?;

// Enable streaming in request
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Write a haiku about Rust"))
    .stream(true)
    .build()?;

let provider_request = provider.transform_request(&request)?;
let mut stream = provider.execute_stream(provider_request).await?;

// Process chunks as they arrive
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);
}
```

### Caching Responses

```rust
use simple_agents_cache::{InMemoryCache, CacheKey};
use std::time::Duration;

let cache = InMemoryCache::new(
    10 * 1024 * 1024,  // 10MB max size
    1000,               // 1000 max entries
);

// Generate cache key (Blake3 hashing - 10x faster than SHA-256)
let cache_key = CacheKey::from_parts("openai", "gpt-4", "What is Rust?");

// Check cache
if let Some(cached) = cache.get(&cache_key).await? {
    return Ok(serde_json::from_slice(&cached)?);
}

// Execute request...
let response = /* ... */;

// Cache for 1 hour
let response_bytes = serde_json::to_vec(&response)?;
cache.set(&cache_key, response_bytes, Duration::from_secs(3600)).await?;
```

### Retry with Exponential Backoff

```rust
use simple_agents_providers::retry::execute_with_retry;
use simple_agents_types::config::RetryConfig;

let config = RetryConfig {
    max_attempts: 3,
    initial_backoff: Duration::from_millis(100),
    max_backoff: Duration::from_secs(10),
    multiplier: 2.0,
    jitter: true,  // ±30% randomization prevents thundering herd
};

let response = execute_with_retry(
    &config,
    |e| e.is_retryable(),
    || provider.execute(provider_request.clone())
).await?;
```

### Rate Limiting

```rust
use simple_agents_types::config::RateLimitConfig;

let provider = OpenAIProvider::new(api_key)?
    .with_rate_limit(RateLimitConfig {
        requests_per_second: 50,
        burst_size: 10,
        shared: true,  // Share limiter across instances with same API key
    });
```

More examples in [`crates/simple-agents-providers/examples/`](crates/simple-agents-providers/examples/).

---

## Performance

SimpleAgents is designed for production use with real-world performance characteristics:

| Optimization | Benefit |
|--------------|---------|
| **HTTP/2 Multiplexing** | Reuses TCP and TLS sessions (~300ms savings per request) |
| **Blake3 Hashing** | 10x faster than SHA-256 for cache keys (1.5GB/s vs 150MB/s) |
| **Zero-Copy Operations** | Borrows instead of cloning (saves MB per request) |
| **Connection Pooling** | 10 idle connections per host, 90s timeout |
| **Jittered Backoff** | ±30% randomization prevents thundering herd |

**Overhead**: ~50ms per request (includes serialization + validation)

See [OPTIMISATION.md](OPTIMISATION.md) for detailed analysis.

---

## Testing

```bash
# Run all tests
cargo test --all

# Run tests for specific crate
cargo test -p simple-agents-providers

# Run with ignored integration tests (requires API keys)
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
cargo test --all -- --ignored
```

**Current Status**:
- ✅ 203 passing tests (171 unit/integration + 32 doctests)
- ✅ 16 ignored tests (require API keys or async runtime)
- ✅ Zero clippy warnings
- ✅ 100% documentation coverage for public APIs

---

## Documentation

### Quick Reference

| Document | Purpose | When to Read |
|----------|---------|--------------|
| **[README.md](README.md)** (this file) | Project overview, quick start | Start here |
| **[TODO.md](TODO.md)** | Task tracking, progress, next steps | Implementation planning |
| **[CODING_GUIDELINES.md](CODING_GUIDELINES.md)** | Rust best practices, design patterns | Before writing code |
| **[OPTIMISATION.md](OPTIMISATION.md)** | Performance optimizations | When optimizing |
| **[research/README.md](research/README.md)** | Research overview and navigation | Understanding architecture |
| **[PHASE2_COMPLETION_REPORT.md](PHASE2_COMPLETION_REPORT.md)** | Phase 2 detailed report | Implementation reference |

### Research Documents

Comprehensive analysis of leading LLM frameworks (44,500+ lines of code analyzed):

- **[litellm-analysis.md](research/litellm-analysis.md)** - 115 providers, routing, retry patterns
- **[baml-analysis.md](research/baml-analysis.md)** - Response healing, parsing, coercion
- **[implementation-plan.md](research/implementation-plan.md)** - 12-week roadmap, architecture
- **[mvp-scope-update.md](research/mvp-scope-update.md)** - MVP features, streaming

---

## Roadmap

### ✅ Phase 1: Foundation (Weeks 1-2) - Complete

- Core type system (`simple-agents-types`)
- API key security (never logged, constant-time comparison)
- Builder patterns for ergonomic APIs
- Transparency tracking (CoercionFlag system)
- 114 comprehensive tests

### ✅ Phase 2: Provider Integration (Weeks 3-4) - Complete

- OpenAI, Anthropic, OpenRouter providers
- Retry logic with exponential backoff
- Rate limiting with token bucket algorithm
- InMemoryCache with LRU eviction
- HTTP/2 connection pooling
- Streaming support (SSE)
- 203 passing tests

### 📅 Phase 3: Response Healing (Weeks 5-6) - Next

- Jsonish parser for malformed JSON
- Type coercion engine with confidence scoring
- Fuzzy field matching
- Streaming parser with partial types
- Flag system for transparency

### 📅 Phase 4-7: Router, Core, CLI, Bindings (Weeks 7-12)

- Routing strategies (round-robin, latency-based, cost-based)
- Fallback chains for provider failover
- Unified `SimpleAgentsClient` API
- CLI tool for testing
- Python, TypeScript, Go bindings

See [TODO.md](TODO.md) for detailed task tracking and progress.

---

## Contributing

Contributions are welcome! Please see:

- **[TODO.md](TODO.md)** - Current tasks and priorities
- **[CODING_GUIDELINES.md](CODING_GUIDELINES.md)** - Code style and best practices
- **[research/](research/)** - Architecture decisions and patterns

### Development Workflow

```bash
# Clone repository
git clone https://github.com/yourusername/SimpleAgents.git
cd SimpleAgents

# Build project
cargo build --all

# Run tests
cargo test --all

# Check code quality
cargo clippy --all-targets
cargo fmt --all -- --check

# Run examples
cargo run --example openai_basic
```

You can also use the Makefile shortcuts:

```bash
# List available make targets
make help

# Run a provider example (default: openai_basic)
make example-providers

# Run a specific provider example
make example-providers EXAMPLE=anthropic_basic

# Run the full API example
make example-full-api

# Run both provider + full API examples
make examples
```

---

## Key Technical Innovations

### 1. Three-Phase Provider Architecture

Clean separation of request transformation, execution, and response transformation enables independent testing and easy provider addition.

### 2. Response Healing System (Phase 3)

BAML-inspired JSON parser handles:
- Markdown code fences: ` ```json {...} ``` `
- Trailing commas: `{"key": "value",}`
- Type coercion: `"42"` → `42` with transparency
- Fuzzy field matching: `userName` → `user_name`

### 3. Intelligent Retry Logic

Exponential backoff with jitter prevents thundering herd while respecting provider `retry-after` headers.

### 4. Provider-Agnostic Abstraction

Write once, run with any provider:
```rust
// Same code works with OpenAI, Anthropic, OpenRouter
let response = client.completion()
    .model("gpt-4")  // or "claude-3-opus", "openrouter/meta-llama/..."
    .messages(messages)
    .send()
    .await?;
```

---

## Project Status

**Current Version**: 0.1.0-alpha
**Phase**: Phase 3 (Response Healing) - Ready to Start
**Completion**: 29% (2/7 phases complete)

**Metrics**:
- ✅ 203 passing tests
- ✅ Zero clippy warnings
- ✅ 3 of 9 crates complete
- ✅ 3 major providers implemented
- ✅ 150+ pages of documentation

**Requirements**:
- Rust 1.75+
- Tokio 1.35+

**Last Updated**: 2026-01-23

---

## License

MIT OR Apache-2.0

---

## Acknowledgments

Built with:
- [tokio](https://tokio.rs/) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [serde](https://serde.rs/) - Serialization
- [thiserror](https://github.com/dtolnay/thiserror) - Error handling
- [blake3](https://github.com/BLAKE3-team/BLAKE3) - Fast hashing
- [subtle](https://github.com/dalek-cryptography/subtle) - Constant-time operations
- [governor](https://github.com/beltram/governor) - Rate limiting

Inspired by:
- [LiteLLM](https://github.com/BerriAI/litellm) - Multi-provider abstraction
- [BAML](https://github.com/BoundaryML/baml) - Response healing system

---

## Support

- **Task Tracking**: [TODO.md](TODO.md) - Single source of truth for project tasks
- **Documentation**: See [`research/`](research/) for comprehensive analysis
- **Issues**: [GitHub Issues](https://github.com/yourusername/SimpleAgents/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/SimpleAgents/discussions)

---

**Ready to start building?** Check out the [examples](crates/simple-agents-providers/examples/) or dive into [Phase 3 tasks](TODO.md#-current-work).
