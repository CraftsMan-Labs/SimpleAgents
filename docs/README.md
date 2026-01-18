# SimpleAgents Documentation

Welcome to the SimpleAgents documentation! This guide will help you understand, use, and contribute to SimpleAgents.

## Table of Contents

- [Getting Started](#getting-started)
- [Documentation Structure](#documentation-structure)
- [Quick Links](#quick-links)

## Getting Started

SimpleAgents is a Rust framework for building LLM-powered applications with a focus on:
- **Type Safety**: Comprehensive compile-time guarantees
- **Performance**: Zero-copy operations, connection pooling, and efficient caching
- **Security**: Constant-time comparisons, secure RNG, and input validation
- **Extensibility**: Provider trait system for supporting multiple LLM APIs

### Quick Start

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a provider
    let api_key = ApiKey::new("sk-...")?;
    let provider = OpenAIProvider::new(api_key)?;

    // Build a request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello, world!"))
        .temperature(0.7)
        .build()?;

    // Execute the request
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    println!("{}", response.content().unwrap_or(""));
    Ok(())
}
```

## Documentation Structure

- **[USAGE.md](USAGE.md)** - Comprehensive usage guide with examples
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Developer guide for contributors
- **[API.md](API.md)** - API reference documentation
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - System architecture and design decisions
- **[EXAMPLES.md](EXAMPLES.md)** - Code examples and patterns

## Quick Links

### For Users
- [Installation](USAGE.md#installation)
- [Basic Usage](USAGE.md#basic-usage)
- [Provider Setup](USAGE.md#providers)
- [Caching](USAGE.md#caching)
- [Error Handling](USAGE.md#error-handling)

### For Developers
- [Project Structure](DEVELOPMENT.md#project-structure)
- [Building from Source](DEVELOPMENT.md#building)
- [Running Tests](DEVELOPMENT.md#testing)
- [Contributing](DEVELOPMENT.md#contributing)
- [Architecture Overview](ARCHITECTURE.md)

## Crate Overview

SimpleAgents is organized into multiple crates:

- **`simple-agents-types`** - Core types, traits, and interfaces
- **`simple-agents-providers`** - Provider implementations (OpenAI, Anthropic, etc.)
- **`simple-agents-cache`** - Caching implementations (in-memory, Redis, etc.)

## Features

- ✅ Type-safe request/response handling
- ✅ OpenAI API support
- ✅ Anthropic API support (stub)
- ✅ In-memory caching with LRU eviction
- ✅ Retry logic with exponential backoff
- ✅ Connection pooling and HTTP/2
- ✅ Streaming support
- ✅ Comprehensive error handling
- ✅ Security-focused design

## Support

- **Issues**: [GitHub Issues](https://github.com/yourusername/SimpleAgents/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/SimpleAgents/discussions)
- **Documentation**: You're reading it!

## License

MIT OR Apache-2.0
