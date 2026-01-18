# SimpleAgents

A high-performance, type-safe Rust framework for building LLM-powered applications.

## Overview

SimpleAgents provides a unified interface for interacting with multiple LLM providers while maintaining strict type safety, security, and performance. Built on Rust's zero-cost abstractions, it offers:

- **Type Safety**: Comprehensive compile-time guarantees through Rust's type system
- **Performance**: Zero-copy operations, connection pooling, and efficient caching
- **Security**: Constant-time comparisons, secure RNG, and automatic input validation
- **Extensibility**: Provider trait system for supporting multiple LLM APIs
- **Production-Ready**: Retry logic, error handling, and observability built-in

## Features

✅ **OpenAI API Support** - Full support for GPT-4, GPT-3.5-Turbo, and other models
✅ **In-Memory Caching** - LRU eviction with TTL support
✅ **Retry Logic** - Exponential backoff with jitter
✅ **Connection Pooling** - HTTP/2 multiplexing for optimal performance
✅ **Streaming Support** - Framework for streaming responses (SSE)
✅ **Security Focused** - Constant-time API key comparison, blake3 hashing
✅ **Zero-Copy Operations** - Borrowed data structures for minimal allocations
✅ **Comprehensive Testing** - 132+ tests covering all functionality

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
simple-agents-types = "0.1.0"
simple-agents-providers = "0.1.0"
simple-agents-cache = "0.1.0"  # Optional
tokio = { version = "1.35", features = ["full"] }
```

### Basic Example

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

#[tokio::main]
async fn main() -> Result<()> {
    // Create provider
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    // Build request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello, world!"))
        .temperature(0.7)
        .build()?;

    // Execute
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    // Print response
    println!("{}", response.content().unwrap_or(""));
    Ok(())
}
```

## Documentation

Comprehensive documentation is available in the [`docs/`](docs/) directory:

- **[Quick Start](docs/QUICKSTART.md)** - Get started in 5 minutes
- **[Usage Guide](docs/USAGE.md)** - Comprehensive usage documentation
- **[API Reference](docs/API.md)** - Complete API documentation
- **[Examples](docs/EXAMPLES.md)** - Code examples and patterns
- **[Architecture](docs/ARCHITECTURE.md)** - System design and decisions
- **[Development](docs/DEVELOPMENT.md)** - Contributing and development guide

## Architecture

SimpleAgents is organized into multiple crates:

```
SimpleAgents/
├── simple-agents-types/      # Core types and traits
├── simple-agents-providers/  # Provider implementations
└── simple-agents-cache/      # Caching strategies
```

### Three-Phase Provider Architecture

```rust
// Phase 1: Transform unified request to provider format
fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest>;

// Phase 2: Execute HTTP request
async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse>;

// Phase 3: Transform provider response to unified format
fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse>;
```

This separation enables:
- Clean separation of concerns
- Easy testing of each phase
- Provider-agnostic application code
- Simple addition of new providers

## Performance Optimizations

SimpleAgents includes numerous optimizations:

- **Zero-Copy Message Passing**: Borrows instead of cloning (saves MB per request)
- **Static String Allocation**: Uses `Cow<'static, str>` for headers (zero heap allocations)
- **Connection Pooling**: Reuses TCP and TLS sessions (saves ~300ms per request)
- **Smart Caching**: LRU eviction with TTL for optimal hit rates
- **Blake3 Hashing**: 10x faster than SHA-256 for cache keys

See [OPTIMISATION.md](OPTIMISATION.md) for details on all optimizations.

## Security

Security is a first-class concern:

- **Constant-Time Comparisons**: Prevents timing attacks on API keys
- **Secure RNG**: Cryptographically secure random number generation
- **Input Validation**: Automatic validation of all requests
- **No Secret Logging**: API keys never logged or serialized in plain text
- **Size Limits**: Prevents memory exhaustion attacks

## Testing

```bash
# Run all tests
cargo test --all

# Run tests for specific crate
cargo test -p simple-agents-types

# Run with output
cargo test -- --nocapture
```

**Current Status**: 132+ tests, all passing ✅

## Examples

### Caching Responses

```rust
use simple_agents_cache::InMemoryCache;
use std::time::Duration;

let cache = InMemoryCache::new(10 * 1024 * 1024, 1000);

// Generate cache key
let cache_key = CacheKey::from_parts("openai", "gpt-4", "question");

// Check cache
if let Some(cached) = cache.get(&cache_key).await? {
    return Ok(serde_json::from_slice(&cached)?);
}

// ... execute request ...

// Cache response
cache.set(&cache_key, response_bytes, Duration::from_secs(3600)).await?;
```

### Retry Logic

```rust
use simple_agents_providers::retry::execute_with_retry;
use simple_agents_types::config::RetryConfig;

let config = RetryConfig::default();

let response = execute_with_retry(
    &config,
    |e| e.is_retryable(),
    || provider.execute(provider_request.clone())
).await?;
```

### Multi-Turn Conversation

```rust
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::system("You are a helpful assistant."))
    .message(Message::user("Hello!"))
    .message(Message::assistant("Hi! How can I help?"))
    .message(Message::user("Tell me about Rust."))
    .build()?;
```

More examples in [docs/EXAMPLES.md](docs/EXAMPLES.md).

## Roadmap

### Implemented ✅
- [x] OpenAI provider
- [x] In-memory caching with LRU
- [x] Retry logic with exponential backoff
- [x] Connection pooling
- [x] Streaming support (framework)
- [x] Security optimizations
- [x] Performance optimizations

### Planned 🚧
- [ ] Anthropic provider (full implementation)
- [ ] Rate limiting
- [ ] Metrics and observability
- [ ] Redis cache backend
- [ ] Async validation
- [ ] Complete SSE streaming implementation

## Contributing

Contributions are welcome! Please see [DEVELOPMENT.md](docs/DEVELOPMENT.md) for:

- Project structure
- Building from source
- Running tests
- Code style guidelines
- Adding new providers
- Security considerations

## Performance

SimpleAgents is designed for production use with real-world performance in mind:

- **Latency**: ~50ms overhead (vs raw HTTP client)
- **Throughput**: Handles 1000s of concurrent requests
- **Memory**: Minimal allocations through zero-copy design
- **CPU**: ~5% overhead vs direct HTTP calls

See benchmarks in [OPTIMISATION.md](OPTIMISATION.md).

## License

MIT OR Apache-2.0

## Acknowledgments

Built with:
- [tokio](https://tokio.rs/) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [serde](https://serde.rs/) - Serialization
- [thiserror](https://github.com/dtolnay/thiserror) - Error handling
- [blake3](https://github.com/BLAKE3-team/BLAKE3) - Hashing
- [subtle](https://github.com/dalek-cryptography/subtle) - Constant-time operations

## Support

- **Documentation**: See [`docs/`](docs/)
- **Issues**: [GitHub Issues](https://github.com/yourusername/SimpleAgents/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/SimpleAgents/discussions)

## Status

**Current Version**: 0.1.0
**Status**: Beta - Production ready for evaluation
**Test Coverage**: 132+ tests
**Rust Version**: 1.75+
