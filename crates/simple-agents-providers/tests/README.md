# Integration Tests

This directory contains integration tests that verify provider implementations against real or local API servers.

## Running Tests

### Prerequisites

For the local proxy tests, you need:
- A local LLM proxy server running on `http://localhost:4000`
- API key: `sk-1234`
- Model: `openai/xai/grok-code-fast-1`

### Run All Integration Tests

```bash
# From project root
cargo test -p simple-agents-providers -- --ignored --nocapture

# Or from the providers crate directory
cd crates/simple-agents-providers
cargo test -- --ignored --nocapture
```

### Run Specific Tests

```bash
# Test basic connection
cargo test -p simple-agents-providers test_local_proxy_connection -- --ignored --nocapture

# Test multiple sequential requests
cargo test -p simple-agents-providers test_local_proxy_multiple_requests -- --ignored --nocapture

# Test error handling with invalid model
cargo test -p simple-agents-providers test_local_proxy_invalid_model -- --ignored --nocapture

# Test temperature variations
cargo test -p simple-agents-providers test_local_proxy_temperature_variations -- --ignored --nocapture

# Test conversation flow
cargo test -p simple-agents-providers test_local_proxy_conversation -- --ignored --nocapture
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
Model: openai/xai/grok-code-fast-1
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

**Solution**: Ensure your local proxy server is running on port 4000.

### Invalid API Key

```
Error: Provider error: Invalid API key
```

**Solution**: Verify your server accepts the API key `sk-1234`, or update the tests with your actual key.

### Model Not Found

```
Error: Provider error: Model not found: openai/xai/grok-code-fast-1
```

**Solution**: Check that your proxy server supports this model name, or update the tests with a valid model.

## Adding New Tests

When adding integration tests:

1. Mark them with `#[ignore]` to prevent running by default
2. Use `#[tokio::test]` for async tests
3. Add clear documentation about what the test verifies
4. Include helpful print statements with `--nocapture`
5. Test both success and error cases

Example:

```rust
#[tokio::test]
#[ignore] // Requires local server running
async fn test_my_feature() {
    let provider = setup_provider();
    // ... test code ...
    println!("✅ Test passed!");
}
```
