//! Latency-based routing implementation.
//!
//! Routes requests to provider with lowest observed latency.

use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, ProviderHealth, Result, SimpleAgentsError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for latency-based routing.
#[derive(Debug, Clone)]
pub struct LatencyRouterConfig {
    /// Exponential moving average factor (0.0-1.0).
    pub alpha: f64,
    /// Threshold after which providers are marked degraded.
    pub slow_threshold: Duration,
}

impl Default for LatencyRouterConfig {
    fn default() -> Self {
        Self {
            alpha: 0.2,
            slow_threshold: Duration::from_secs(2),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LatencyStats {
    avg_latency_ms: f64,
    samples: u64,
    health: ProviderHealth,
}

impl LatencyStats {
    fn new() -> Self {
        Self {
            avg_latency_ms: 0.0,
            samples: 0,
            health: ProviderHealth::Healthy,
        }
    }

    fn record(&mut self, latency: Duration, alpha: f64, slow_threshold: Duration) {
        let latency_ms = latency.as_secs_f64() * 1000.0;
        if self.samples == 0 {
            self.avg_latency_ms = latency_ms;
        } else {
            let previous = self.avg_latency_ms;
            self.avg_latency_ms = (alpha * latency_ms) + ((1.0 - alpha) * previous);
        }
        self.samples = self.samples.saturating_add(1);

        let threshold_ms = slow_threshold.as_secs_f64() * 1000.0;
        self.health = if self.avg_latency_ms >= threshold_ms {
            ProviderHealth::Degraded
        } else {
            ProviderHealth::Healthy
        };
    }
}

/// Router that selects providers based on observed latency.
pub struct LatencyRouter {
    providers: Vec<Arc<dyn Provider>>,
    stats: Mutex<Vec<LatencyStats>>,
    counter: AtomicUsize,
    config: LatencyRouterConfig,
}

impl LatencyRouter {
    /// Create a latency router with default configuration.
    ///
    /// # Errors
    /// Returns a routing error if no providers are supplied.
    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Result<Self> {
        Self::with_config(providers, LatencyRouterConfig::default())
    }

    /// Create a latency router with explicit configuration.
    ///
    /// # Errors
    /// Returns a routing error if no providers are supplied.
    pub fn with_config(
        providers: Vec<Arc<dyn Provider>>,
        config: LatencyRouterConfig,
    ) -> Result<Self> {
        if providers.is_empty() {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        let stats = vec![LatencyStats::new(); providers.len()];
        Ok(Self {
            providers,
            stats: Mutex::new(stats),
            counter: AtomicUsize::new(0),
            config,
        })
    }

    /// Return the number of configured providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Execute a completion request using latency-based selection.
    pub async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let index = self.select_provider_index()?;
        let provider = &self.providers[index];
        let start = Instant::now();
        let provider_request = provider.transform_request(request)?;
        let provider_response = provider.execute(provider_request).await?;
        let response = provider.transform_response(provider_response)?;
        self.record_latency(index, start.elapsed());
        Ok(response)
    }

    /// Execute a streaming request using latency-based selection.
    pub async fn stream(
        &self,
        request: &CompletionRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        let index = self.select_provider_index()?;
        let provider = &self.providers[index];
        let provider_request = provider.transform_request(request)?;
        provider.execute_stream(provider_request).await
    }

    fn select_provider_index(&self) -> Result<usize> {
        let len = self.providers.len();
        if len == 0 {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        let stats = self.stats.lock().expect("latency stats lock poisoned");
        let mut best_index: Option<usize> = None;
        let mut best_latency = f64::MAX;
        let mut has_samples = false;
        let mut has_healthy = false;

        for stat in stats.iter() {
            if stat.samples == 0 {
                continue;
            }
            has_samples = true;
            if stat.health == ProviderHealth::Healthy {
                has_healthy = true;
            }
        }

        if has_samples {
            for (index, stat) in stats.iter().enumerate() {
                if stat.samples == 0 {
                    continue;
                }
                if has_healthy && stat.health != ProviderHealth::Healthy {
                    continue;
                }
                if stat.avg_latency_ms < best_latency {
                    best_latency = stat.avg_latency_ms;
                    best_index = Some(index);
                }
            }
        }

        if let Some(index) = best_index {
            return Ok(index);
        }

        let index = self.counter.fetch_add(1, Ordering::Relaxed);
        Ok(index % len)
    }

    fn record_latency(&self, index: usize, latency: Duration) {
        let mut stats = self.stats.lock().expect("latency stats lock poisoned");
        if let Some(stat) = stats.get_mut(index) {
            stat.record(latency, self.config.alpha, self.config.slow_threshold);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use simple_agent_type::prelude::*;

    struct MockProvider {
        name: &'static str,
    }

    impl MockProvider {
        fn new(name: &'static str) -> Self {
            Self { name }
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
            Ok(ProviderResponse::new(200, serde_json::Value::Null))
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
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            })
        }
    }

    fn build_request() -> CompletionRequest {
        CompletionRequest::builder()
            .model("test-model")
            .message(Message::user("hello"))
            .build()
            .unwrap()
    }

    #[test]
    fn empty_router_returns_error() {
        let result = LatencyRouter::new(Vec::new());
        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(SimpleAgentsError::Routing(message)) => {
                assert_eq!(message, "no providers configured");
            }
            Err(_) => panic!("unexpected error type"),
        }
    }

    #[test]
    fn selects_lowest_latency_provider() {
        let router = LatencyRouter::new(vec![
            Arc::new(MockProvider::new("p1")),
            Arc::new(MockProvider::new("p2")),
        ])
        .unwrap();

        router.record_latency(0, Duration::from_millis(250));
        router.record_latency(1, Duration::from_millis(50));

        let index = router.select_provider_index().unwrap();
        assert_eq!(index, 1);
    }

    #[test]
    fn prefers_healthy_over_degraded() {
        let config = LatencyRouterConfig {
            alpha: 1.0,
            slow_threshold: Duration::from_millis(100),
        };
        let router = LatencyRouter::with_config(
            vec![
                Arc::new(MockProvider::new("p1")),
                Arc::new(MockProvider::new("p2")),
            ],
            config,
        )
        .unwrap();

        router.record_latency(0, Duration::from_millis(400));
        router.record_latency(1, Duration::from_millis(80));

        let index = router.select_provider_index().unwrap();
        assert_eq!(index, 1);
    }

    #[test]
    fn round_robin_when_no_metrics() {
        let router = LatencyRouter::new(vec![
            Arc::new(MockProvider::new("p1")),
            Arc::new(MockProvider::new("p2")),
        ])
        .unwrap();

        let first = router.select_provider_index().unwrap();
        let second = router.select_provider_index().unwrap();

        assert_eq!(first, 0);
        assert_eq!(second, 1);
    }

    #[tokio::test]
    async fn records_latency_on_success() {
        let router = LatencyRouter::new(vec![Arc::new(MockProvider::new("p1"))]).unwrap();
        let request = build_request();

        let _ = router.complete(&request).await.unwrap();
        let stats = router.stats.lock().expect("latency stats lock poisoned");
        assert_eq!(stats[0].samples, 1);
    }
}
