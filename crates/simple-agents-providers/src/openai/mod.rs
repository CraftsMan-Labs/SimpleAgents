//! OpenAI provider implementation.
//!
//! This module provides integration with the OpenAI API, supporting:
//! - GPT-4, GPT-3.5-Turbo, and other OpenAI models
//! - Streaming responses via Server-Sent Events (SSE)
//! - Function calling and vision capabilities
//! - Comprehensive error handling and retry logic

mod error;
mod models;
pub mod streaming;

pub use error::OpenAIError;
pub use models::*;

use async_trait::async_trait;
use reqwest::Client;
use simple_agent_type::prelude::*;
use simple_agent_type::request::ResponseFormat;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::healing_integration::{HealingConfig, HealingIntegration};

/// OpenAI API provider
#[derive(Clone)]
pub struct OpenAIProvider {
    api_key: ApiKey,
    base_url: String,
    client: Client,
    rate_limiter: crate::rate_limit::MaybeRateLimiter,
    healing: Option<Arc<HealingIntegration>>,
    current_request: Arc<Mutex<Option<CompletionRequest>>>,
}

impl std::fmt::Debug for OpenAIProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIProvider")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl OpenAIProvider {
    /// Default OpenAI API base URL
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com/v1";

    /// Create a new OpenAI provider with default configuration
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key (starts with "sk-")
    ///
    /// # Errors
    ///
    /// Returns error if the HTTP client cannot be created
    pub fn new(api_key: ApiKey) -> Result<Self> {
        Self::with_base_url(api_key, Self::DEFAULT_BASE_URL.to_string())
    }

    /// Create a new OpenAI provider from environment variables.
    ///
    /// Required:
    /// - `OPENAI_API_KEY`
    ///
    ///   Optional:
    /// - `OPENAI_API_BASE`
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            SimpleAgentsError::Config("OPENAI_API_KEY environment variable is required".to_string())
        })?;
        let api_key = ApiKey::new(api_key)?;
        let base_url =
            std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| Self::DEFAULT_BASE_URL.to_string());
        let is_local = base_url.contains("localhost") || base_url.contains("127.0.0.1");

        let mut client_builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90));
        if is_local {
            client_builder = client_builder.no_proxy();
        } else {
            client_builder = client_builder.http2_prior_knowledge();
        }
        let client = client_builder.build().map_err(|e| {
            SimpleAgentsError::Config(format!("Failed to create HTTP client: {}", e))
        })?;

        Self::with_client(api_key, base_url, client)
    }

    /// Create a new OpenAI provider with custom base URL
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key
    /// * `base_url` - Custom base URL (e.g., for Azure OpenAI)
    ///
    /// # Connection Pooling
    ///
    /// The HTTP client uses connection pooling automatically:
    /// - **Pool size**: 10 idle connections per host (configurable)
    /// - **Keep-alive**: Connections are reused across requests
    /// - **HTTP/2**: Enabled by default for multiplexing
    /// - **Timeout**: 30 seconds per request
    ///
    /// This significantly improves performance by reusing TCP connections
    /// and TLS sessions across multiple API calls.
    ///
    /// # Note
    ///
    /// For local servers that only support HTTP/1.1 (e.g., vLLM, Ollama),
    /// use [`with_client`] to provide a custom HTTP client:
    /// ```rust
    /// use reqwest::Client;
    /// use simple_agents_providers::openai::OpenAIProvider;
    /// use simple_agent_type::prelude::*;
    /// use std::time::Duration;
    ///
    /// let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
    /// let base_url = "http://localhost:4000/v1".to_string();
    /// let client = Client::builder()
    ///     .timeout(Duration::from_secs(30))
    ///     .build()
    ///     .expect("Failed to build reqwest client");
    ///
    /// let provider = OpenAIProvider::with_client(api_key, base_url, client).unwrap();
    /// assert_eq!(provider.base_url(), "http://localhost:4000/v1");
    /// ```
    pub fn with_base_url(api_key: ApiKey, base_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10) // Connection pooling configuration
            .pool_idle_timeout(Duration::from_secs(90)) // Keep connections alive
            .http2_prior_knowledge() // Use HTTP/2 for multiplexing
            .build()
            .map_err(|e| {
                SimpleAgentsError::Config(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            api_key,
            base_url,
            client,
            rate_limiter: crate::rate_limit::MaybeRateLimiter::None,
            healing: None,
            current_request: Arc::new(Mutex::new(None)),
        })
    }

    /// Enable rate limiting with the given configuration.
    ///
    /// # Example
    /// ```
    /// use simple_agents_providers::openai::OpenAIProvider;
    /// use simple_agent_type::prelude::*;
    /// use simple_agent_type::config::RateLimitConfig;
    ///
    /// # fn example() -> Result<()> {
    /// let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")?;
    /// let provider = OpenAIProvider::new(api_key)?
    ///     .with_rate_limit(RateLimitConfig::new(50, 100));
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_rate_limit(mut self, config: simple_agent_type::config::RateLimitConfig) -> Self {
        self.rate_limiter = crate::rate_limit::MaybeRateLimiter::from_config(&config);
        self
    }

    /// Enable healing system for automatic recovery from malformed responses.
    ///
    /// When enabled, if native structured output parsing fails, the healing system
    /// will attempt to recover the response using tolerant parsing and type coercion.
    ///
    /// # Example
    /// ```
    /// use simple_agents_providers::openai::OpenAIProvider;
    /// use simple_agents_providers::healing_integration::HealingConfig;
    /// use simple_agent_type::prelude::*;
    ///
    /// # fn example() -> Result<()> {
    /// let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")?;
    /// let provider = OpenAIProvider::new(api_key)?
    ///     .with_healing(HealingConfig::default());
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_healing(mut self, config: HealingConfig) -> Self {
        self.healing = Some(Arc::new(HealingIntegration::new(config)));
        self
    }

    /// Create a new OpenAI provider with a custom HTTP client.
    ///
    /// This is useful for:
    /// - Local servers that only support HTTP/1.1 (e.g., vLLM, Ollama)
    /// - Custom proxy configurations
    /// - Testing with mock servers
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key
    /// * `base_url` - Base URL for API
    /// * `client` - Custom reqwest client
    ///
    /// # Example
    /// ```
    /// use simple_agents_providers::openai::OpenAIProvider;
    /// use simple_agent_type::prelude::*;
    /// use reqwest::Client;
    /// use std::time::Duration;
    ///
    /// # fn example() -> Result<()> {
    /// let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890")?;
    ///
    /// // Create client without HTTP/2 for local servers
    /// let client = Client::builder()
    ///     .timeout(Duration::from_secs(30))
    ///     .pool_max_idle_per_host(10)
    ///     .pool_idle_timeout(Duration::from_secs(90))
    ///     .build()
    ///     .expect("Failed to build reqwest client");
    ///
    /// let provider = OpenAIProvider::with_client(
    ///     api_key,
    ///     "http://localhost:4000/v1".to_string(),
    ///     client
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_client(api_key: ApiKey, base_url: String, client: Client) -> Result<Self> {
        Ok(Self {
            api_key,
            base_url,
            client,
            rate_limiter: crate::rate_limit::MaybeRateLimiter::None,
            healing: None,
            current_request: Arc::new(Mutex::new(None)),
        })
    }

    /// Get the base URL for this provider
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        // Store request context for potential healing
        if self.healing.is_some() && req.response_format.is_some() {
            if let Ok(mut current) = self.current_request.lock() {
                *current = Some(req.clone());
            }
        }

        // Build OpenAI-specific request (borrowing messages to avoid cloning)
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

        // Make HTTP request
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

        // Handle error responses with structured logging
        if !status.is_success() {
            // Capture headers for debugging (before consuming response)
            let headers_debug: Vec<(String, String)> = response
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_string(),
                        v.to_str().unwrap_or("<binary>").to_string(),
                    )
                })
                .collect();

            let error_body = match response.text().await {
                Ok(body) => {
                    tracing::warn!(
                        status = %status,
                        body_preview = %body.chars().take(200).collect::<String>(),
                        "API request failed"
                    );
                    body
                }
                Err(e) => {
                    tracing::error!(
                        status = %status,
                        error = %e,
                        "Failed to read error response body"
                    );
                    format!("HTTP {} - Could not read response body: {}", status, e)
                }
            };

            let openai_error = OpenAIError::from_response(status.as_u16(), &error_body);

            // Log additional context for debugging
            tracing::debug!(
                status = %status,
                headers = ?headers_debug,
                error_type = ?openai_error,
                "OpenAI API error details"
            );

            // Record error metrics
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
        // Try native parsing first (fast path)
        match serde_json::from_value::<OpenAICompletionResponse>(resp.body.clone()) {
            Ok(openai_response) => {
                // Native parsing succeeded - transform to unified format
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
            Err(parse_error) => {
                // Native parsing failed - try healing if enabled
                if self.healing.is_some() {
                    self.try_healing(&resp, parse_error)
                } else {
                    Err(SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                        format!("Failed to deserialize response: {}", parse_error),
                    )))
                }
            }
        }
    }
}

impl OpenAIProvider {
    /// Attempt to heal a malformed response using the healing system.
    fn try_healing(
        &self,
        resp: &ProviderResponse,
        original_error: serde_json::Error,
    ) -> Result<CompletionResponse> {
        let healing = self.healing.as_ref().unwrap();

        // Get the stored request context
        let request = self
            .current_request
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .ok_or_else(|| {
                SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                    "No request context available for healing".to_string(),
                ))
            })?;

        // Extract JSON schema from request
        let json_schema = match request.response_format.as_ref() {
            Some(ResponseFormat::JsonSchema { json_schema }) => json_schema,
            _ => {
                return Err(SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                    "No JSON schema available for healing".to_string(),
                )))
            }
        };

        // Extract the content from the response
        let content = resp.body["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                    "No content field in response".to_string(),
                ))
            })?;

        // Attempt healing
        let healed = healing.heal_response(
            content,
            &json_schema.schema,
            &format!("JSON parse error: {}", original_error),
        )?;

        // Construct response with healed content
        let healed_content = serde_json::to_string(&healed.value)?;

        Ok(CompletionResponse {
            id: resp.body["id"].as_str().unwrap_or("healed").to_string(),
            model: resp.body["model"].as_str().unwrap_or("unknown").to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant(healed_content),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage {
                prompt_tokens: resp.body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: resp.body["usage"]["completion_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
                total_tokens: resp.body["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            },
            created: resp.body["created"].as_i64(),
            provider: Some(self.name().to_string()),
            healing_metadata: Some(healed.metadata),
        })
    }

    #[allow(dead_code)]
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

        // Make HTTP request with streaming
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
                "Streaming API request failed"
            );

            let openai_error = OpenAIError::from_response(status.as_u16(), &error_body);
            return Err(SimpleAgentsError::Provider(openai_error.into()));
        }

        // Create SSE stream from response bytes
        let byte_stream = response.bytes_stream();
        let sse_stream = streaming::SseStream::new(byte_stream);

        Ok(Box::new(sse_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_provider_creation() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.base_url(), OpenAIProvider::DEFAULT_BASE_URL);
    }

    #[test]
    fn test_transform_request() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .temperature(0.7)
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();

        assert_eq!(
            provider_request.url,
            "https://api.openai.com/v1/chat/completions"
        );
        assert!(provider_request
            .headers
            .iter()
            .any(|(k, _)| k == "Authorization"));
        assert!(provider_request.body["model"] == "gpt-4");
    }

    #[test]
    fn test_transform_request_with_streaming() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .stream(true)
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();

        assert_eq!(provider_request.body["stream"], true);
    }

    #[tokio::test]
    async fn test_streaming_integration() {
        use crate::openai::streaming::SseStream;
        use bytes::Bytes;
        use futures_util::stream;
        use futures_util::StreamExt;

        let stream_body = concat!(
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"finish_reason\":null}]}\n",
            "\n",
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":123,\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"},\"finish_reason\":\"stop\"}]}\n",
            "\n",
            "data: [DONE]\n",
            "\n",
        );
        let byte_stream = stream::iter(vec![Ok(Bytes::from(stream_body))]);
        let mut stream = SseStream::new(byte_stream);

        let mut chunks_received = 0;
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => {
                    chunks_received += 1;
                    println!("Chunk {}: {:?}", chunks_received, chunk);
                }
                Err(e) => {
                    panic!("Stream error: {}", e);
                }
            }
        }

        assert!(chunks_received > 0, "Should receive at least one chunk");
    }
}
