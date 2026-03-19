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
use std::sync::Arc;
use std::time::Duration;

use crate::healing_integration::{HealingConfig, HealingIntegration};
use crate::utils::DEFAULT_TIMEOUT;

pub(crate) fn normalize_openai_strict_json_schema(body: &mut serde_json::Value) {
    let Some(response_format) = body.get_mut("response_format") else {
        return;
    };
    if !response_format_is_strict_json_schema(response_format) {
        return;
    }
    let Some(schema) = response_format
        .get_mut("json_schema")
        .and_then(|json_schema| json_schema.get_mut("schema"))
    else {
        return;
    };
    enforce_strict_object_schema(schema);
}

fn response_format_is_strict_json_schema(response_format: &serde_json::Value) -> bool {
    let is_json_schema = response_format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(|value| value == "json_schema")
        .unwrap_or(false);
    if !is_json_schema {
        return false;
    }

    response_format
        .get("json_schema")
        .and_then(|json_schema| json_schema.get("strict"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn enforce_strict_object_schema(schema: &mut serde_json::Value) {
    match schema {
        serde_json::Value::Object(map) => {
            if type_includes_object(map.get("type")) {
                map.entry("additionalProperties".to_string())
                    .or_insert(serde_json::Value::Bool(false));
            }

            for key in ["properties", "$defs", "definitions", "dependentSchemas"] {
                if let Some(serde_json::Value::Object(children)) = map.get_mut(key) {
                    for value in children.values_mut() {
                        enforce_strict_object_schema(value);
                    }
                }
            }

            for key in ["items", "contains", "if", "then", "else", "not"] {
                if let Some(value) = map.get_mut(key) {
                    enforce_strict_object_schema(value);
                }
            }

            for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
                if let Some(serde_json::Value::Array(values)) = map.get_mut(key) {
                    for value in values {
                        enforce_strict_object_schema(value);
                    }
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                enforce_strict_object_schema(value);
            }
        }
        _ => {}
    }
}

fn type_includes_object(type_field: Option<&serde_json::Value>) -> bool {
    match type_field {
        Some(serde_json::Value::String(value)) => value == "object",
        Some(serde_json::Value::Array(values)) => values.iter().any(|entry| {
            entry
                .as_str()
                .map(|value| value == "object")
                .unwrap_or(false)
        }),
        _ => false,
    }
}

/// OpenAI API provider
#[derive(Clone)]
pub struct OpenAIProvider {
    api_key: ApiKey,
    base_url: String,
    client: Client,
    rate_limiter: crate::rate_limit::MaybeRateLimiter,
    healing: Option<Arc<HealingIntegration>>,
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
            .timeout(DEFAULT_TIMEOUT)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90));
        if is_local {
            client_builder = client_builder.no_proxy();
        }
        // Removed http2_prior_knowledge for non-local connections
        // to allow ALPN negotiation which is more compatible
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
            .timeout(DEFAULT_TIMEOUT)
            .pool_max_idle_per_host(10) // Connection pooling configuration
            .pool_idle_timeout(Duration::from_secs(90)) // Keep connections alive
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
        // Build OpenAI-specific request (borrowing messages to avoid cloning)
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
        self.embed_healing_schema(&mut body, req)?;

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

    async fn execute(&self, mut req: ProviderRequest) -> Result<ProviderResponse> {
        let healing_schema = Self::take_healing_schema(&mut req.body);

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
                        DEFAULT_TIMEOUT,
                    )));
                } else {
                    timer.complete_error("network");
                    return Err(SimpleAgentsError::Network(format!("Network error: {}", e)));
                }
            }
        };

        let status = response.status();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(crate::utils::parse_retry_after);

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

            let openai_error =
                OpenAIError::from_response(status.as_u16(), &error_body, retry_after);

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
        let mut body = match response.json::<serde_json::Value>().await {
            Ok(b) => b,
            Err(e) => {
                timer.complete_error("parse_error");
                return Err(SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                    format!("Failed to parse JSON response: {}", e),
                )));
            }
        };

        // Extract token usage for metrics
        let prompt_tokens = Self::safe_token_count(
            body["usage"]["prompt_tokens"].as_u64(),
            "usage.prompt_tokens",
        );
        let completion_tokens = Self::safe_token_count(
            body["usage"]["completion_tokens"].as_u64(),
            "usage.completion_tokens",
        );

        if let Some(schema) = healing_schema {
            if let serde_json::Value::Object(map) = &mut body {
                map.insert(Self::HEALING_SCHEMA_KEY.to_string(), schema);
            }
        }

        // Record success metrics
        timer.complete_success(prompt_tokens, completion_tokens);

        Ok(ProviderResponse {
            status: status.as_u16(),
            body,
            headers: None,
        })
    }

    fn transform_response(&self, mut resp: ProviderResponse) -> Result<CompletionResponse> {
        Self::normalize_tool_message_content(&mut resp.body);
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
                        reasoning_tokens: openai_response.usage.reasoning_tokens(),
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

    async fn execute_stream(
        &self,
        req: ProviderRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        self.execute_stream_impl(req).await
    }
}

impl OpenAIProvider {
    const HEALING_SCHEMA_KEY: &'static str = "_simple_agents_healing_schema";

    fn embed_healing_schema(
        &self,
        body: &mut serde_json::Value,
        req: &CompletionRequest,
    ) -> Result<()> {
        if self.healing.is_none() {
            return Ok(());
        }

        let Some(ResponseFormat::JsonSchema { json_schema }) = req.response_format.as_ref() else {
            return Ok(());
        };

        let serde_json::Value::Object(map) = body else {
            return Err(SimpleAgentsError::Provider(ProviderError::BadRequest(
                "OpenAI request body must be a JSON object".to_string(),
            )));
        };

        map.insert(
            Self::HEALING_SCHEMA_KEY.to_string(),
            json_schema.schema.clone(),
        );
        Ok(())
    }

    fn take_healing_schema(body: &mut serde_json::Value) -> Option<serde_json::Value> {
        if let serde_json::Value::Object(map) = body {
            return map.remove(Self::HEALING_SCHEMA_KEY);
        }
        None
    }

    fn normalize_tool_message_content(body: &mut serde_json::Value) {
        let Some(choices) = body
            .get_mut("choices")
            .and_then(serde_json::Value::as_array_mut)
        else {
            return;
        };

        for choice in choices {
            let Some(message) = choice
                .get_mut("message")
                .and_then(serde_json::Value::as_object_mut)
            else {
                continue;
            };

            if !message.contains_key("content") || message["content"].is_null() {
                message.insert(
                    "content".to_string(),
                    serde_json::Value::String(String::new()),
                );
            }
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

    /// Attempt to heal a malformed response using the healing system.
    fn try_healing(
        &self,
        resp: &ProviderResponse,
        original_error: serde_json::Error,
    ) -> Result<CompletionResponse> {
        let healing = self.healing.as_ref().unwrap();

        let json_schema = resp.body.get(Self::HEALING_SCHEMA_KEY).ok_or_else(|| {
            SimpleAgentsError::Provider(ProviderError::InvalidResponse(
                "No JSON schema available for healing".to_string(),
            ))
        })?;

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
            json_schema,
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
                prompt_tokens: Self::safe_token_count(
                    resp.body["usage"]["prompt_tokens"].as_u64(),
                    "usage.prompt_tokens",
                ),
                completion_tokens: Self::safe_token_count(
                    resp.body["usage"]["completion_tokens"].as_u64(),
                    "usage.completion_tokens",
                ),
                total_tokens: Self::safe_token_count(
                    resp.body["usage"]["total_tokens"].as_u64(),
                    "usage.total_tokens",
                ),
                reasoning_tokens: Self::extract_reasoning_tokens_from_usage(&resp.body["usage"]),
            },
            created: resp.body["created"].as_i64(),
            provider: Some(self.name().to_string()),
            healing_metadata: Some(healed.metadata),
        })
    }

    fn extract_reasoning_tokens_from_usage(usage: &serde_json::Value) -> Option<u32> {
        serde_json::from_value::<OpenAIUsage>(usage.clone())
            .ok()
            .and_then(|parsed| parsed.reasoning_tokens())
    }

    async fn execute_stream_impl(
        &self,
        mut req: ProviderRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        let _ = Self::take_healing_schema(&mut req.body);

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
                    SimpleAgentsError::Provider(ProviderError::Timeout(DEFAULT_TIMEOUT))
                } else {
                    SimpleAgentsError::Network(format!("Network error: {}", e))
                }
            })?;

        let status = response.status();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(crate::utils::parse_retry_after);

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

            let openai_error =
                OpenAIError::from_response(status.as_u16(), &error_body, retry_after);
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
    use serde_json::json;
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

    async fn spawn_malformed_chunked_error_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr should resolve");

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let response = concat!(
                    "HTTP/1.1 429 Too Many Requests\r\n",
                    "transfer-encoding: chunked\r\n",
                    "content-type: application/json\r\n",
                    "retry-after: 1\r\n",
                    "\r\n",
                    "ZZ\r\n",
                    "not-valid-chunk\r\n",
                    "0\r\n\r\n"
                );
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
        assert_eq!(
            provider_request.body["stream_options"]["include_usage"],
            true
        );
    }

    #[test]
    fn test_transform_request_normalizes_strict_json_schema_objects() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4.1")
            .message(Message::user("Extract person"))
            .response_format(ResponseFormat::JsonSchema {
                json_schema: simple_agent_type::request::JsonSchemaFormat {
                    name: "person".to_string(),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "meta": {
                                "type": "object",
                                "properties": {
                                    "city": {"type": "string"}
                                },
                                "required": ["city"]
                            }
                        },
                        "required": ["name", "meta"]
                    }),
                    strict: Some(true),
                },
            })
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();
        assert_eq!(
            provider_request.body["response_format"]["json_schema"]["schema"]
                ["additionalProperties"],
            json!(false)
        );
        assert_eq!(
            provider_request.body["response_format"]["json_schema"]["schema"]["properties"]["meta"]
                ["additionalProperties"],
            json!(false)
        );
    }

    #[test]
    fn test_transform_request_does_not_normalize_non_strict_json_schema() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4.1")
            .message(Message::user("Extract person"))
            .response_format(ResponseFormat::JsonSchema {
                json_schema: simple_agent_type::request::JsonSchemaFormat {
                    name: "person".to_string(),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"}
                        },
                        "required": ["name"]
                    }),
                    strict: Some(false),
                },
            })
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();
        assert_eq!(
            provider_request.body["response_format"]["json_schema"]["schema"]
                ["additionalProperties"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn test_transform_request_does_not_normalize_when_strict_omitted() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4.1")
            .message(Message::user("Extract person"))
            .response_format(ResponseFormat::JsonSchema {
                json_schema: simple_agent_type::request::JsonSchemaFormat {
                    name: "person".to_string(),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"}
                        },
                        "required": ["name"]
                    }),
                    strict: None,
                },
            })
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();
        assert_eq!(
            provider_request.body["response_format"]["json_schema"]["schema"]
                ["additionalProperties"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn test_transform_request_preserves_existing_additional_properties_schema() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4.1")
            .message(Message::user("Extract map"))
            .response_format(ResponseFormat::JsonSchema {
                json_schema: simple_agent_type::request::JsonSchemaFormat {
                    name: "kv_map".to_string(),
                    schema: json!({
                        "type": "object",
                        "additionalProperties": {"type": "string"}
                    }),
                    strict: Some(true),
                },
            })
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();
        assert_eq!(
            provider_request.body["response_format"]["json_schema"]["schema"]
                ["additionalProperties"],
            json!({"type": "string"})
        );
    }

    #[test]
    fn test_transform_response_allows_null_message_content_for_tool_calls() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let response = ProviderResponse {
            status: 200,
            body: json!({
                "id": "chatcmpl-tool",
                "object": "chat.completion",
                "created": 1700000000,
                "model": "gemini-3-flash",
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": null,
                            "tool_calls": [
                                {
                                    "id": "call_1",
                                    "type": "function",
                                    "function": {
                                        "name": "get_employee_record",
                                        "arguments": "{\"employee_name\":\"Alex Johnson\"}"
                                    }
                                }
                            ]
                        },
                        "finish_reason": "tool_calls"
                    }
                ],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 6,
                    "total_tokens": 16
                }
            }),
            headers: None,
        };

        let parsed = provider
            .transform_response(response)
            .expect("response should parse");
        let choice = parsed.choices.first().expect("choice should exist");
        assert_eq!(choice.message.content, "");
        assert_eq!(choice.finish_reason, FinishReason::ToolCalls);
        assert!(choice.message.tool_calls.is_some());
    }

    #[test]
    fn test_transform_response_maps_reasoning_tokens_from_usage_details() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let response = ProviderResponse {
            status: 200,
            body: json!({
                "id": "chatcmpl-reasoning",
                "object": "chat.completion",
                "created": 1700000000,
                "model": "gpt-4o",
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "Answer"
                        },
                        "finish_reason": "stop"
                    }
                ],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 6,
                    "total_tokens": 16,
                    "completion_tokens_details": {
                        "reasoning_tokens": 4
                    }
                }
            }),
            headers: None,
        };

        let parsed = provider
            .transform_response(response)
            .expect("response should parse");
        assert_eq!(parsed.usage.reasoning_tokens, Some(4));
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

    #[tokio::test]
    async fn test_execute_timeout_maps_to_default_timeout_constant() {
        let base_url = spawn_hanging_server().await;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(30))
            .build()
            .expect("client should build");
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::with_client(api_key, base_url, client).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4")
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
            Some("2"),
            r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit"}}"#,
        )
        .await;
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::with_base_url(api_key, base_url).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .stream(true)
            .build()
            .unwrap();
        let provider_request = provider.transform_request(&request).unwrap();

        let result = provider.execute_stream(provider_request).await;
        assert!(matches!(
            result,
            Err(SimpleAgentsError::Provider(ProviderError::RateLimit { retry_after: Some(d) })) if d == Duration::from_secs(2)
        ));
    }

    #[tokio::test]
    async fn test_execute_stream_handles_unreadable_error_body() {
        let base_url = spawn_malformed_chunked_error_server().await;
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::with_base_url(api_key, base_url).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .stream(true)
            .build()
            .unwrap();
        let provider_request = provider.transform_request(&request).unwrap();

        let result = provider.execute_stream(provider_request).await;
        assert!(matches!(
            result,
            Err(SimpleAgentsError::Provider(ProviderError::RateLimit { retry_after: Some(d) })) if d == Duration::from_secs(1)
        ));
    }

    #[test]
    fn test_from_env_requires_api_key() {
        let _guard = env_lock().lock().expect("env lock should not be poisoned");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_API_BASE");

        let result = OpenAIProvider::from_env();
        assert!(matches!(result, Err(SimpleAgentsError::Config(_))));
    }

    #[test]
    fn test_from_env_respects_custom_base_url() {
        let _guard = env_lock().lock().expect("env lock should not be poisoned");
        std::env::set_var(
            "OPENAI_API_KEY",
            "sk-test1234567890123456789012345678901234567890",
        );
        std::env::set_var("OPENAI_API_BASE", "http://localhost:9999/v1");

        let provider = OpenAIProvider::from_env().expect("from_env should build provider");
        assert_eq!(provider.base_url(), "http://localhost:9999/v1");

        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_API_BASE");
    }
}
