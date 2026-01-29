//! Cost-based routing implementation.
//!
//! Routes requests to lowest-cost provider.

use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, Result, SimpleAgentsError,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Cost metadata for a provider.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderCost {
    /// Provider name that matches `Provider::name()`.
    pub name: String,
    /// Cost per 1k tokens.
    pub cost_per_1k_tokens: f64,
}

impl ProviderCost {
    /// Create a new provider cost entry.
    pub fn new(name: impl Into<String>, cost_per_1k_tokens: f64) -> Result<Self> {
        if !cost_per_1k_tokens.is_finite() || cost_per_1k_tokens < 0.0 {
            return Err(SimpleAgentsError::Routing(
                "provider cost must be a non-negative finite value".to_string(),
            ));
        }

        Ok(Self {
            name: name.into(),
            cost_per_1k_tokens,
        })
    }
}

/// Configuration for cost-based routing.
#[derive(Debug, Clone, Default)]
pub struct CostRouterConfig {
    /// Provider costs.
    pub costs: Vec<ProviderCost>,
}

impl CostRouterConfig {
    /// Create a config from a list of provider costs.
    pub fn new(costs: Vec<ProviderCost>) -> Self {
        Self { costs }
    }
}

/// Router that selects providers based on lowest cost.
pub struct CostRouter {
    providers: Vec<Arc<dyn Provider>>,
    costs: Vec<f64>,
    counter: AtomicUsize,
}

impl CostRouter {
    /// Create a cost router using the provided config.
    ///
    /// # Errors
    /// Returns a routing error if providers or costs are missing.
    pub fn new(providers: Vec<Arc<dyn Provider>>, config: CostRouterConfig) -> Result<Self> {
        if providers.is_empty() {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        let mut cost_map = HashMap::new();
        for cost in config.costs {
            if !cost.cost_per_1k_tokens.is_finite() || cost.cost_per_1k_tokens < 0.0 {
                return Err(SimpleAgentsError::Routing(
                    "provider cost must be a non-negative finite value".to_string(),
                ));
            }
            cost_map.insert(cost.name, cost.cost_per_1k_tokens);
        }

        let mut costs = Vec::with_capacity(providers.len());
        for provider in &providers {
            let name = provider.name();
            match cost_map.get(name) {
                Some(cost) => costs.push(*cost),
                None => {
                    return Err(SimpleAgentsError::Routing(format!(
                        "missing cost for provider: {}",
                        name
                    )));
                }
            }
        }

        Ok(Self {
            providers,
            costs,
            counter: AtomicUsize::new(0),
        })
    }

    /// Execute a completion request using cost-based selection.
    pub async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let index = self.select_provider_index()?;
        let provider = &self.providers[index];
        let provider_request = provider.transform_request(request)?;
        let provider_response = provider.execute(provider_request).await?;
        provider.transform_response(provider_response)
    }

    /// Execute a streaming request using cost-based selection.
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
        if self.providers.is_empty() {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        let mut min_cost = f64::INFINITY;
        for cost in &self.costs {
            if *cost < min_cost {
                min_cost = *cost;
            }
        }

        if !min_cost.is_finite() {
            return Err(SimpleAgentsError::Routing(
                "invalid provider costs".to_string(),
            ));
        }

        let min_indices: Vec<usize> = self
            .costs
            .iter()
            .enumerate()
            .filter(|(_, cost)| **cost == min_cost)
            .map(|(index, _)| index)
            .collect();

        if min_indices.is_empty() {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        let offset = self.counter.fetch_add(1, Ordering::Relaxed);
        Ok(min_indices[offset % min_indices.len()])
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

    fn build_costs(entries: Vec<ProviderCost>) -> CostRouterConfig {
        CostRouterConfig::new(entries)
    }

    #[test]
    fn empty_router_returns_error() {
        let config = build_costs(vec![ProviderCost::new("p1", 0.5).unwrap()]);
        let result = CostRouter::new(Vec::new(), config);
        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(SimpleAgentsError::Routing(message)) => {
                assert_eq!(message, "no providers configured");
            }
            Err(_) => panic!("unexpected error type"),
        }
    }

    #[test]
    fn missing_cost_returns_error() {
        let config = build_costs(vec![ProviderCost::new("p1", 0.5).unwrap()]);
        let result = CostRouter::new(
            vec![
                Arc::new(MockProvider::new("p1")),
                Arc::new(MockProvider::new("p2")),
            ],
            config,
        );

        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(SimpleAgentsError::Routing(message)) => {
                assert_eq!(message, "missing cost for provider: p2");
            }
            Err(_) => panic!("unexpected error type"),
        }
    }

    #[test]
    fn selects_lowest_cost() {
        let config = build_costs(vec![
            ProviderCost::new("p1", 0.8).unwrap(),
            ProviderCost::new("p2", 0.2).unwrap(),
        ]);
        let router = CostRouter::new(
            vec![
                Arc::new(MockProvider::new("p1")),
                Arc::new(MockProvider::new("p2")),
            ],
            config,
        )
        .unwrap();

        let index = router.select_provider_index().unwrap();
        assert_eq!(index, 1);
    }

    #[test]
    fn tie_breaks_with_rotation() {
        let config = build_costs(vec![
            ProviderCost::new("p1", 0.5).unwrap(),
            ProviderCost::new("p2", 0.5).unwrap(),
            ProviderCost::new("p3", 0.8).unwrap(),
        ]);
        let router = CostRouter::new(
            vec![
                Arc::new(MockProvider::new("p1")),
                Arc::new(MockProvider::new("p2")),
                Arc::new(MockProvider::new("p3")),
            ],
            config,
        )
        .unwrap();

        let first = router.select_provider_index().unwrap();
        let second = router.select_provider_index().unwrap();

        assert_eq!(first, 0);
        assert_eq!(second, 1);
    }
}
