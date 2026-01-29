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

use crate::openai::{OpenAICompletionRequest, OpenAICompletionResponse};

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
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .http2_prior_knowledge()
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
            stop: req.stop.as_ref(),
            response_format: req.response_format.as_ref(),
            tools: req.tools.as_ref(),
            tool_choice: req.tool_choice.as_ref(),
        };

        let body = serde_json::to_value(&openai_request)?;

        Ok(ProviderRequest {
            url: format!("{}/chat/completions", self.base_url),
            headers: vec![
                (
                    std::borrow::Cow::Borrowed(
                        simple_agent_type::provider::headers::AUTHORIZATION,
                    ),
                    std::borrow::Cow::Owned(format!("Bearer {}", self.api_key.expose())),
                ),
                (
                    std::borrow::Cow::Borrowed(
                        simple_agent_type::provider::headers::CONTENT_TYPE,
                    ),
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
        let response = match self
            .client
            .post(&req.url)
            .headers(headers)
            .json(&req.body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    timer.complete_timeout();
                    return Err(SimpleAgentsError::Provider(ProviderError::Timeout(
                        Duration::from_secs(30),
                    )));
                } else {
                    timer.complete_error("network");
                    return Err(SimpleAgentsError::Network(format!("Network error: {}", e)));
                }
            }
        };

        let status = response.status();

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
            let openai_error =
                crate::openai::OpenAIError::from_response(status.as_u16(), &error_body);
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
        let prompt_tokens = body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = body["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;

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
        let response = self
            .client
            .post(&req.url)
            .headers(headers)
            .json(&req.body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    SimpleAgentsError::Provider(ProviderError::Timeout(Duration::from_secs(30)))
                } else {
                    SimpleAgentsError::Network(format!("Network error: {}", e))
                }
            })?;

        let status = response.status();

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

            let openai_error =
                crate::openai::OpenAIError::from_response(status.as_u16(), &error_body);
            return Err(SimpleAgentsError::Provider(openai_error.into()));
        }

        // Create SSE stream from response bytes (same as OpenAI)
        let byte_stream = response.bytes_stream();
        let sse_stream = crate::openai::streaming::SseStream::new(byte_stream);

        Ok(Box::new(sse_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
