//! Fallback routing implementation.
//!
//! Attempts providers in order, falling back on retryable errors.

use simple_agent_type::prelude::{
    CompletionChunk, CompletionRequest, CompletionResponse, Provider, ProviderError, Result, SimpleAgentsError,
};
use std::sync::Arc;

/// Configuration for fallback routing.
#[derive(Debug, Clone, Copy)]
pub struct FallbackRouterConfig {
    /// If true, fallback only on retryable provider errors.
    pub retryable_only: bool,
}

impl Default for FallbackRouterConfig {
    fn default() -> Self {
        Self {
            retryable_only: true,
        }
    }
}

/// Router that tries providers in order and falls back on eligible errors.
pub struct FallbackRouter {
    providers: Vec<Arc<dyn Provider>>,
    config: FallbackRouterConfig,
}

impl FallbackRouter {
    /// Create a new fallback router.
    ///
    /// # Errors
    /// Returns a routing error if no providers are supplied.
    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Result<Self> {
        Self::with_config(providers, FallbackRouterConfig::default())
    }

    /// Create a new fallback router with custom configuration.
    ///
    /// # Errors
    /// Returns a routing error if no providers are supplied.
    pub fn with_config(
        providers: Vec<Arc<dyn Provider>>,
        config: FallbackRouterConfig,
    ) -> Result<Self> {
        if providers.is_empty() {
            return Err(SimpleAgentsError::Routing(
                "no providers configured".to_string(),
            ));
        }

        Ok(Self { providers, config })
    }

    /// Return the number of configured providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Execute a completion request with fallback logic.
    pub async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let mut last_error: Option<SimpleAgentsError> = None;

        for provider in &self.providers {
            let attempt = self.execute_provider(provider, request).await;
            match attempt {
                Ok(response) => return Ok(response),
                Err(err) => {
                    if !self.should_fallback(&err) {
                        return Err(err);
                    }
                    last_error = Some(err);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| SimpleAgentsError::Routing("no providers configured".to_string())))
    }

    /// Execute a streaming request with fallback logic.
    pub async fn stream(
        &self,
        request: &CompletionRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        for provider in &self.providers {
            let provider_request = provider.transform_request(request)?;
            match provider.execute_stream(provider_request).await {
                Ok(stream) => return Ok(stream),
                Err(err) => {
                    if !self.should_fallback(&err) {
                        return Err(err);
                    }
                    // Continue to next provider
                }
            }
        }

        Err(SimpleAgentsError::Routing("no providers configured".to_string()))
    }

    async fn execute_provider(
        &self,
        provider: &Arc<dyn Provider>,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse> {
        let provider_request = provider.transform_request(request)?;
        let provider_response = provider.execute(provider_request).await?;
        provider.transform_response(provider_response)
    }

    fn should_fallback(&self, error: &SimpleAgentsError) -> bool {
        if !self.config.retryable_only {
            return true;
        }

        matches!(
            error,
            SimpleAgentsError::Provider(
                ProviderError::RateLimit { .. }
                    | ProviderError::Timeout(_)
                    | ProviderError::ServerError(_)
            ) | SimpleAgentsError::Network(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use simple_agent_type::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockProvider {
        name: &'static str,
        attempts: AtomicUsize,
        result: MockResult,
    }

    enum MockResult {
        Ok,
        RetryableError,
        NonRetryableError,
    }

    impl MockProvider {
        fn new(name: &'static str, result: MockResult) -> Self {
            Self {
                name,
                attempts: AtomicUsize::new(0),
                result,
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
            self.attempts.fetch_add(1, Ordering::Relaxed);
            match self.result {
                MockResult::Ok => Ok(ProviderResponse::new(200, serde_json::Value::Null)),
                MockResult::RetryableError => Err(SimpleAgentsError::Provider(
                    ProviderError::Timeout(std::time::Duration::from_secs(1)),
                )),
                MockResult::NonRetryableError => {
                    Err(SimpleAgentsError::Provider(ProviderError::InvalidApiKey))
                }
            }
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
        let result = FallbackRouter::new(Vec::new());
        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(SimpleAgentsError::Routing(message)) => {
                assert_eq!(message, "no providers configured");
            }
            Err(_) => panic!("unexpected error type"),
        }
    }

    #[tokio::test]
    async fn falls_back_on_retryable_error() {
        let router = FallbackRouter::new(vec![
            Arc::new(MockProvider::new("p1", MockResult::RetryableError)),
            Arc::new(MockProvider::new("p2", MockResult::Ok)),
        ])
        .unwrap();

        let response = router.complete(&build_request()).await.unwrap();
        assert_eq!(response.provider.as_deref(), Some("p2"));
    }

    #[tokio::test]
    async fn stops_on_non_retryable_error() {
        let router = FallbackRouter::new(vec![
            Arc::new(MockProvider::new("p1", MockResult::NonRetryableError)),
            Arc::new(MockProvider::new("p2", MockResult::Ok)),
        ])
        .unwrap();

        let err = router.complete(&build_request()).await.unwrap_err();
        match err {
            SimpleAgentsError::Provider(ProviderError::InvalidApiKey) => {}
            _ => panic!("unexpected error"),
        }
    }

    #[tokio::test]
    async fn falls_back_on_all_errors_when_configured() {
        let config = FallbackRouterConfig {
            retryable_only: false,
        };
        let router = FallbackRouter::with_config(
            vec![
                Arc::new(MockProvider::new("p1", MockResult::NonRetryableError)),
                Arc::new(MockProvider::new("p2", MockResult::Ok)),
            ],
            config,
        )
        .unwrap();

        let response = router.complete(&build_request()).await.unwrap();
        assert_eq!(response.provider.as_deref(), Some("p2"));
    }
}
