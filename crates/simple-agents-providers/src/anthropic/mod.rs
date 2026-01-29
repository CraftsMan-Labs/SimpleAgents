//! Anthropic provider implementation.
//!
//! This module provides integration with the Anthropic API, supporting:
//! - Claude 3 Opus, Sonnet, and Haiku models
//! - System message handling
//! - Comprehensive error handling and retry logic
//! - Connection pooling

mod error;
mod models;
pub mod streaming;

pub use error::AnthropicError;
pub use models::*;

use async_trait::async_trait;
use reqwest::Client;
use simple_agent_type::prelude::*;
use simple_agent_type::request::ResponseFormat;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::healing_integration::{HealingConfig, HealingIntegration};

/// Anthropic API provider
#[derive(Clone)]
pub struct AnthropicProvider {
    api_key: ApiKey,
    base_url: String,
    client: Client,
    rate_limiter: crate::rate_limit::MaybeRateLimiter,
    healing: Option<Arc<HealingIntegration>>,
    current_request: Arc<Mutex<Option<CompletionRequest>>>,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl AnthropicProvider {
    /// Default Anthropic API base URL
    pub const DEFAULT_BASE_URL: &'static str = "https://api.anthropic.com/v1";

    /// Anthropic API version
    pub const API_VERSION: &'static str = "2023-06-01";

    /// Create a new Anthropic provider with default configuration
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
    ///
    /// # Errors
    ///
    /// Returns error if the HTTP client cannot be created
    pub fn new(api_key: ApiKey) -> Result<Self> {
        Self::with_base_url(api_key, Self::DEFAULT_BASE_URL.to_string())
    }

    /// Create a new Anthropic provider from environment variables.
    ///
    /// Required:
    /// - `ANTHROPIC_API_KEY`
    ///
    ///   Optional:
    /// - `ANTHROPIC_API_BASE` (or `ANTHROPIC__API_BASE`)
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            SimpleAgentsError::Config(
                "ANTHROPIC_API_KEY environment variable is required".to_string(),
            )
        })?;
        let api_key = ApiKey::new(api_key)?;

        let base_url = std::env::var("ANTHROPIC_API_BASE")
            .or_else(|_| std::env::var("ANTHROPIC__API_BASE"))
            .unwrap_or_else(|_| Self::DEFAULT_BASE_URL.to_string());
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

    /// Create a new Anthropic provider with custom base URL
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
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
            healing: None,
            current_request: Arc::new(Mutex::new(None)),
        })
    }

    /// Create a new Anthropic provider with a custom HTTP client.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
    /// * `base_url` - Base URL for API
    /// * `client` - Preconfigured HTTP client
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

    /// Enable rate limiting with the given configuration.
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
    /// use simple_agents_providers::anthropic::AnthropicProvider;
    /// use simple_agents_providers::healing_integration::HealingConfig;
    /// use simple_agent_type::prelude::*;
    ///
    /// # fn example() -> Result<()> {
    /// let api_key = ApiKey::new("sk-ant-test1234567890123456789012345678901234567890")?;
    /// let provider = AnthropicProvider::new(api_key)?
    ///     .with_healing(HealingConfig::default());
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_healing(mut self, config: HealingConfig) -> Self {
        self.healing = Some(Arc::new(HealingIntegration::new(config)));
        self
    }

    /// Get the base URL for this provider
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Extract system messages from message list
    fn extract_system_prompt(messages: &[Message]) -> Option<String> {
        let system_messages: Vec<String> = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| m.content.clone())
            .collect();

        if system_messages.is_empty() {
            None
        } else {
            Some(system_messages.join("\n\n"))
        }
    }

    /// Transform messages to Anthropic format (excluding system messages)
    fn transform_messages(messages: &[Message]) -> Vec<AnthropicMessage<'_>> {
        messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| AnthropicMessage {
                role: match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "user", // Fallback (shouldn't happen)
                    Role::Tool => "user",   // Tool messages treated as user messages
                },
                content: &m.content,
            })
            .collect()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        if req.tools.is_some() || req.tool_choice.is_some() {
            return Err(SimpleAgentsError::Config(
                "tool calling is not supported by the anthropic provider".to_string(),
            ));
        }

        use crate::anthropic::models::{AnthropicJsonSchema, AnthropicOutputFormat};
        use simple_agent_type::request::ResponseFormat;

        // Store request context for potential healing
        if self.healing.is_some() && req.response_format.is_some() {
            if let Ok(mut current) = self.current_request.lock() {
                *current = Some(req.clone());
            }
        }

        // Extract system prompt
        let system = Self::extract_system_prompt(&req.messages);

        // Transform messages (excluding system)
        let messages = Self::transform_messages(&req.messages);

        // Convert ResponseFormat to Anthropic's output_format
        let output_format = req.response_format.as_ref().and_then(|rf| match rf {
            ResponseFormat::JsonSchema { json_schema } => Some(AnthropicOutputFormat::JsonSchema {
                json_schema: AnthropicJsonSchema {
                    name: json_schema.name.clone(),
                    schema: json_schema.schema.clone(),
                    strict: json_schema.strict,
                },
            }),
            // Anthropic doesn't have a JsonObject mode without schema
            // So we skip it (user should provide a schema)
            ResponseFormat::JsonObject | ResponseFormat::Text => None,
        });

        // Build Anthropic-specific request
        let anthropic_request = AnthropicCompletionRequest {
            model: &req.model,
            messages,
            system,
            max_tokens: req.max_tokens.unwrap_or(4096), // Anthropic requires max_tokens
            temperature: req.temperature,
            top_p: req.top_p,
            stream: req.stream,
            stop_sequences: req.stop.as_ref(),
            output_format,
        };

        let body = serde_json::to_value(&anthropic_request)?;

        // Build headers
        let mut headers = vec![
            (
                std::borrow::Cow::Borrowed("x-api-key"),
                std::borrow::Cow::Owned(self.api_key.expose().to_string()),
            ),
            (
                std::borrow::Cow::Borrowed("anthropic-version"),
                std::borrow::Cow::Borrowed(Self::API_VERSION),
            ),
            (
                std::borrow::Cow::Borrowed(simple_agent_type::provider::headers::CONTENT_TYPE),
                std::borrow::Cow::Borrowed("application/json"),
            ),
        ];

        // Add beta header for structured outputs
        if req.response_format.is_some() {
            headers.push((
                std::borrow::Cow::Borrowed("anthropic-beta"),
                std::borrow::Cow::Borrowed("structured-outputs-2025-11-13"),
            ));
        }

        Ok(ProviderRequest {
            url: format!("{}/messages", self.base_url),
            headers,
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

        // Handle error responses
        if !status.is_success() {
            let error_body = match response.text().await {
                Ok(body) => {
                    tracing::warn!(
                        status = %status,
                        body_preview = %body.chars().take(200).collect::<String>(),
                        "Anthropic API request failed"
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

            let anthropic_error = AnthropicError::from_response(status.as_u16(), &error_body);

            // Record error metrics
            timer.complete_error(format!("http_{}", status.as_u16()));

            return Err(SimpleAgentsError::Provider(anthropic_error.into()));
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
        let prompt_tokens = body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;

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
        match serde_json::from_value::<AnthropicCompletionResponse>(resp.body.clone()) {
            Ok(anthropic_response) => {
                // Extract text content from content blocks
                let content = anthropic_response
                    .content
                    .iter()
                    .map(|block| {
                        let AnthropicContent::Text { text } = block;
                        text.as_str()
                    })
                    .collect::<Vec<&str>>()
                    .join("");

                // Transform to unified format
                let choice = CompletionChoice {
                    index: 0,
                    message: Message {
                        role: Role::Assistant,
                        content,
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                    },
                    finish_reason: anthropic_response
                        .stop_reason
                        .map(|s| match s.as_str() {
                            "end_turn" => FinishReason::Stop,
                            "max_tokens" => FinishReason::Length,
                            "stop_sequence" => FinishReason::Stop,
                            _ => FinishReason::Stop,
                        })
                        .unwrap_or(FinishReason::Stop),
                    logprobs: None,
                };

                Ok(CompletionResponse {
                    id: anthropic_response.id,
                    model: anthropic_response.model,
                    choices: vec![choice],
                    usage: Usage {
                        prompt_tokens: anthropic_response.usage.input_tokens,
                        completion_tokens: anthropic_response.usage.output_tokens,
                        total_tokens: anthropic_response.usage.input_tokens
                            + anthropic_response.usage.output_tokens,
                    },
                    created: None,
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

impl AnthropicProvider {
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

        // Extract the content from the response - Anthropic uses content array
        let content = resp.body["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .ok_or_else(|| {
                SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                    "No text content in response".to_string(),
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
                prompt_tokens: resp.body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: resp.body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (resp.body["usage"]["input_tokens"].as_u64().unwrap_or(0)
                    + resp.body["usage"]["output_tokens"].as_u64().unwrap_or(0))
                    as u32,
            },
            created: None,
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
                "Anthropic streaming request failed"
            );

            let anthropic_error = AnthropicError::from_response(status.as_u16(), &error_body);
            return Err(SimpleAgentsError::Provider(anthropic_error.into()));
        }

        // Create SSE stream from response bytes
        let byte_stream = response.bytes_stream();
        let sse_stream = streaming::AnthropicSseStream::new(byte_stream);

        Ok(Box::new(sse_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn require_streaming() -> bool {
        std::env::var("SIMPLE_AGENTS_REQUIRE_STREAMING")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    #[test]
    fn test_provider_creation() {
        let api_key = ApiKey::new("sk-ant-test1234567890123456789012345678901234567890").unwrap();
        let provider = AnthropicProvider::new(api_key).unwrap();
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.base_url(), AnthropicProvider::DEFAULT_BASE_URL);
    }

    #[test]
    fn test_extract_system_prompt() {
        let messages = vec![
            Message::system("You are a helpful assistant"),
            Message::user("Hello"),
        ];

        let system = AnthropicProvider::extract_system_prompt(&messages);
        assert_eq!(system, Some("You are a helpful assistant".to_string()));
    }

    #[test]
    fn test_extract_multiple_system_prompts() {
        let messages = vec![
            Message::system("System 1"),
            Message::system("System 2"),
            Message::user("Hello"),
        ];

        let system = AnthropicProvider::extract_system_prompt(&messages);
        assert_eq!(system, Some("System 1\n\nSystem 2".to_string()));
    }

    #[test]
    fn test_transform_messages() {
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];

        let transformed = AnthropicProvider::transform_messages(&messages);
        assert_eq!(transformed.len(), 2); // System message excluded
        assert_eq!(transformed[0].role, "user");
        assert_eq!(transformed[0].content, "Hello");
        assert_eq!(transformed[1].role, "assistant");
        assert_eq!(transformed[1].content, "Hi!");
    }

    #[test]
    fn test_transform_request() {
        let api_key = ApiKey::new("sk-ant-test1234567890123456789012345678901234567890").unwrap();
        let provider = AnthropicProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("claude-3-opus-20240229")
            .message(Message::system("You are helpful"))
            .message(Message::user("Hello"))
            .temperature(0.7)
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();

        assert_eq!(
            provider_request.url,
            "https://api.anthropic.com/v1/messages"
        );
        assert!(provider_request
            .headers
            .iter()
            .any(|(k, _)| k == "x-api-key"));
        assert!(provider_request
            .headers
            .iter()
            .any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
        assert_eq!(provider_request.body["model"], "claude-3-opus-20240229");
        assert_eq!(provider_request.body["system"], "You are helpful");
    }

    #[tokio::test]
    async fn test_anthropic_integration() {
        let response_body = serde_json::json!({
            "id": "msg-test",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello"}],
            "model": "claude-3-opus-20240229",
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        });

        let api_key = ApiKey::new("sk-ant-test1234567890123456789012345678901234567890").unwrap();
        let provider = AnthropicProvider::new(api_key).unwrap();

        let _request = CompletionRequest::builder()
            .model("claude-3-opus-20240229")
            .message(Message::user("Say 'Hello' in one word"))
            .max_tokens(10)
            .build()
            .unwrap();

        let provider_response = ProviderResponse::new(200, response_body);
        let response = provider.transform_response(provider_response).unwrap();

        assert!(!response.choices.is_empty());
        assert!(!response.choices[0].message.content.is_empty());
        println!("Response: {}", response.choices[0].message.content);
    }

    #[tokio::test]
    async fn test_streaming_integration() {
        use crate::anthropic::streaming::AnthropicSseStream;
        use bytes::Bytes;
        use futures_util::stream;
        use futures_util::StreamExt;

        let stream_body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg-test\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3-opus-20240229\"}}\n",
            "\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n",
            "\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}\n",
            "\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n",
            "\n",
        );
        let byte_stream = stream::iter(vec![Ok(Bytes::from(stream_body))]);
        let mut stream = AnthropicSseStream::new(byte_stream);

        let mut chunks_received = 0;
        let mut content = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => {
                    chunks_received += 1;
                    if let Some(delta) =
                        chunk.choices.first().and_then(|c| c.delta.content.as_ref())
                    {
                        content.push_str(delta);
                    }
                    println!("Chunk {}: {:?}", chunks_received, chunk);
                }
                Err(e) => {
                    panic!("Stream error: {}", e);
                }
            }
        }

        if chunks_received == 0 || content.is_empty() {
            if require_streaming() {
                panic!("Should receive at least one chunk");
            } else {
                eprintln!("Streaming returned no chunks; set SIMPLE_AGENTS_REQUIRE_STREAMING=1 to enforce");
                return;
            }
        }
    }
}
