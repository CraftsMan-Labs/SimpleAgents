//! SimpleAgents client implementation.

use crate::healing::{HealedJsonResponse, HealedSchemaResponse, HealingFailure, HealingSettings};
use rand::Rng;
use serde::{Deserialize, Serialize};
use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, Result, SimpleAgentsError,
};
use simple_agent_type::provider::RetryConfig;
use simple_agent_type::telemetry::{ApiFormat, TelemetryConfig, TraceContext};
use simple_agents_healing::coercion::CoercionEngine;
use simple_agents_healing::parser::JsonishParser;
use simple_agents_healing::schema::Schema;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Top-level configuration for a [`SimpleAgentsClient`].
///
/// The [`api_key`](Self::api_key) field is excluded from serialization and
/// redacted in `Debug` output to prevent accidental leakage into logs or
/// JSON responses.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Provider identifier (e.g. `"openai"`, `"anthropic"`).
    pub provider: String,
    /// API key used for authentication with the provider.
    ///
    /// Excluded from serialization output and redacted in `Debug` to prevent
    /// accidental leakage into logs, traces, or API responses.
    #[serde(skip_serializing)]
    pub api_key: String,
    /// Optional custom base URL for the provider API.
    pub base_url: Option<String>,
    /// Wire format for the provider API.
    pub api_format: ApiFormat,
    /// Extra HTTP headers sent with every request.
    pub extra_headers: Option<Vec<(String, String)>>,
    /// Optional telemetry / OTEL export configuration.
    pub telemetry: Option<TelemetryConfig>,
    /// Default retry policy applied to all requests.
    pub default_retry: RetryConfig,
}

impl std::fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redacted = if self.api_key.is_empty() {
            "<empty>"
        } else {
            "[REDACTED]"
        };

        f.debug_struct("ClientConfig")
            .field("provider", &self.provider)
            .field("api_key", &redacted)
            .field("base_url", &self.base_url)
            .field("api_format", &self.api_format)
            .field("extra_headers", &self.extra_headers)
            .field("telemetry", &self.telemetry)
            .field("default_retry", &self.default_retry)
            .finish()
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            api_key: String::new(),
            base_url: None,
            api_format: ApiFormat::default(),
            extra_headers: None,
            telemetry: None,
            default_retry: RetryConfig::default(),
        }
    }
}

/// Flags that control execution behaviour for a single run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionFlags {
    /// Whether the workflow-level orchestrator streams between nodes.
    pub workflow_streaming: bool,
    /// Whether individual LLM calls within a node use streaming.
    pub node_llm_streaming: bool,
}

impl Default for ExecutionFlags {
    fn default() -> Self {
        Self {
            workflow_streaming: false,
            node_llm_streaming: true,
        }
    }
}

/// Per-run options passed to the executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOptions {
    /// Print timing / token-usage statistics after the run.
    pub nerdstats: bool,
    /// Whether to emit telemetry spans for this run.
    pub telemetry_enabled: bool,
    /// Distributed trace context to propagate.
    pub trace_context: Option<TraceContext>,
    /// Execution behaviour flags.
    pub execution_flags: ExecutionFlags,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            nerdstats: true,
            telemetry_enabled: true,
            trace_context: None,
            execution_flags: ExecutionFlags::default(),
        }
    }
}

/// Mode for completion post-processing.
#[derive(Clone)]
pub enum CompletionMode {
    /// Return the raw completion response.
    Standard,
    /// Parse the response content as JSON using healing.
    HealedJson,
    /// Parse and coerce the response into the provided schema.
    CoercedSchema(Schema),
}

/// Options that control completion behavior.
#[derive(Clone)]
pub struct CompletionOptions {
    /// Completion post-processing mode.
    pub mode: CompletionMode,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            mode: CompletionMode::Standard,
        }
    }
}

/// Result of a unified completion call.
pub enum CompletionOutcome {
    /// A standard, non-streaming completion response.
    Response(CompletionResponse),
    /// A streaming response yielding completion chunks.
    Stream(Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>),
    /// A healed JSON response.
    HealedJson(HealedJsonResponse),
    /// A schema-coerced response.
    CoercedSchema(HealedSchemaResponse),
}

/// Unified SimpleAgents client.
pub struct SimpleAgentsClient {
    provider: Arc<dyn Provider>,
    config: ClientConfig,
    healing: HealingSettings,
}

impl SimpleAgentsClient {
    /// Create a new client wrapping a single provider with default config and healing.
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            config: ClientConfig::default(),
            healing: HealingSettings::default(),
        }
    }

    /// Create a new client from a [`ClientConfig`], using the supplied provider.
    pub fn from_config(provider: Arc<dyn Provider>, config: ClientConfig) -> Self {
        Self {
            provider,
            config,
            healing: HealingSettings::default(),
        }
    }

    /// Create a new client with custom healing settings.
    pub fn with_healing(provider: Arc<dyn Provider>, healing: HealingSettings) -> Self {
        Self {
            provider,
            config: ClientConfig::default(),
            healing,
        }
    }

    /// Return a reference to the client's configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Return the name of the underlying provider.
    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    /// Execute a completion request.
    pub async fn complete(
        &self,
        request: &CompletionRequest,
        options: CompletionOptions,
    ) -> Result<CompletionOutcome> {
        if request.stream.unwrap_or(false) {
            if matches!(
                options.mode,
                CompletionMode::HealedJson | CompletionMode::CoercedSchema(_)
            ) {
                return Err(SimpleAgentsError::Config(
                    "streaming is incompatible with HealedJson/CoercedSchema modes; \
                     use Raw mode for streaming or disable streaming for structured output"
                        .to_string(),
                ));
            }
            let stream = self.stream(request).await?;
            return Ok(CompletionOutcome::Stream(stream));
        }

        match options.mode {
            CompletionMode::Standard => {
                let response = self.complete_response(request).await?;
                Ok(CompletionOutcome::Response(response))
            }
            CompletionMode::HealedJson => {
                let healed = self.complete_json_internal(request).await?;
                Ok(CompletionOutcome::HealedJson(healed))
            }
            CompletionMode::CoercedSchema(schema) => {
                let healed = self.complete_with_schema_internal(request, &schema).await?;
                Ok(CompletionOutcome::CoercedSchema(healed))
            }
        }
    }

    async fn complete_response(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        request.validate()?;

        let provider_request = self.provider.transform_request(request)?;
        let provider_response = self.execute_with_retries(provider_request).await?;
        self.provider.transform_response(provider_response)
    }

    async fn execute_with_retries(
        &self,
        provider_request: simple_agent_type::provider::ProviderRequest,
    ) -> Result<simple_agent_type::provider::ProviderResponse> {
        let retry = &self.config.default_retry;
        let max_attempts = retry.max_attempts.max(1);
        let mut attempt = 1;

        loop {
            // Clone required because `Provider::execute` consumes the request body.
            // For large multimodal payloads (images, audio) this duplication is
            // non-trivial. If retries on big requests are common, prefer using
            // multiple `SimpleAgentsClient` instances rather than relying on the
            // retry loop.
            match self.provider.execute(provider_request.clone()).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    if attempt >= max_attempts || !is_retryable_error(&error) {
                        return Err(error);
                    }

                    let delay = retry_delay(retry, attempt, &error);
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    attempt += 1;
                }
            }
        }
    }

    async fn complete_json_internal(
        &self,
        request: &CompletionRequest,
    ) -> Result<HealedJsonResponse> {
        self.ensure_healing_enabled()?;
        let response = self.complete_response(request).await?;

        let parsed = match response.content() {
            Some(content) => {
                let parser = JsonishParser::with_config(self.healing.parser_config.clone());
                match parser.parse(content) {
                    Ok(result) => Ok(result),
                    Err(SimpleAgentsError::Healing(healing_err)) => Err(HealingFailure {
                        error: healing_err,
                        raw_text: content.to_string(),
                    }),
                    Err(other) => return Err(other),
                }
            }
            None => Err(HealingFailure {
                error: simple_agent_type::error::HealingError::ParseFailed {
                    error_message: "response contained no content".to_string(),
                    input: String::new(),
                },
                raw_text: String::new(),
            }),
        };

        Ok(HealedJsonResponse { response, parsed })
    }

    async fn complete_with_schema_internal(
        &self,
        request: &CompletionRequest,
        schema: &Schema,
    ) -> Result<HealedSchemaResponse> {
        self.ensure_healing_enabled()?;
        let healed = self.complete_json_internal(request).await?;

        let coerced = match &healed.parsed {
            Ok(parsed) => {
                let engine = CoercionEngine::with_config(self.healing.coercion_config.clone());
                match engine.coerce(&parsed.value, schema) {
                    Ok(result) => Some(Ok(result)),
                    Err(healing_err) => Some(Err(HealingFailure {
                        error: healing_err,
                        raw_text: serde_json::to_string(&parsed.value).unwrap_or_default(),
                    })),
                }
            }
            Err(_) => None,
        };

        Ok(HealedSchemaResponse {
            response: healed.response,
            parsed: healed.parsed,
            coerced,
        })
    }

    async fn stream(
        &self,
        request: &CompletionRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        request.validate()?;
        debug!(
            model = %request.model,
            stream = ?request.stream,
            "SimpleAgentsClient.stream start"
        );

        let provider_request = self.provider.transform_request(request)?;
        self.provider.execute_stream(provider_request).await
    }

    fn ensure_healing_enabled(&self) -> Result<()> {
        if self.healing.enabled {
            Ok(())
        } else {
            Err(SimpleAgentsError::HealingDisabled)
        }
    }
}

fn is_retryable_error(error: &SimpleAgentsError) -> bool {
    match error {
        SimpleAgentsError::Provider(provider_error) => provider_error.is_retryable(),
        SimpleAgentsError::Network(_) => true,
        _ => false,
    }
}

fn retry_after(error: &SimpleAgentsError) -> Option<Duration> {
    match error {
        SimpleAgentsError::Provider(simple_agent_type::error::ProviderError::RateLimit {
            retry_after,
        }) => *retry_after,
        _ => None,
    }
}

fn retry_delay(retry: &RetryConfig, failed_attempt: u32, error: &SimpleAgentsError) -> Duration {
    if let Some(delay) = retry_after(error) {
        return delay;
    }

    let factor = retry
        .backoff_multiplier
        .max(1.0)
        .powi(failed_attempt.saturating_sub(1).min(31) as i32);
    let delay = retry.initial_backoff.mul_f32(factor);
    let delay = delay.min(retry.max_backoff.max(retry.initial_backoff));

    if retry.jitter {
        let jitter_factor = rand::thread_rng().gen_range(0.5..=1.5);
        delay.mul_f64(jitter_factor)
    } else {
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures_util::StreamExt;
    use simple_agent_type::error::ProviderError;
    use simple_agent_type::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockProvider {
        name: &'static str,
        calls: AtomicUsize,
    }

    impl MockProvider {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &str {
            self.name
        }

        fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
            Ok(ProviderRequest::new("http://example.com"))
        }

        async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(ProviderResponse::new(
                200,
                serde_json::json!({"content": "ok"}),
            ))
        }

        fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
            Ok(CompletionResponse {
                id: "resp_test".to_string(),
                model: "test-model".to_string(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("ok"),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                }],
                usage: Usage::new(1, 1),
                created: None,
                provider: Some(self.name.to_string()),
                healing_metadata: None,
            })
        }
    }

    #[tokio::test]
    async fn complete_returns_response() {
        let provider = Arc::new(MockProvider::new("p1"));
        let client = SimpleAgentsClient::new(provider);

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hi"))
            .build()
            .unwrap();

        let outcome = client
            .complete(&request, CompletionOptions::default())
            .await
            .unwrap();

        match outcome {
            CompletionOutcome::Response(resp) => {
                assert_eq!(resp.provider.as_deref(), Some("p1"));
            }
            _ => panic!("expected Response outcome"),
        }
    }

    struct RetryProvider {
        name: &'static str,
        failures_before_success: usize,
        error: ProviderError,
        calls: AtomicUsize,
    }

    impl RetryProvider {
        fn new(name: &'static str, failures_before_success: usize, error: ProviderError) -> Self {
            Self {
                name,
                failures_before_success,
                error,
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl Provider for RetryProvider {
        fn name(&self) -> &str {
            self.name
        }

        fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
            Ok(ProviderRequest::new("http://example.com"))
        }

        async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed);
            if call < self.failures_before_success {
                return Err(SimpleAgentsError::Provider(self.error.clone()));
            }

            Ok(ProviderResponse::new(
                200,
                serde_json::json!({"content": "ok"}),
            ))
        }

        fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
            Ok(CompletionResponse {
                id: "resp_retry".to_string(),
                model: "test-model".to_string(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("ok"),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                }],
                usage: Usage::new(1, 1),
                created: None,
                provider: Some(self.name.to_string()),
                healing_metadata: None,
            })
        }
    }

    fn retry_test_config(max_attempts: u32, backoff_multiplier: f32) -> ClientConfig {
        ClientConfig {
            default_retry: RetryConfig {
                max_attempts,
                initial_backoff: Duration::ZERO,
                max_backoff: Duration::ZERO,
                backoff_multiplier,
                jitter: false,
            },
            ..ClientConfig::default()
        }
    }

    #[tokio::test]
    async fn complete_retries_retryable_provider_errors() {
        let provider = Arc::new(RetryProvider::new(
            "retry",
            2,
            ProviderError::ServerError("temporary".to_string()),
        ));
        let client = SimpleAgentsClient::from_config(provider.clone(), retry_test_config(3, 1.0));

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hi"))
            .build()
            .unwrap();

        let outcome = client
            .complete(&request, CompletionOptions::default())
            .await
            .unwrap();

        assert!(matches!(outcome, CompletionOutcome::Response(_)));
        assert_eq!(provider.calls(), 3);
    }

    #[tokio::test]
    async fn complete_does_not_retry_non_retryable_provider_errors() {
        let provider = Arc::new(RetryProvider::new("retry", 1, ProviderError::InvalidApiKey));
        let client = SimpleAgentsClient::from_config(provider.clone(), retry_test_config(3, 1.0));

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hi"))
            .build()
            .unwrap();

        let result = client
            .complete(&request, CompletionOptions::default())
            .await;

        assert!(result.is_err());
        assert_eq!(provider.calls(), 1);
    }

    #[tokio::test]
    async fn complete_does_not_retry_when_strategy_is_none() {
        let provider = Arc::new(RetryProvider::new(
            "retry",
            1,
            ProviderError::ServerError("temporary".to_string()),
        ));
        let client = SimpleAgentsClient::from_config(provider.clone(), retry_test_config(1, 1.0));

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hi"))
            .build()
            .unwrap();

        let result = client
            .complete(&request, CompletionOptions::default())
            .await;

        assert!(result.is_err());
        assert_eq!(provider.calls(), 1);
    }

    #[test]
    fn retry_delay_uses_backoff_multiplier() {
        let error =
            SimpleAgentsError::Provider(ProviderError::ServerError("temporary".to_string()));
        let fixed = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_millis(1_000),
            backoff_multiplier: 1.0,
            jitter: false,
        };
        let exponential = RetryConfig {
            backoff_multiplier: 2.0,
            ..fixed.clone()
        };

        assert_eq!(retry_delay(&fixed, 2, &error).as_millis(), 100);
        assert_eq!(retry_delay(&exponential, 1, &error).as_millis(), 100);
        assert_eq!(retry_delay(&exponential, 4, &error).as_millis(), 800);
    }

    #[test]
    fn retry_delay_with_jitter_stays_within_expected_range() {
        let error =
            SimpleAgentsError::Provider(ProviderError::ServerError("temporary".to_string()));
        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1_000),
            max_backoff: Duration::from_millis(10_000),
            backoff_multiplier: 1.0,
            jitter: true,
        };

        let base_ms = 1_000u128;
        let min_expected = base_ms / 2; // 0.5x
        let max_expected = base_ms * 3 / 2; // 1.5x

        for _ in 0..50 {
            let delay = retry_delay(&config, 1, &error);
            let ms = delay.as_millis();
            assert!(
                ms >= min_expected && ms <= max_expected,
                "jittered delay {ms}ms outside expected range [{min_expected}, {max_expected}]",
            );
        }

        let mut delays = std::collections::HashSet::new();
        for _ in 0..20 {
            delays.insert(retry_delay(&config, 1, &error).as_nanos());
        }
        assert!(
            delays.len() > 1,
            "expected jitter to produce varying delays, but got {} distinct value(s)",
            delays.len(),
        );
    }

    struct StreamingProvider {
        name: &'static str,
        fail_after_first: bool,
    }

    impl StreamingProvider {
        fn new(name: &'static str, fail_after_first: bool) -> Self {
            Self {
                name,
                fail_after_first,
            }
        }

        fn build_chunk(id: &str, content: &str) -> CompletionChunk {
            CompletionChunk {
                id: id.to_string(),
                model: "test-model".to_string(),
                choices: vec![ChoiceDelta {
                    index: 0,
                    delta: MessageDelta {
                        role: Some(Role::Assistant),
                        content: Some(content.to_string()),
                        reasoning_content: None,
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
                created: None,
                usage: None,
            }
        }
    }

    #[async_trait]
    impl Provider for StreamingProvider {
        fn name(&self) -> &str {
            self.name
        }

        fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
            Ok(ProviderRequest::new("http://example.com"))
        }

        async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
            Ok(ProviderResponse::new(
                200,
                serde_json::json!({"content": "ok"}),
            ))
        }

        fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
            Ok(CompletionResponse {
                id: "resp_stream".to_string(),
                model: "test-model".to_string(),
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("ok"),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                }],
                usage: Usage::new(1, 1),
                created: None,
                provider: Some(self.name.to_string()),
                healing_metadata: None,
            })
        }

        async fn execute_stream(
            &self,
            _req: ProviderRequest,
        ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>>
        {
            let stream = if self.fail_after_first {
                let items: Vec<Result<CompletionChunk>> = vec![
                    Ok(Self::build_chunk("chunk-1", "hello")),
                    Err(SimpleAgentsError::Provider(ProviderError::ServerError(
                        "stream error".to_string(),
                    ))),
                ];
                futures_util::stream::iter(items)
            } else {
                let items: Vec<Result<CompletionChunk>> =
                    vec![Ok(Self::build_chunk("chunk-1", "hello"))];
                futures_util::stream::iter(items)
            };

            Ok(Box::new(stream))
        }
    }

    #[tokio::test]
    async fn streaming_returns_chunks() {
        let provider = Arc::new(StreamingProvider::new("p1", false));
        let client = SimpleAgentsClient::new(provider);

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hi"))
            .stream(true)
            .build()
            .unwrap();

        let outcome = client
            .complete(&request, CompletionOptions::default())
            .await
            .unwrap();

        let mut collected = Vec::new();
        match outcome {
            CompletionOutcome::Stream(mut stream) => {
                while let Some(chunk) = stream.next().await {
                    collected.push(chunk.unwrap());
                }
            }
            _ => panic!("expected stream outcome"),
        }

        assert_eq!(collected.len(), 1);
    }

    #[tokio::test]
    async fn streaming_propagates_error() {
        let provider = Arc::new(StreamingProvider::new("p1", true));
        let client = SimpleAgentsClient::new(provider);

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hi"))
            .stream(true)
            .build()
            .unwrap();

        let outcome = client
            .complete(&request, CompletionOptions::default())
            .await
            .unwrap();

        let mut chunks = Vec::new();
        match outcome {
            CompletionOutcome::Stream(mut stream) => {
                while let Some(chunk) = stream.next().await {
                    chunks.push(chunk);
                }
            }
            _ => panic!("expected stream outcome"),
        }

        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].is_ok());
        assert!(chunks[1].is_err());
    }
}
