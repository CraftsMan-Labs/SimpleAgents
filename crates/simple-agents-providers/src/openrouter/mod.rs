//! OpenRouter provider implementation.
//!
//! OpenRouter provides unified access to multiple open-source and commercial LLM models
//! through a single OpenAI-compatible API. This provider inherits most functionality
//! from the OpenAI provider but uses a different base URL and supports model prefixes.
//!
//! # Model Prefixes
//!
//! OpenRouter uses model prefixes to route to different providers:
//! - `openai/gpt-4` → Routes to OpenAI
//! - `anthropic/claude-3-opus` → Routes to Anthropic
//! - `meta-llama/llama-2-70b-chat` → Routes to Llama
//!
//! # Examples
//!
//! ```no_run
//! use simple_agents_providers::openrouter::OpenRouterProvider;
//! use simple_agents_providers::Provider;
//! use simple_agent_type::prelude::*;
//!
//! # async fn example() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let api_key = ApiKey::new(std::env::var("OPENROUTER_API_KEY")?)?;
//! let provider = OpenRouterProvider::new(api_key)?;
//!
//! let request = CompletionRequest::builder()
//!     .model("openai/gpt-4")  // Model with prefix
//!     .message(Message::user("Hello!"))
//!     .build()?;
//!
//! let provider_request = provider.transform_request(&request)?;
//! let provider_response = provider.execute(provider_request).await?;
//! let response = provider.transform_response(provider_response)?;
//!
//! println!("{}", response.content().unwrap_or(""));
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use reqwest::Client;
use simple_agent_type::prelude::*;
use std::time::Duration;

use crate::openai::{
    normalize_openai_strict_json_schema, OpenAICompletionRequest, OpenAICompletionResponse,
    OpenAIStreamOptions,
};
use crate::utils::DEFAULT_TIMEOUT;

/// OpenRouter API provider.
///
/// OpenRouter is OpenAI-compatible with additional features like model routing
/// and unified access to multiple providers.
#[derive(Clone)]
pub struct OpenRouterProvider {
    api_key: ApiKey,
    base_url: String,
    client: Client,
    rate_limiter: crate::rate_limit::MaybeRateLimiter,
}

impl std::fmt::Debug for OpenRouterProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenRouterProvider")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl OpenRouterProvider {
    /// Default OpenRouter API base URL
    pub const DEFAULT_BASE_URL: &'static str = "https://openrouter.ai/api/v1";

    /// Create a new OpenRouter provider with default configuration.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenRouter API key
    ///
    /// # Errors
    ///
    /// Returns error if the HTTP client cannot be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use simple_agents_providers::openrouter::OpenRouterProvider;
    /// use simple_agent_type::prelude::*;
    ///
    /// # fn example() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// let api_key = ApiKey::new("sk-or-test1234567890123456789012345678901234567890")?;
    /// let provider = OpenRouterProvider::new(api_key)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(api_key: ApiKey) -> Result<Self> {
        Self::with_base_url(api_key, Self::DEFAULT_BASE_URL.to_string())
    }

    /// Create a new OpenRouter provider from environment variables.
    ///
    /// Required:
    /// - `OPENROUTER_API_KEY`
    ///
    ///   Optional:
    /// - `OPENROUTER_API_BASE`
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
            SimpleAgentsError::Config(
                "OPENROUTER_API_KEY environment variable is required".to_string(),
            )
        })?;
        let api_key = ApiKey::new(api_key)?;
        let base_url = std::env::var("OPENROUTER_API_BASE")
            .unwrap_or_else(|_| Self::DEFAULT_BASE_URL.to_string());

        Self::with_base_url(api_key, base_url)
    }

    /// Create a new OpenRouter provider with custom base URL.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenRouter API key
    /// * `base_url` - Custom base URL
    pub fn with_base_url(api_key: ApiKey, base_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| {
                SimpleAgentsError::Config(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            api_key,
            base_url,
            client,
            rate_limiter: crate::rate_limit::MaybeRateLimiter::None,
        })
    }

    /// Create a new OpenRouter provider with a custom HTTP client.
    pub fn with_client(api_key: ApiKey, base_url: String, client: Client) -> Result<Self> {
        Ok(Self {
            api_key,
            base_url,
            client,
            rate_limiter: crate::rate_limit::MaybeRateLimiter::None,
        })
    }

    /// Enable rate limiting with the given configuration.
    pub fn with_rate_limit(mut self, config: simple_agent_type::config::RateLimitConfig) -> Self {
        self.rate_limiter = crate::rate_limit::MaybeRateLimiter::from_config(&config);
        self
    }

    /// Get the base URL for this provider.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl Provider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        // OpenRouter uses the same format as OpenAI
        // Model prefixes are passed through directly (e.g., "openai/gpt-4")
        let openai_request = OpenAICompletionRequest {
            model: &req.model,
            messages: &req.messages,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            top_p: req.top_p,
            n: req.n,
            stream: req.stream,
            stream_options: req.stream.and_then(|streaming| {
                if streaming {
                    Some(OpenAIStreamOptions {
                        include_usage: true,
                    })
                } else {
                    None
                }
            }),
            stop: req.stop.as_ref(),
            response_format: req.response_format.as_ref(),
            tools: req.tools.as_ref(),
            tool_choice: req.tool_choice.as_ref(),
        };

        let mut body = serde_json::to_value(&openai_request)?;
        normalize_openai_strict_json_schema(&mut body);

        Ok(ProviderRequest {
            url: format!("{}/chat/completions", self.base_url),
            headers: vec![
                (
                    std::borrow::Cow::Borrowed(simple_agent_type::provider::headers::AUTHORIZATION),
                    std::borrow::Cow::Owned(format!("Bearer {}", self.api_key.expose())),
                ),
                (
                    std::borrow::Cow::Borrowed(simple_agent_type::provider::headers::CONTENT_TYPE),
                    std::borrow::Cow::Borrowed("application/json"),
                ),
            ],
            body,
            timeout: None,
        })
    }

    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
        // Apply rate limiting
        self.rate_limiter
            .until_ready(Some(self.api_key.expose()))
            .await;

        // Extract model for metrics
        let model = req.body["model"].as_str().unwrap_or("unknown");

        // Start metrics timer
        let timer = crate::metrics::RequestTimer::start(self.name(), model);

        // Build headers
        let headers = crate::utils::build_headers(req.headers)
            .map_err(|e| SimpleAgentsError::Config(format!("Invalid headers: {}", e)))?;

        // Make HTTP request (same as OpenAI)
        let response = crate::utils::send_json_request(&self.client, &req.url, headers, &req.body)
            .await
            .map_err(|e| crate::utils::map_transport_error_with_timer(e, timer.clone()))?;

        let status = response.status();
        let retry_after = crate::utils::retry_after_from_headers(response.headers());

        // Handle error responses
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|e| {
                format!("HTTP {} - Could not read response body: {}", status, e)
            });

            tracing::warn!(
                status = %status,
                body_preview = %error_body.chars().take(200).collect::<String>(),
                "OpenRouter API request failed"
            );

            // Use OpenAI error parsing (compatible format)
            let openai_error = crate::openai::OpenAIError::from_response(
                status.as_u16(),
                &error_body,
                retry_after,
            );
            timer.complete_error(format!("http_{}", status.as_u16()));

            return Err(SimpleAgentsError::Provider(openai_error.into()));
        }

        // Parse successful response
        let body = match response.json::<serde_json::Value>().await {
            Ok(b) => b,
            Err(e) => {
                timer.complete_error("parse_error");
                return Err(SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                    format!("Failed to parse JSON response: {}", e),
                )));
            }
        };

        // Extract token usage for metrics
        let prompt_tokens = safe_token_count(
            body["usage"]["prompt_tokens"].as_u64(),
            "usage.prompt_tokens",
        );
        let completion_tokens = safe_token_count(
            body["usage"]["completion_tokens"].as_u64(),
            "usage.completion_tokens",
        );

        // Record success metrics
        timer.complete_success(prompt_tokens, completion_tokens);

        Ok(ProviderResponse {
            status: status.as_u16(),
            body,
            headers: None,
        })
    }

    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
        // OpenRouter uses the same response format as OpenAI
        let openai_response: OpenAICompletionResponse =
            serde_json::from_value(resp.body).map_err(|e| {
                SimpleAgentsError::Provider(ProviderError::InvalidResponse(format!(
                    "Failed to deserialize response: {}",
                    e
                )))
            })?;

        // Transform to unified format (same as OpenAI)
        let choices: Vec<CompletionChoice> = openai_response
            .choices
            .iter()
            .map(|choice| CompletionChoice {
                index: choice.index,
                message: choice.message.clone(),
                finish_reason: choice
                    .finish_reason
                    .as_ref()
                    .map(|s: &String| match s.as_str() {
                        "stop" => FinishReason::Stop,
                        "length" => FinishReason::Length,
                        "content_filter" => FinishReason::ContentFilter,
                        "tool_calls" => FinishReason::ToolCalls,
                        _ => FinishReason::Stop,
                    })
                    .unwrap_or(FinishReason::Stop),
                logprobs: None,
            })
            .collect();

        Ok(CompletionResponse {
            id: openai_response.id,
            model: openai_response.model,
            choices,
            usage: Usage {
                prompt_tokens: openai_response.usage.prompt_tokens,
                completion_tokens: openai_response.usage.completion_tokens,
                total_tokens: openai_response.usage.total_tokens,
                reasoning_tokens: openai_response.usage.reasoning_tokens(),
            },
            created: Some(openai_response.created as i64),
            provider: Some(self.name().to_string()),
            healing_metadata: None,
        })
    }

    async fn execute_stream(
        &self,
        req: ProviderRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        // Apply rate limiting
        self.rate_limiter
            .until_ready(Some(self.api_key.expose()))
            .await;

        // Build headers
        let headers = crate::utils::build_headers(req.headers)
            .map_err(|e| SimpleAgentsError::Config(format!("Invalid headers: {}", e)))?;

        // Make HTTP request with streaming (same as OpenAI)
        let response = crate::utils::send_json_request(&self.client, &req.url, headers, &req.body)
            .await
            .map_err(crate::utils::map_transport_error)?;

        let status = response.status();
        let retry_after = crate::utils::retry_after_from_headers(response.headers());

        // Handle error responses
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|e| {
                format!("HTTP {} - Could not read response body: {}", status, e)
            });

            tracing::warn!(
                status = %status,
                body_preview = %error_body.chars().take(200).collect::<String>(),
                "OpenRouter streaming request failed"
            );

            let openai_error = crate::openai::OpenAIError::from_response(
                status.as_u16(),
                &error_body,
                retry_after,
            );
            return Err(SimpleAgentsError::Provider(openai_error.into()));
        }

        // Create SSE stream from response bytes (same as OpenAI)
        let byte_stream = response.bytes_stream();
        let sse_stream = crate::openai::streaming::SseStream::new(byte_stream);

        Ok(Box::new(sse_stream))
    }
}

fn safe_token_count(raw: Option<u64>, field: &str) -> u32 {
    let raw = raw.unwrap_or(0);
    match u32::try_from(raw) {
        Ok(value) => value,
        Err(_) => {
            tracing::warn!(
                field = field,
                raw = raw,
                "Token count exceeded u32::MAX; clamping value"
            );
            u32::MAX
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    async fn spawn_hanging_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr should resolve");

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n").await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        format!("http://{}", addr)
    }

    async fn spawn_error_server(
        status_line: &str,
        retry_after: Option<&str>,
        body: &str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr should resolve");
        let response = if let Some(retry_after) = retry_after {
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\nretry-after: {retry_after}\r\ncontent-length: {len}\r\n\r\n{body}",
                status = status_line,
                retry_after = retry_after,
                len = body.len(),
                body = body
            )
        } else {
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {len}\r\n\r\n{body}",
                status = status_line,
                len = body.len(),
                body = body
            )
        };

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        format!("http://{}", addr)
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_provider_creation() {
        let api_key = ApiKey::new("sk-or-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenRouterProvider::new(api_key).unwrap();
        assert_eq!(provider.name(), "openrouter");
        assert_eq!(provider.base_url(), OpenRouterProvider::DEFAULT_BASE_URL);
    }

    #[test]
    fn test_transform_request_with_model_prefix() {
        let api_key = ApiKey::new("sk-or-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenRouterProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("openai/gpt-4") // Model with prefix
            .message(Message::user("Hello"))
            .temperature(0.7)
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();

        assert_eq!(
            provider_request.url,
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert!(provider_request
            .headers
            .iter()
            .any(|(k, _)| k == "Authorization"));
        assert_eq!(provider_request.body["model"], "openai/gpt-4");
    }

    #[test]
    fn test_transform_request_anthropic_model() {
        let api_key = ApiKey::new("sk-or-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenRouterProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("anthropic/claude-3-opus")
            .message(Message::user("Hello"))
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();

        assert_eq!(provider_request.body["model"], "anthropic/claude-3-opus");
    }

    #[tokio::test]
    async fn test_integration() {
        let response_body = serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 123,
            "model": "openai/gpt-4",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "test" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
        });

        let api_key = ApiKey::new("sk-or-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenRouterProvider::new(api_key).unwrap();

        let _request = CompletionRequest::builder()
            .model("openai/gpt-4")
            .message(Message::user("Say 'test'"))
            .max_tokens(10)
            .build()
            .unwrap();

        let provider_response = ProviderResponse::new(200, response_body);
        let response = provider.transform_response(provider_response).unwrap();

        assert!(!response.choices.is_empty());
    }

    #[tokio::test]
    async fn test_execute_timeout_maps_to_default_timeout_constant() {
        let base_url = spawn_hanging_server().await;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(30))
            .build()
            .expect("client should build");
        let api_key = ApiKey::new("sk-or-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenRouterProvider::with_client(api_key, base_url, client).unwrap();

        let request = CompletionRequest::builder()
            .model("openai/gpt-4")
            .message(Message::user("Hello"))
            .build()
            .unwrap();
        let provider_request = provider.transform_request(&request).unwrap();

        let result = provider.execute(provider_request).await;
        assert!(matches!(
            result,
            Err(SimpleAgentsError::Provider(ProviderError::Timeout(d))) if d == crate::utils::DEFAULT_TIMEOUT
        ));
    }

    #[tokio::test]
    async fn test_execute_stream_non_success_maps_retry_after() {
        let base_url = spawn_error_server(
            "429 Too Many Requests",
            Some("4"),
            r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit"}}"#,
        )
        .await;
        let api_key = ApiKey::new("sk-or-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenRouterProvider::with_base_url(api_key, base_url).unwrap();

        let request = CompletionRequest::builder()
            .model("openai/gpt-4")
            .message(Message::user("Hello"))
            .stream(true)
            .build()
            .unwrap();
        let provider_request = provider.transform_request(&request).unwrap();

        let result = provider.execute_stream(provider_request).await;
        assert!(matches!(
            result,
            Err(SimpleAgentsError::Provider(ProviderError::RateLimit { retry_after: Some(d) })) if d == Duration::from_secs(4)
        ));
    }

    #[test]
    fn test_from_env_requires_api_key() {
        let _guard = env_lock().lock().expect("env lock should not be poisoned");
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("OPENROUTER_API_BASE");

        let result = OpenRouterProvider::from_env();
        assert!(matches!(result, Err(SimpleAgentsError::Config(_))));
    }
}
