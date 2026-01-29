//! Round-robin routing implementation.
//!
//! Distributes requests evenly across configured providers.

use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, Result, SimpleAgentsError,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Router that selects providers using round-robin order.
pub struct RoundRobinRouter {
    providers: Vec<Arc<dyn Provider>>,
    counter: AtomicUsize,
}

impl RoundRobinRouter {
    /// Create a new round-robin router.
    ///
    /// # Errors
    /// Returns a routing error if no providers are supplied.
    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Result<Self> {
        if providers.is_empty() {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        Ok(Self {
            providers,
            counter: AtomicUsize::new(0),
        })
    }

    /// Return the number of configured providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Execute a completion request using round-robin provider selection.
    pub async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let index = self.select_provider_index()?;
        let provider = &self.providers[index];
        let provider_request = provider.transform_request(request)?;
        let provider_response = provider.execute(provider_request).await?;
        provider.transform_response(provider_response)
    }

    /// Execute a streaming request using round-robin provider selection.
    pub async fn stream(
        &self,
        request: &CompletionRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        let index = self.select_provider_index()?;
        let provider = &self.providers[index];
        eprintln!(
            "RoundRobinRouter.stream: provider={}, stream={:?}",
            provider.name(),
            request.stream
        );
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

        let index = self.counter.fetch_add(1, Ordering::Relaxed);
        Ok(index % len)
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
        let result = RoundRobinRouter::new(Vec::new());
        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(SimpleAgentsError::Routing(message)) => {
                assert_eq!(message, "no providers configured");
            }
            Err(_) => panic!("unexpected error type"),
        }
    }

    #[tokio::test]
    async fn round_robin_rotates_providers() {
        let router = RoundRobinRouter::new(vec![
            Arc::new(MockProvider::new("p1")),
            Arc::new(MockProvider::new("p2")),
        ])
        .unwrap();

        let request = build_request();
        let first = router.complete(&request).await.unwrap();
        let second = router.complete(&request).await.unwrap();
        let third = router.complete(&request).await.unwrap();

        assert_eq!(first.provider.as_deref(), Some("p1"));
        assert_eq!(second.provider.as_deref(), Some("p2"));
        assert_eq!(third.provider.as_deref(), Some("p1"));
    }
}
