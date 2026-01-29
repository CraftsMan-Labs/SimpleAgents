//! SimpleAgents client implementation.

use crate::healing::{HealedJsonResponse, HealedSchemaResponse, HealingSettings};
use crate::middleware::Middleware;
use crate::routing::{RouterEngine, RoutingMode};
use async_trait::async_trait;
use simple_agent_type::cache::Cache;
use simple_agents_healing::coercion::CoercionEngine;
use simple_agents_healing::parser::JsonishParser;
use simple_agents_healing::schema::Schema;
use simple_agent_type::cache::CacheKey;
use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, Result, SimpleAgentsError,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

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
    pub fn provider_names(&self) -> Result<Vec<String>> {
        let state = self.state.read().map_err(|_| {
            SimpleAgentsError::Config("provider registry lock poisoned".to_string())
        })?;
        Ok(state.provider_map.keys().cloned().collect())
    }

    /// Retrieve a provider by name.
    pub fn provider(&self, name: &str) -> Result<Option<Arc<dyn Provider>>> {
        let state = self.state.read().map_err(|_| {
            SimpleAgentsError::Config("provider registry lock poisoned".to_string())
        })?;
        Ok(state.provider_map.get(name).cloned())
    }

    /// Register an additional provider and rebuild the router.
    pub fn register_provider(&self, provider: Arc<dyn Provider>) -> Result<()> {
        let mut state = self.state.write().map_err(|_| {
            SimpleAgentsError::Config("provider registry lock poisoned".to_string())
        })?;
        state
            .provider_map
            .insert(provider.name().to_string(), provider.clone());
        state.providers.push(provider);
        state.router = Arc::new(self.routing_mode.build_router(state.providers.clone())?);
        Ok(())
    }

    /// Execute a completion request with routing, caching, and middleware.
    pub async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
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
            let state = self.state.read().map_err(|_| {
                SimpleAgentsError::Config("provider registry lock poisoned".to_string())
            })?;
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
    pub async fn complete_json(&self, request: &CompletionRequest) -> Result<HealedJsonResponse> {
        self.ensure_healing_enabled()?;
        let response = self.complete(request).await?;
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
    pub async fn complete_with_schema(
        &self,
        request: &CompletionRequest,
        schema: &Schema,
    ) -> Result<HealedSchemaResponse> {
        self.ensure_healing_enabled()?;
        let healed = self.complete_json(request).await?;
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
    pub async fn stream(
        &self,
        request: &CompletionRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        request.validate()?;
        self.before_request(request).await?;
        eprintln!(
            "SimpleAgentsClient.stream: model={}, stream={:?}",
            request.model, request.stream
        );

        let router = {
            let state = self.state.read().map_err(|_| {
                SimpleAgentsError::Config("provider registry lock poisoned".to_string())
            })?;
            state.router.clone()
        };

        router.stream(request).await
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
        client.register_provider(second).unwrap();

        let names = client.provider_names().unwrap();
        assert!(names.contains(&"p1".to_string()));
        assert!(names.contains(&"p2".to_string()));
    }
}
