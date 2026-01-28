# simple-agents-providers

**Provider implementations for SimpleAgents**

This crate provides concrete implementations of LLM providers that integrate with the SimpleAgents framework.

## Supported Providers

- **OpenAI**: GPT-4, GPT-3.5-Turbo, and other OpenAI models
- **Anthropic**: Claude 3 Opus, Sonnet, and Haiku

## Features

- Request/response transformation for each provider
- Streaming support with Server-Sent Events (SSE)
- Provider-specific error mapping
- Retry configuration per provider
- Provider capability reporting

## Usage

```rust
use simple_agents_providers::openai::OpenAIProvider;
use simple_agent_type::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = ApiKey::new("sk-...")?;
    let provider = OpenAIProvider::new(api_key)?;

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello!"))
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    println!("{}", response.content().unwrap_or(""));
    Ok(())
}
```

## Architecture

Each provider implements the `Provider` trait from `simple-agent-type`:

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest>;
    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse>;
    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse>;
}
```

## Testing

### Unit Tests

Run all unit tests:
```bash
cargo test -p simple-agents-providers
```

### Integration Tests

Integration tests verify provider implementations against real or local API servers. They run by default and require a configured `.env`.

**Quick Start:**
```bash
# Using the provided script (loads CUSTOM_API_* from project .env)
cd crates/simple-agents-providers
./run_integration_tests.sh

# Or run manually
cargo test -p simple-agents-providers --nocapture
```

**Configuration for local proxy:**
- `CUSTOM_API_BASE` (e.g. `http://localhost:4000`)
- `CUSTOM_API_KEY`
- `CUSTOM_API_MODEL`

**Available integration tests:**
- `test_local_proxy_connection` - Basic connectivity test
- `test_local_proxy_multiple_requests` - Sequential request stability
- `test_local_proxy_invalid_model` - Error handling verification
- `test_local_proxy_temperature_variations` - Parameter testing
- `test_local_proxy_conversation` - Multi-turn conversation

See `tests/README.md` for detailed documentation.

### Real API Tests (Optional)

For testing against real OpenAI/Anthropic APIs:
```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
cargo test -p simple-agents-providers -- --ignored
```

## License

MIT OR Apache-2.0
