# Code Examples

Practical examples for using SimpleAgents.

## Table of Contents

- [Basic Examples](#basic-examples)
- [Provider Examples](#provider-examples)
- [Caching Examples](#caching-examples)
- [Error Handling Examples](#error-handling-examples)
- [Advanced Examples](#advanced-examples)

## Basic Examples

### Hello World

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

### Simple Chat

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    // Build conversation
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("What is Rust?"))
        .temperature(0.7)
        .max_tokens(150)
        .build()?;

    // Get response
    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    println!("{}", response.content().unwrap_or(""));
    println!("Tokens: {}", response.usage.total_tokens);

    Ok(())
}
```

## Provider Examples

### OpenAI with Custom Configuration

```rust
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_types::prelude::*;

let api_key = ApiKey::new("sk-...")?;
let provider = OpenAIProvider::new(api_key)?;

let request = CompletionRequest::builder()
    .model("gpt-4-turbo-preview")
    .message(Message::user("Explain quantum computing"))
    .temperature(0.8)
    .max_tokens(500)
    .top_p(0.95)
    .frequency_penalty(0.3)
    .presence_penalty(0.3)
    .build()?;

let provider_request = provider.transform_request(&request)?;
let provider_response = provider.execute(provider_request).await?;
let response = provider.transform_response(provider_response)?;
```

### Azure OpenAI

```rust
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_types::prelude::*;

let api_key = ApiKey::new(std::env::var("AZURE_OPENAI_KEY")?)?;

// Use custom base URL for Azure
let provider = OpenAIProvider::with_base_url(
    api_key,
    "https://your-resource.openai.azure.com/openai/deployments/your-deployment".to_string()
)?;

let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Hello"))
    .build()?;

// Execute same as regular OpenAI
let provider_request = provider.transform_request(&request)?;
let provider_response = provider.execute(provider_request).await?;
let response = provider.transform_response(provider_response)?;
```

### Multi-Turn Conversation

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    // Track conversation history
    let mut messages = vec![
        Message::system("You are a helpful math tutor."),
    ];

    // First turn
    messages.push(Message::user("What is 2 + 2?"));

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .messages(messages.clone())
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    let assistant_reply = response.content().unwrap_or("").to_string();
    println!("Assistant: {}", assistant_reply);
    messages.push(Message::assistant(assistant_reply));

    // Second turn
    messages.push(Message::user("And what is 3 + 3?"));

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .messages(messages.clone())
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    println!("Assistant: {}", response.content().unwrap_or(""));

    Ok(())
}
```

## Caching Examples

### Basic Caching

```rust
use simple_agents_cache::InMemoryCache;
use simple_agents_types::cache::Cache;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Create cache
    let cache = InMemoryCache::new(
        10 * 1024 * 1024,  // 10MB max
        1000                // 1000 entries max
    );

    // Store data
    let key = "my_key";
    let data = b"Hello, cache!".to_vec();
    cache.set(key, data, Duration::from_secs(3600)).await?;

    // Retrieve data
    if let Some(cached) = cache.get(key).await? {
        let text = String::from_utf8(cached)?;
        println!("Cached: {}", text);
    }

    Ok(())
}
```

### Caching LLM Responses

```rust
use simple_agents_cache::InMemoryCache;
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;
use std::time::Duration;

async fn get_completion_cached(
    provider: &OpenAIProvider,
    cache: &InMemoryCache,
    request: &CompletionRequest,
) -> Result<CompletionResponse> {
    // Generate cache key
    let cache_key = simple_agents_types::cache::CacheKey::from_parts(
        "openai",
        &request.model,
        &serde_json::to_string(&request.messages)?
    );

    // Check cache
    if let Some(cached) = cache.get(&cache_key).await? {
        println!("Cache hit!");
        return Ok(serde_json::from_slice(&cached)?);
    }

    println!("Cache miss, calling API...");

    // Execute request
    let provider_request = provider.transform_request(request)?;
    let provider_response = provider.execute(provider_request).await?;
    let response = provider.transform_response(provider_response)?;

    // Cache response (1 hour TTL)
    let response_bytes = serde_json::to_vec(&response)?;
    cache.set(&cache_key, response_bytes, Duration::from_secs(3600)).await?;

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;
    let cache = InMemoryCache::new(10 * 1024 * 1024, 1000);

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("What is the capital of France?"))
        .build()?;

    // First call - miss
    let response1 = get_completion_cached(&provider, &cache, &request).await?;
    println!("{}", response1.content().unwrap_or(""));

    // Second call - hit
    let response2 = get_completion_cached(&provider, &cache, &request).await?;
    println!("{}", response2.content().unwrap_or(""));

    Ok(())
}
```

### Testing with NoOpCache

```rust
use simple_agents_cache::NoOpCache;
use simple_agents_types::cache::Cache;
use std::time::Duration;

#[tokio::test]
async fn test_without_cache() {
    let cache = NoOpCache;

    // Set does nothing
    cache.set("key", b"value".to_vec(), Duration::from_secs(60)).await.unwrap();

    // Get always returns None
    assert_eq!(cache.get("key").await.unwrap(), None);

    // is_enabled is false
    assert!(!cache.is_enabled());
}
```

## Error Handling Examples

### Basic Error Handling

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;

#[tokio::main]
async fn main() {
    let api_key = match ApiKey::new(std::env::var("OPENAI_API_KEY").unwrap_or_default()) {
        Ok(key) => key,
        Err(e) => {
            eprintln!("Invalid API key: {}", e);
            return;
        }
    };

    let provider = OpenAIProvider::new(api_key).unwrap();

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello"))
        .build()
        .unwrap();

    let provider_request = provider.transform_request(&request).unwrap();

    match provider.execute(provider_request).await {
        Ok(provider_response) => {
            match provider.transform_response(provider_response) {
                Ok(response) => {
                    println!("{}", response.content().unwrap_or(""));
                }
                Err(e) => {
                    eprintln!("Failed to parse response: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Request failed: {}", e);
        }
    }
}
```

### Detailed Error Handling

```rust
use simple_agents_types::error::{SimpleAgentsError, ProviderError, ValidationError};

match result {
    Ok(response) => {
        println!("Success: {}", response.content().unwrap_or(""));
    }
    Err(SimpleAgentsError::Validation(ValidationError::TooLong { field, max })) => {
        eprintln!("Input too long: {} exceeds {} bytes", field, max);
    }
    Err(SimpleAgentsError::Provider(ProviderError::RateLimit { retry_after, message })) => {
        eprintln!("Rate limited: {}", message);
        if let Some(duration) = retry_after {
            eprintln!("Retry after: {:?}", duration);
        }
    }
    Err(SimpleAgentsError::Provider(ProviderError::Authentication(msg))) => {
        eprintln!("Authentication failed: {}", msg);
        eprintln!("Please check your API key");
    }
    Err(SimpleAgentsError::Provider(ProviderError::Timeout(duration))) => {
        eprintln!("Request timed out after {:?}", duration);
    }
    Err(SimpleAgentsError::Network(msg)) => {
        eprintln!("Network error: {}", msg);
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

### Retry on Errors

```rust
use simple_agents_providers::retry::execute_with_retry;
use simple_agents_types::config::RetryConfig;
use simple_agents_types::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::user("Hello"))
        .build()?;

    let provider_request = provider.transform_request(&request)?;

    // Configure retry
    let retry_config = RetryConfig {
        max_attempts: 3,
        initial_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_secs(10),
        backoff_multiplier: 2.0,
        jitter: true,
    };

    // Execute with retry
    let provider_response = execute_with_retry(
        &retry_config,
        |e| {
            // Only retry if error is retryable
            matches!(e, SimpleAgentsError::Provider(pe) if pe.is_retryable())
        },
        || async {
            provider.execute(provider_request.clone()).await
        }
    ).await?;

    let response = provider.transform_response(provider_response)?;
    println!("{}", response.content().unwrap_or(""));

    Ok(())
}
```

## Advanced Examples

### Concurrent Requests

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = std::sync::Arc::new(OpenAIProvider::new(api_key)?);

    let questions = vec![
        "What is the capital of France?",
        "What is 2 + 2?",
        "Who wrote Romeo and Juliet?",
    ];

    let mut join_set = JoinSet::new();

    for question in questions {
        let provider = provider.clone();
        let question = question.to_string();

        join_set.spawn(async move {
            let request = CompletionRequest::builder()
                .model("gpt-4")
                .message(Message::user(question.clone()))
                .build()?;

            let provider_request = provider.transform_request(&request)?;
            let provider_response = provider.execute(provider_request).await?;
            let response = provider.transform_response(provider_response)?;

            Ok::<_, SimpleAgentsError>((question, response.content().unwrap_or("").to_string()))
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok((question, answer))) => {
                println!("Q: {}", question);
                println!("A: {}\n", answer);
            }
            Ok(Err(e)) => eprintln!("Error: {}", e),
            Err(e) => eprintln!("Task error: {}", e),
        }
    }

    Ok(())
}
```

### Custom Provider Wrapper

```rust
use async_trait::async_trait;
use simple_agents_types::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

// Track statistics
struct ProviderWithStats<P: Provider> {
    inner: P,
    stats: Arc<Mutex<Stats>>,
}

struct Stats {
    total_requests: u64,
    total_tokens: u64,
    errors: u64,
}

impl<P: Provider> ProviderWithStats<P> {
    fn new(provider: P) -> Self {
        Self {
            inner: provider,
            stats: Arc::new(Mutex::new(Stats {
                total_requests: 0,
                total_tokens: 0,
                errors: 0,
            })),
        }
    }

    async fn get_stats(&self) -> Stats {
        self.stats.lock().await.clone()
    }
}

#[async_trait]
impl<P: Provider + Send + Sync> Provider for ProviderWithStats<P> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        self.inner.transform_request(req)
    }

    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
        let mut stats = self.stats.lock().await;
        stats.total_requests += 1;
        drop(stats);

        match self.inner.execute(req).await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let mut stats = self.stats.lock().await;
                stats.errors += 1;
                Err(e)
            }
        }
    }

    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
        let response = self.inner.transform_response(resp)?;

        // Track tokens
        let mut stats = self.stats.lock().await;
        stats.total_tokens += response.usage.total_tokens as u64;

        Ok(response)
    }
}
```

### Building a Simple CLI

```rust
use simple_agents_types::prelude::*;
use simple_agents_providers::openai::OpenAIProvider;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let provider = OpenAIProvider::new(api_key)?;

    let mut messages = vec![
        Message::system("You are a helpful assistant."),
    ];

    loop {
        print!("You: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() || input == "exit" {
            break;
        }

        messages.push(Message::user(input));

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .messages(messages.clone())
            .temperature(0.7)
            .build()?;

        let provider_request = provider.transform_request(&request)?;

        print!("Assistant: ");
        io::stdout().flush()?;

        match provider.execute(provider_request).await {
            Ok(provider_response) => {
                match provider.transform_response(provider_response) {
                    Ok(response) => {
                        let content = response.content().unwrap_or("");
                        println!("{}\n", content);
                        messages.push(Message::assistant(content));
                    }
                    Err(e) => {
                        eprintln!("Error: {}\n", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}\n", e);
            }
        }
    }

    Ok(())
}
```

### Complete Application with All Features

```rust
use simple_agents_cache::InMemoryCache;
use simple_agents_providers::{openai::OpenAIProvider, retry::execute_with_retry};
use simple_agents_types::{
    cache::{Cache, CacheKey},
    config::RetryConfig,
    prelude::*,
};
use std::time::Duration;

struct LLMClient {
    provider: OpenAIProvider,
    cache: InMemoryCache,
    retry_config: RetryConfig,
}

impl LLMClient {
    fn new(api_key: ApiKey) -> Result<Self> {
        Ok(Self {
            provider: OpenAIProvider::new(api_key)?,
            cache: InMemoryCache::new(50 * 1024 * 1024, 10000),
            retry_config: RetryConfig::default(),
        })
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        // Generate cache key
        let cache_key = CacheKey::from_parts(
            "openai",
            &request.model,
            &serde_json::to_string(&request.messages)?,
        );

        // Check cache
        if let Some(cached) = self.cache.get(&cache_key).await? {
            tracing::info!("Cache hit");
            return Ok(serde_json::from_slice(&cached)?);
        }

        tracing::info!("Cache miss");

        // Transform request
        let provider_request = self.provider.transform_request(request)?;

        // Execute with retry
        let provider_response = execute_with_retry(
            &self.retry_config,
            |e| matches!(e, SimpleAgentsError::Provider(pe) if pe.is_retryable()),
            || async {
                self.provider.execute(provider_request.clone()).await
            },
        )
        .await?;

        // Transform response
        let response = self.provider.transform_response(provider_response)?;

        // Cache response
        let response_bytes = serde_json::to_vec(&response)?;
        self.cache
            .set(&cache_key, response_bytes, Duration::from_secs(3600))
            .await?;

        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup tracing
    tracing_subscriber::fmt::init();

    // Create client
    let api_key = ApiKey::new(std::env::var("OPENAI_API_KEY")?)?;
    let client = LLMClient::new(api_key)?;

    // Make request
    let request = CompletionRequest::builder()
        .model("gpt-4")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("Explain Rust in one sentence."))
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    let response = client.complete(&request).await?;

    println!("{}", response.content().unwrap_or(""));
    println!("Tokens: {}", response.usage.total_tokens);

    Ok(())
}
```

## Best Practices

1. **Always validate API keys**:
   ```rust
   let api_key = ApiKey::new(env::var("API_KEY")?)?;
   ```

2. **Use caching for expensive requests**:
   ```rust
   let cache_key = CacheKey::from_parts(provider, model, content);
   if let Some(cached) = cache.get(&cache_key).await? { ... }
   ```

3. **Implement retry logic for production**:
   ```rust
   execute_with_retry(&config, |e| e.is_retryable(), operation).await?;
   ```

4. **Handle errors gracefully**:
   ```rust
   match result {
       Ok(response) => { /* use response */ }
       Err(e) if e.is_retryable() => { /* retry */ }
       Err(e) => { /* log and fail */ }
   }
   ```

5. **Monitor token usage**:
   ```rust
   println!("Tokens used: {}", response.usage.total_tokens);
   ```

6. **Set appropriate limits**:
   ```rust
   .max_tokens(500)
   .temperature(0.7)
   ```
