use async_trait::async_trait;
use simple_agents_cache::InMemoryCache;
use simple_agents_core::{HealingSettings, Middleware, RoutingMode, SimpleAgentsClientBuilder};
use simple_agents_healing::schema::Schema;
use simple_agent_type::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct MockProvider {
    name: &'static str,
    content: &'static str,
    calls: AtomicUsize,
}

impl MockProvider {
    fn new(name: &'static str, content: &'static str) -> Self {
        Self {
            name,
            content,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
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
        Ok(ProviderResponse::new(200, serde_json::Value::Null))
    }

    fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
        Ok(CompletionResponse {
            id: "resp_test".to_string(),
            model: "test-model".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant(self.content),
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

struct TrackingMiddleware {
    before: AtomicUsize,
    after: AtomicUsize,
    cache_hits: AtomicUsize,
    errors: AtomicUsize,
    last_latency_ms: Mutex<Option<u128>>,
}

impl TrackingMiddleware {
    fn new() -> Self {
        Self {
            before: AtomicUsize::new(0),
            after: AtomicUsize::new(0),
            cache_hits: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
            last_latency_ms: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Middleware for TrackingMiddleware {
    async fn before_request(&self, _request: &CompletionRequest) -> Result<()> {
        self.before.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn after_response(
        &self,
        _request: &CompletionRequest,
        _response: &CompletionResponse,
        latency: Duration,
    ) -> Result<()> {
        self.after.fetch_add(1, Ordering::Relaxed);
        let mut lock = self
            .last_latency_ms
            .lock()
            .map_err(|_| SimpleAgentsError::Config("latency lock poisoned".to_string()))?;
        *lock = Some(latency.as_millis());
        Ok(())
    }

    async fn on_cache_hit(
        &self,
        _request: &CompletionRequest,
        _response: &CompletionResponse,
    ) -> Result<()> {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
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
}

fn build_request() -> CompletionRequest {
    CompletionRequest::builder()
        .model("test-model")
        .message(Message::user("hello"))
        .build()
        .unwrap()
}

#[tokio::test]
async fn complete_uses_cache() {
    let provider = Arc::new(MockProvider::new("p1", "ok"));
    let cache = Arc::new(InMemoryCache::new(1024 * 1024, 10));

    let client = SimpleAgentsClientBuilder::new()
        .with_provider(provider.clone())
        .with_cache(cache)
        .with_cache_ttl(Duration::from_secs(60))
        .build()
        .unwrap();

    let request = build_request();
    let first = client.complete(&request).await.unwrap();
    let second = client.complete(&request).await.unwrap();

    assert_eq!(first.content(), Some("ok"));
    assert_eq!(second.content(), Some("ok"));
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn complete_json_parses_markdown() {
    let provider = Arc::new(MockProvider::new("p1", "```json\n{\"value\": 42,}\n```"));
    let client = SimpleAgentsClientBuilder::new()
        .with_provider(provider)
        .with_healing_settings(HealingSettings::default())
        .build()
        .unwrap();

    let healed = client.complete_json(&build_request()).await.unwrap();
    assert_eq!(healed.parsed.value["value"], 42);
}

#[tokio::test]
async fn complete_with_schema_coerces_types() {
    let provider = Arc::new(MockProvider::new("p1", "{\"count\": \"5\"}"));
    let client = SimpleAgentsClientBuilder::new()
        .with_provider(provider)
        .with_healing_settings(HealingSettings::default())
        .build()
        .unwrap();

    let schema = Schema::object(vec![("count".into(), Schema::Int, true)]);
    let healed = client
        .complete_with_schema(&build_request(), &schema)
        .await
        .unwrap();

    assert_eq!(healed.coerced.value["count"], 5);
}

#[tokio::test]
async fn middleware_hooks_fire() {
    let provider = Arc::new(MockProvider::new("p1", "ok"));
    let middleware = Arc::new(TrackingMiddleware::new());

    let client = SimpleAgentsClientBuilder::new()
        .with_provider(provider)
        .with_routing_mode(RoutingMode::RoundRobin)
        .with_middleware(middleware.clone())
        .build()
        .unwrap();

    let _ = client.complete(&build_request()).await.unwrap();

    assert_eq!(middleware.before.load(Ordering::Relaxed), 1);
    assert_eq!(middleware.after.load(Ordering::Relaxed), 1);
    assert_eq!(middleware.cache_hits.load(Ordering::Relaxed), 0);
}
