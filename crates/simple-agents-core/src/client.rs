//! SimpleAgents client implementation.

use crate::healing::{HealedJsonResponse, HealedSchemaResponse, HealingSettings};
use crate::middleware::Middleware;
use crate::routing::{RouterEngine, RoutingMode};
use async_trait::async_trait;
use futures_util::future::BoxFuture;
use futures_util::stream::{self, Stream};
use futures_util::StreamExt;
use simple_agent_type::cache::Cache;
use simple_agent_type::cache::CacheKey;
use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, Result, SimpleAgentsError,
};
use simple_agents_healing::coercion::CoercionEngine;
use simple_agents_healing::parser::JsonishParser;
use simple_agents_healing::schema::Schema;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

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

struct ClientState {
    providers: Vec<Arc<dyn Provider>>,
    provider_map: HashMap<String, Arc<dyn Provider>>,
    router: Arc<RouterEngine>,
}

/// Unified SimpleAgents client.
pub struct SimpleAgentsClient {
    state: RwLock<ClientState>,
    routing_mode: RoutingMode,
    cache: Option<Arc<dyn Cache>>,
    cache_ttl: Duration,
    healing: HealingSettings,
    middleware: Vec<Arc<dyn Middleware>>,
}

impl SimpleAgentsClient {
    /// Start a new client builder.
    pub fn builder() -> SimpleAgentsClientBuilder {
        SimpleAgentsClientBuilder::new()
    }

    /// List registered provider names.
    pub async fn provider_names(&self) -> Result<Vec<String>> {
        let state = self.state.read().await;
        Ok(state.provider_map.keys().cloned().collect())
    }

    /// Retrieve a provider by name.
    pub async fn provider(&self, name: &str) -> Result<Option<Arc<dyn Provider>>> {
        let state = self.state.read().await;
        Ok(state.provider_map.get(name).cloned())
    }

    /// Register an additional provider and rebuild the router.
    pub async fn register_provider(&self, provider: Arc<dyn Provider>) -> Result<()> {
        let mut state = self.state.write().await;
        let name = provider.name().to_string();

        if state.provider_map.contains_key(&name) {
            return Err(SimpleAgentsError::Config(format!(
                "provider already registered: {}",
                name
            )));
        }

        state.provider_map.insert(name, provider.clone());
        state.providers.push(provider);
        state.router = Arc::new(self.routing_mode.build_router(state.providers.clone())?);
        Ok(())
    }

    /// Execute a completion request with routing, caching, and middleware.
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
        self.before_request(request).await?;

        let cache_key = if let Some(cache) = &self.cache {
            if cache.is_enabled() {
                Some(self.cache_key(request)?)
            } else {
                None
            }
        } else {
            None
        };

        if let (Some(cache), Some(key)) = (&self.cache, cache_key.as_deref()) {
            if let Some(cached) = cache.get(key).await? {
                let response: CompletionResponse = serde_json::from_slice(&cached)?;
                self.on_cache_hit(request, &response).await?;
                return Ok(response);
            }
        }

        let start = Instant::now();
        let router = {
            let state = self.state.read().await;
            state.router.clone()
        };
        let response = router.complete(request).await;

        match response {
            Ok(response) => {
                self.after_response(request, &response, start.elapsed())
                    .await?;
                if let (Some(cache), Some(key)) = (&self.cache, cache_key) {
                    let payload = serde_json::to_vec(&response)?;
                    cache.set(&key, payload, self.cache_ttl).await?;
                }
                Ok(response)
            }
            Err(error) => {
                self.on_error(request, &error, start.elapsed()).await?;
                Err(error)
            }
        }
    }

    /// Execute a completion request and parse the response content as JSON.
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

    /// Execute a completion request and coerce the response into a schema.
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

    /// Execute a streaming completion request.
    async fn stream(
        &self,
        request: &CompletionRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        request.validate()?;
        self.before_request(request).await?;
        debug!(
            model = %request.model,
            stream = ?request.stream,
            "SimpleAgentsClient.stream start"
        );

        let router = {
            let state = self.state.read().await;
            state.router.clone()
        };

        let start = Instant::now();
        let middleware = self.middleware.clone();
        let instrumented_request = request.clone();
        let inner = router.stream(request).await?;

        let wrapped = Self::instrument_stream(inner, instrumented_request, middleware, start);
        Ok(Box::new(wrapped))
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

    fn cache_key(&self, request: &CompletionRequest) -> Result<String> {
        let serialized = serde_json::to_string(request)?;
        Ok(CacheKey::from_parts("core", &request.model, &serialized))
    }

    async fn before_request(&self, request: &CompletionRequest) -> Result<()> {
        for middleware in &self.middleware {
            middleware.before_request(request).await?;
        }
        Ok(())
    }

    async fn after_response(
        &self,
        request: &CompletionRequest,
        response: &CompletionResponse,
        latency: Duration,
    ) -> Result<()> {
        for middleware in &self.middleware {
            middleware
                .after_response(request, response, latency)
                .await?;
        }
        Ok(())
    }

    async fn on_cache_hit(
        &self,
        request: &CompletionRequest,
        response: &CompletionResponse,
    ) -> Result<()> {
        for middleware in &self.middleware {
            middleware.on_cache_hit(request, response).await?;
        }
        Ok(())
    }

    async fn on_error(
        &self,
        request: &CompletionRequest,
        error: &SimpleAgentsError,
        latency: Duration,
    ) -> Result<()> {
        for middleware in &self.middleware {
            middleware.on_error(request, error, latency).await?;
        }
        Ok(())
    }
}

impl SimpleAgentsClient {
    fn instrument_stream(
        inner: Box<dyn Stream<Item = Result<CompletionChunk>> + Send + Unpin>,
        request: CompletionRequest,
        middleware: Vec<Arc<dyn Middleware>>,
        start: Instant,
    ) -> impl Stream<Item = Result<CompletionChunk>> + Send + Unpin {
        struct StreamState {
            inner: Box<dyn Stream<Item = Result<CompletionChunk>> + Send + Unpin>,
            middleware: Vec<Arc<dyn Middleware>>,
            request: CompletionRequest,
            start: Instant,
            done: bool,
        }

        stream::unfold(
            StreamState {
                inner,
                middleware,
                request,
                start,
                done: false,
            },
            |mut state| -> BoxFuture<Option<(Result<CompletionChunk>, StreamState)>> {
                Box::pin(async move {
                    if state.done {
                        return None;
                    }

                    match state.inner.next().await {
                        Some(Ok(chunk)) => Some((Ok(chunk), state)),
                        Some(Err(err)) => {
                            let latency = state.start.elapsed();
                            for middleware in &state.middleware {
                                if let Err(mw_err) =
                                    middleware.on_error(&state.request, &err, latency).await
                                {
                                    state.done = true;
                                    return Some((Err(mw_err), state));
                                }
                            }
                            state.done = true;
                            Some((Err(err), state))
                        }
                        None => {
                            let latency = state.start.elapsed();
                            for middleware in &state.middleware {
                                if let Err(mw_err) =
                                    middleware.after_stream(&state.request, latency).await
                                {
                                    state.done = true;
                                    return Some((Err(mw_err), state));
                                }
                            }
                            None
                        }
                    }
                })
            },
        )
    }
}

/// Builder for `SimpleAgentsClient`.
pub struct SimpleAgentsClientBuilder {
    providers: Vec<Arc<dyn Provider>>,
    routing_mode: RoutingMode,
    cache: Option<Arc<dyn Cache>>,
    cache_ttl: Duration,
    healing: HealingSettings,
    middleware: Vec<Arc<dyn Middleware>>,
}

impl SimpleAgentsClientBuilder {
    /// Create a new builder with defaults.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            routing_mode: RoutingMode::default(),
            cache: None,
            cache_ttl: Duration::from_secs(60),
            healing: HealingSettings::default(),
            middleware: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn with_provider(mut self, provider: Arc<dyn Provider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Register multiple providers at once.
    pub fn with_providers(mut self, providers: Vec<Arc<dyn Provider>>) -> Self {
        self.providers.extend(providers);
        self
    }

    /// Configure routing mode.
    pub fn with_routing_mode(mut self, mode: RoutingMode) -> Self {
        self.routing_mode = mode;
        self
    }

    /// Configure response cache.
    pub fn with_cache(mut self, cache: Arc<dyn Cache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Configure cache TTL.
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Configure healing settings.
    pub fn with_healing_settings(mut self, settings: HealingSettings) -> Self {
        self.healing = settings;
        self
    }

    /// Register a middleware hook.
    pub fn with_middleware(mut self, middleware: Arc<dyn Middleware>) -> Self {
        self.middleware.push(middleware);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<SimpleAgentsClient> {
        if self.providers.is_empty() {
            return Err(SimpleAgentsError::Config(
                "at least one provider is required".to_string(),
            ));
        }

        let mut seen = HashSet::new();
        for provider in &self.providers {
            let name = provider.name();
            if !seen.insert(name.to_string()) {
                return Err(SimpleAgentsError::Config(format!(
                    "duplicate provider configured in builder: {}",
                    name
                )));
            }
        }

        let provider_map = self
            .providers
            .iter()
            .map(|provider| (provider.name().to_string(), provider.clone()))
            .collect::<HashMap<_, _>>();

        let router = Arc::new(self.routing_mode.build_router(self.providers.clone())?);
        let state = ClientState {
            providers: self.providers,
            provider_map,
            router,
        };

        Ok(SimpleAgentsClient {
            state: RwLock::new(state),
            routing_mode: self.routing_mode,
            cache: self.cache,
            cache_ttl: self.cache_ttl,
            healing: self.healing,
            middleware: self.middleware,
        })
    }
}

impl Default for SimpleAgentsClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for () {
    async fn before_request(&self, _request: &CompletionRequest) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{stream, StreamExt};
    use simple_agent_type::error::ProviderError;
    use simple_agent_type::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

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
    async fn client_build_requires_provider() {
        let result = SimpleAgentsClientBuilder::new().build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn register_provider_rebuilds_router() {
        let provider = Arc::new(MockProvider::new("p1"));
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(provider)
            .build()
            .unwrap();

        let second = Arc::new(MockProvider::new("p2"));
        client.register_provider(second).await.unwrap();

        let names = client.provider_names().await.unwrap();
        assert!(names.contains(&"p1".to_string()));
        assert!(names.contains(&"p2".to_string()));
    }

    #[tokio::test]
    async fn duplicate_provider_registration_fails() {
        let provider = Arc::new(MockProvider::new("p1"));
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(provider.clone())
            .build()
            .unwrap();

        let result = client.register_provider(provider).await;
        assert!(matches!(
            result,
            Err(SimpleAgentsError::Config(msg)) if msg.contains("provider already registered")
        ));
    }

    #[tokio::test]
    async fn duplicate_provider_in_builder_with_provider_fails() {
        let p1 = Arc::new(MockProvider::new("p1"));
        let p1_dup = Arc::new(MockProvider::new("p1"));

        let result = SimpleAgentsClientBuilder::new()
            .with_provider(p1)
            .with_provider(p1_dup)
            .build();

        assert!(matches!(
            result,
            Err(SimpleAgentsError::Config(msg)) if msg.contains("duplicate provider configured in builder")
        ));
    }

    #[tokio::test]
    async fn duplicate_provider_in_builder_with_providers_fails() {
        let result = SimpleAgentsClientBuilder::new()
            .with_providers(vec![
                Arc::new(MockProvider::new("p1")),
                Arc::new(MockProvider::new("p1")),
            ])
            .build();

        assert!(matches!(
            result,
            Err(SimpleAgentsError::Config(msg)) if msg.contains("duplicate provider configured in builder")
        ));
    }

    #[derive(Default)]
    struct RecordingMiddleware {
        before: AtomicUsize,
        after_stream: AtomicUsize,
        errors: AtomicUsize,
    }

    #[async_trait]
    impl Middleware for RecordingMiddleware {
        async fn before_request(&self, _request: &CompletionRequest) -> Result<()> {
            self.before.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn after_stream(
            &self,
            _request: &CompletionRequest,
            _latency: Duration,
        ) -> Result<()> {
            self.after_stream.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn on_error(
            &self,
            _request: &CompletionRequest,
            _error: &SimpleAgentsError,
            _latency: Duration,
        ) -> Result<()> {
            self.errors.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn name(&self) -> &str {
            "recording"
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
                    },
                    finish_reason: None,
                }],
                created: None,
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
                stream::iter(items)
            } else {
                let items: Vec<Result<CompletionChunk>> =
                    vec![Ok(Self::build_chunk("chunk-1", "hello"))];
                stream::iter(items)
            };

            Ok(Box::new(stream))
        }
    }

    #[tokio::test]
    async fn streaming_invokes_after_stream_on_success() {
        let provider = Arc::new(StreamingProvider::new("p1", false));
        let middleware = Arc::new(RecordingMiddleware::default());

        let client = SimpleAgentsClientBuilder::new()
            .with_provider(provider)
            .with_middleware(middleware.clone())
            .build()
            .unwrap();

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
        assert_eq!(middleware.before.load(Ordering::Relaxed), 1);
        assert_eq!(middleware.after_stream.load(Ordering::Relaxed), 1);
        assert_eq!(middleware.errors.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn streaming_invokes_on_error_on_failure() {
        let provider = Arc::new(StreamingProvider::new("p1", true));
        let middleware = Arc::new(RecordingMiddleware::default());

        let client = SimpleAgentsClientBuilder::new()
            .with_provider(provider)
            .with_middleware(middleware.clone())
            .build()
            .unwrap();

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

        assert_eq!(middleware.before.load(Ordering::Relaxed), 1);
        assert_eq!(middleware.after_stream.load(Ordering::Relaxed), 0);
        assert_eq!(middleware.errors.load(Ordering::Relaxed), 1);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].as_ref().is_ok());
        assert!(chunks[1].is_err());
    }
}
