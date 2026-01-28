//! Routing integration for SimpleAgents core.

use simple_agents_router::{
    CostRouter, CostRouterConfig, FallbackRouter, FallbackRouterConfig, LatencyRouter,
    LatencyRouterConfig, RoundRobinRouter,
};
use simple_agents_types::prelude::{
    CompletionRequest, CompletionResponse, Provider, Result, SimpleAgentsError,
};
use std::sync::Arc;

/// Routing modes supported by the core client.
#[derive(Debug, Clone, Default)]
pub enum RoutingMode {
    /// Direct execution against the first provider.
    Direct,
    /// Round-robin routing across providers.
    #[default]
    RoundRobin,
    /// Latency-based routing with configurable smoothing.
    Latency(LatencyRouterConfig),
    /// Cost-based routing with per-provider costs.
    Cost(CostRouterConfig),
    /// Fallback routing (try providers in order).
    Fallback(FallbackRouterConfig),
}

pub(crate) enum RouterEngine {
    Direct(Arc<dyn Provider>),
    RoundRobin(RoundRobinRouter),
    Latency(LatencyRouter),
    Cost(CostRouter),
    Fallback(FallbackRouter),
}

impl RouterEngine {
    pub(crate) async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        match self {
            Self::Direct(provider) => {
                let provider_request = provider.transform_request(request)?;
                let provider_response = provider.execute(provider_request).await?;
                provider.transform_response(provider_response)
            }
            Self::RoundRobin(router) => router.complete(request).await,
            Self::Latency(router) => router.complete(request).await,
            Self::Cost(router) => router.complete(request).await,
            Self::Fallback(router) => router.complete(request).await,
        }
    }
}

impl RoutingMode {
    pub(crate) fn build_router(&self, providers: Vec<Arc<dyn Provider>>) -> Result<RouterEngine> {
        if providers.is_empty() {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        match self {
            RoutingMode::Direct => Ok(RouterEngine::Direct(
                providers
                    .first()
                    .ok_or_else(|| {
                        SimpleAgentsError::Routing("no providers configured".to_string())
                    })?
                    .clone(),
            )),
            RoutingMode::RoundRobin => {
                Ok(RouterEngine::RoundRobin(RoundRobinRouter::new(providers)?))
            }
            RoutingMode::Latency(config) => Ok(RouterEngine::Latency(LatencyRouter::with_config(
                providers,
                config.clone(),
            )?)),
            RoutingMode::Cost(config) => Ok(RouterEngine::Cost(CostRouter::new(
                providers,
                config.clone(),
            )?)),
            RoutingMode::Fallback(config) => Ok(RouterEngine::Fallback(
                FallbackRouter::with_config(providers, *config)?,
            )),
        }
    }
}
