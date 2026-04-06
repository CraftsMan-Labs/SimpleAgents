//! SimpleAgents client implementation.

use crate::healing::{HealedJsonResponse, HealedSchemaResponse, HealingSettings};
use serde::{Deserialize, Serialize};
use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, Result, SimpleAgentsError,
};
use simple_agent_type::telemetry::{ApiFormat, TelemetryConfig, TraceContext};
use simple_agents_healing::coercion::CoercionEngine;
use simple_agents_healing::parser::JsonishParser;
use simple_agents_healing::schema::Schema;
use std::sync::Arc;
use tracing::debug;

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Retry behaviour for failed LLM requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the initial one).
    pub max_attempts: u8,
    /// Base back-off between retries in milliseconds.
    pub backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_ms: 1000,
        }
    }
}

/// Top-level configuration for a [`SimpleAgentsClient`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Provider identifier (e.g. `"openai"`, `"anthropic"`).
    pub provider: String,
    /// API key used for authentication with the provider.
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
        let provider_response = self.provider.execute(provider_request).await?;
        self.provider.transform_response(provider_response)
    }

    async fn complete_json_internal(
        &self,
        request: &CompletionRequest,
    ) -> Result<HealedJsonResponse> {
        self.ensure_healing_enabled()?;
        let response = self.complete_response(request).await?;
        let content = response.content().ok_or_else(|| {
            SimpleAgentsError::Healing(simple_agent_type::error::HealingError::ParseFailed {
                error_message: "response contained no content".to_string(),
                input: String::new(),
            })
        })?;

        let parser = JsonishParser::with_config(self.healing.parser_config.clone());
        let parsed = parser.parse(content)?;

        Ok(HealedJsonResponse { response, parsed })
    }

    async fn complete_with_schema_internal(
        &self,
        request: &CompletionRequest,
        schema: &Schema,
    ) -> Result<HealedSchemaResponse> {
        self.ensure_healing_enabled()?;
        let healed = self.complete_json_internal(request).await?;
        let engine = CoercionEngine::with_config(self.healing.coercion_config.clone());
        let coerced = engine
            .coerce(&healed.parsed.value, schema)
            .map_err(SimpleAgentsError::Healing)?;

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
            Err(SimpleAgentsError::Config(
                "healing is disabled for this client".to_string(),
            ))
        }
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
