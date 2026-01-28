//! Middleware hooks for SimpleAgents core.

use async_trait::async_trait;
use simple_agent_type::prelude::{
    CompletionRequest, CompletionResponse, Result, SimpleAgentsError,
};
use std::time::Duration;

/// Middleware hooks for request lifecycle events.
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before a request is executed.
    async fn before_request(&self, _request: &CompletionRequest) -> Result<()> {
        Ok(())
    }

    /// Called after a response is received.
    async fn after_response(
        &self,
        _request: &CompletionRequest,
        _response: &CompletionResponse,
        _latency: Duration,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when a response is served from cache.
    async fn on_cache_hit(
        &self,
        _request: &CompletionRequest,
        _response: &CompletionResponse,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when a request fails.
    async fn on_error(
        &self,
        _request: &CompletionRequest,
        _error: &SimpleAgentsError,
        _latency: Duration,
    ) -> Result<()> {
        Ok(())
    }

    /// Middleware name for diagnostics.
    fn name(&self) -> &str {
        "middleware"
    }
}
