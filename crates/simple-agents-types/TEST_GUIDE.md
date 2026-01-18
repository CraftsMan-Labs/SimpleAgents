# Testing Guide for simple-agents-types

This guide shows you how to test and explore the `simple-agents-types` crate.

## Quick Test Commands

### 1. Run All Unit Tests (83 tests)
```bash
cd crates/simple-agents-types
cargo test
```

Expected output: `test result: ok. 83 passed; 0 failed`

### 2. Run Integration Tests
```bash
cargo test --test integration_test
```

This runs comprehensive end-to-end tests showing how all types work together.

### 3. Run the Basic Usage Example
```bash
cargo run --example basic_usage
```

This demonstrates:
- Creating messages
- Building requests
- Working with responses
- Error handling
- API key security
- Coercion tracking
- Configuration

Expected output:
```
=== SimpleAgents Types Demo ===

📝 Creating Messages:
📤 Building Requests:
📨 Working with Responses:
⚠️  Error Handling:
🔐 API Key Security:
🔄 Coercion Tracking:
⚙️  Configuration:

=== All demos completed successfully! ===
```

### 4. Run the Mock Provider Example
```bash
cargo run --example mock_provider
```

This shows:
- Implementing the `Provider` trait
- Request transformation
- Response transformation
- Async execution

Expected output:
```
=== Mock Provider Demo ===

📦 Provider: mock-ai
⚙️  Capabilities:
📤 Original request:
🔄 Transforming request...
📡 Executing request...
🔄 Transforming response...
📥 Final response:

=== Demo completed successfully! ===
```

### 5. Check Code Quality
```bash
# Run clippy (linter)
cargo clippy -- -D warnings

# Check formatting
cargo fmt -- --check

# Build documentation
cargo doc --no-deps --open
```

## Interactive Testing

### 1. Explore the Documentation
```bash
cargo doc --no-deps --open
```

This opens the full documentation in your browser. You can:
- Browse all types and traits
- See example code
- Understand the API design

### 2. Rust Playground in Doctests
All the `///` documentation comments include executable examples. Run them with:
```bash
cargo test --doc
```

### 3. Try Your Own Code

Create a new file `examples/my_test.rs`:

```rust
use simple_agents_types::prelude::*;

fn main() -> Result<()> {
    // Create your own request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Your message here"))
        .temperature(0.9)
        .build()?;

    println!("{:#?}", request);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&request)?;
    println!("{}", json);

    Ok(())
}
```

Run it:
```bash
cargo run --example my_test
```

## What Each Test Covers

### Unit Tests (in `src/*.rs`)
- **message.rs**: 8 tests - Message creation, serialization
- **request.rs**: 8 tests - Builder pattern, validation
- **response.rs**: 8 tests - Response parsing, streaming
- **error.rs**: 4 tests - Error types, conversions
- **validation.rs**: 9 tests - API key security
- **coercion.rs**: 15 tests - Coercion tracking
- **config.rs**: 9 tests - Configuration types
- **provider.rs**: 5 tests - Provider trait, types
- **cache.rs**: 4 tests - Cache trait, key generation
- **router.rs**: 7 tests - Routing types, metrics
- **lib.rs**: 6 tests - Integration, Send/Sync

### Integration Tests (in `tests/integration_test.rs`)
- Complete request/response cycle
- Error handling flow
- Coercion tracking
- API key security
- Configuration validation
- Request validation
- Streaming types
- Provider request/response
- Router types
- Cache key generation
- Send/Sync verification

## Common Test Scenarios

### Test API Key Security
```bash
cargo test api_key
```

This runs all tests related to API key security, verifying that keys are never logged.

### Test Validation
```bash
cargo test validation
```

Tests all validation logic for requests, ensuring invalid data is rejected early.

### Test Serialization
```bash
cargo test serialization
```

Verifies that all types can be serialized to/from JSON correctly.

## Performance Testing

While this is a types-only crate (no runtime), you can benchmark serialization:

```bash
# Run tests in release mode
cargo test --release

# Show test execution time
cargo test -- --nocapture
```

## Continuous Testing

Watch mode (requires `cargo-watch`):
```bash
cargo install cargo-watch
cargo watch -x test
```

This re-runs tests whenever you change files.

## Troubleshooting

### Tests fail to compile
- Ensure Rust 1.75+ is installed: `rustc --version`
- Update dependencies: `cargo update`

### Examples don't run
- Make sure you're in the crate directory: `cd crates/simple-agents-types`
- Check that tokio is installed for async examples

### Documentation doesn't open
- Try: `cargo doc --no-deps` then manually open `target/doc/simple_agents_types/index.html`

## Next Steps

After testing this crate, you'll be ready to build:
1. `simple-agents-providers` - Actual OpenAI/Anthropic implementations
2. `simple-agents-healing` - JSON healing with these coercion types
3. `simple-agents-router` - Retry/fallback using these error types
4. `simple-agents-core` - Client using all these types

All these crates will depend on `simple-agents-types` as their foundation!
