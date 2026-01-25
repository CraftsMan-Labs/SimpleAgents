# Integration Tests

This directory contains integration tests that verify provider implementations against real or local API servers.

## Running Tests

### Prerequisites

For the integration tests, you need a `.env` in the project root with:
- `CUSTOM_API_BASE` (e.g. `http://localhost:4000`)
- `CUSTOM_API_KEY`
- `CUSTOM_API_MODEL`

### Run All Integration Tests

```bash
# From project root
cargo test -p simple-agents-providers --nocapture

# Or from the providers crate directory
cd crates/simple-agents-providers
cargo test --nocapture
```

### Run Specific Tests

```bash
# Test basic connection
cargo test -p simple-agents-providers test_local_proxy_connection -- --nocapture

# Test multiple sequential requests
cargo test -p simple-agents-providers test_local_proxy_multiple_requests -- --nocapture

# Test error handling with invalid model
cargo test -p simple-agents-providers test_local_proxy_invalid_model -- --nocapture

# Test temperature variations
cargo test -p simple-agents-providers test_local_proxy_temperature_variations -- --nocapture

# Test conversation flow
cargo test -p simple-agents-providers test_local_proxy_conversation -- --nocapture
```

## Test Coverage

### `test_local_proxy_connection`
Basic connectivity test that:
- Creates a provider with custom base URL
- Makes a simple completion request
- Verifies response structure
- Checks token usage statistics

### `test_local_proxy_multiple_requests`
Verifies connection stability with sequential requests

### `test_local_proxy_invalid_model`
Tests error handling when using an invalid model name

### `test_local_proxy_temperature_variations`
Tests different temperature settings (0.0, 0.5, 1.0)

### `test_local_proxy_conversation`
Tests multi-turn conversation with system, user, and assistant messages

## Expected Output

When tests pass, you'll see output like:

```
Making request to: http://localhost:4000/chat/completions
Model: your/model
Response status: 200
Response content: Hello from SimpleAgents!
✅ Integration test passed!
   Prompt tokens: 12
   Completion tokens: 8
   Total tokens: 20
```

## Troubleshooting

### Connection Refused

```
Error: Network error: error sending request for url (http://localhost:4000/...): error trying to connect: tcp connect error: Connection refused
```

**Solution**: Ensure your API server is running at `CUSTOM_API_BASE`.

### Invalid API Key

```
Error: Provider error: Invalid API key
```

**Solution**: Verify your server accepts the API key from `CUSTOM_API_KEY`.

### Model Not Found

```
Error: Provider error: Model not found: your/model
```

**Solution**: Check that your proxy server supports the model name from `CUSTOM_API_MODEL`.

## Adding New Tests

When adding integration tests:

1. Use `#[tokio::test]` for async tests
2. Add clear documentation about what the test verifies
3. Include helpful print statements with `--nocapture`
4. Test both success and error cases

Example:

```rust
#[tokio::test]
async fn test_my_feature() {
    let provider = setup_provider();
    // ... test code ...
    println!("✅ Test passed!");
}
```
