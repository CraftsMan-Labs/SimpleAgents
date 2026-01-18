//! Provider trait and types.
//!
//! Defines the interface for LLM providers with transformation hooks.

use crate::config::{Capabilities, RetryConfig};
use crate::error::{Result, SimpleAgentsError, ProviderError};
use crate::request::CompletionRequest;
use crate::response::{CompletionResponse, CompletionChunk};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::time::Duration;

/// Common HTTP header names (static to avoid allocations)
pub mod headers {
    /// Authorization header
    pub const AUTHORIZATION: &str = "Authorization";
    /// Content-Type header
    pub const CONTENT_TYPE: &str = "Content-Type";
    /// API key header (used by some providers like Anthropic)
    pub const X_API_KEY: &str = "x-api-key";
}

/// Trait for LLM providers.
///
/// Providers implement this trait to support different LLM APIs while
/// presenting a unified interface to the rest of SimpleAgents.
///
/// # Architecture
///
/// The provider trait follows a three-phase architecture:
/// 1. **Transform Request**: Convert unified request to provider format
/// 2. **Execute**: Make the actual API call
/// 3. **Transform Response**: Convert provider response to unified format
///
/// This design allows for:
/// - Maximum flexibility in provider-specific transformations
/// - Clean separation between protocol logic and business logic
/// - Easy testing and mocking
///
/// # Example Implementation
///
/// ```ignore
/// use simple_agents_types::provider::{Provider, ProviderRequest, ProviderResponse};
/// use simple_agents_types::request::CompletionRequest;
/// use simple_agents_types::response::CompletionResponse;
/// use simple_agents_types::error::Result;
/// use async_trait::async_trait;
///
/// struct MyProvider {
///     api_key: String,
/// }
///
/// #[async_trait]
/// impl Provider for MyProvider {
///     fn name(&self) -> &str {
///         "my-provider"
///     }
///
///     fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
///         // Transform to provider format
///         todo!()
///     }
///
///     async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
///         // Make HTTP request
///         todo!()
///     }
///
///     fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
///         // Transform from provider format
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g., "openai", "anthropic").
    fn name(&self) -> &str;

    /// Transform unified request to provider-specific format.
    ///
    /// This method converts the standardized `CompletionRequest` into
    /// the provider's native API format.
    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest>;

    /// Execute request against provider API.
    ///
    /// This method makes the actual HTTP request to the provider.
    /// Implementations should handle:
    /// - Authentication (API keys, tokens)
    /// - Rate limiting
    /// - Network errors
    /// - Provider-specific error codes
    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse>;

    /// Transform provider response to unified format.
    ///
    /// This method converts the provider's native response format into
    /// the standardized `CompletionResponse`.
    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse>;

    /// Get retry configuration.
    ///
    /// Override to customize retry behavior for this provider.
    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }

    /// Get provider capabilities.
    ///
    /// Override to specify what features this provider supports.
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    /// Get default timeout.
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    /// Execute streaming request against provider API.
    ///
    /// This method returns a stream of completion chunks for streaming responses.
    /// Not all providers support streaming - default implementation returns an error.
    ///
    /// # Arguments
    /// - `req`: The provider-specific request
    ///
    /// # Returns
    /// A boxed stream of Result<CompletionChunk>
    ///
    /// # Example
    /// ```ignore
    /// let stream = provider.execute_stream(request).await?;
    /// while let Some(chunk) = stream.next().await {
    ///     match chunk {
    ///         Ok(chunk) => println!("Received: {:?}", chunk),
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// ```
    async fn execute_stream(
        &self,
        _req: ProviderRequest,
    ) -> Result<Box<dyn futures_core::Stream<Item = Result<CompletionChunk>> + Send + Unpin>> {
        Err(SimpleAgentsError::Provider(
            ProviderError::UnsupportedFeature("streaming".to_string())
        ))
    }
}

/// Opaque provider-specific request.
///
/// This type encapsulates all information needed to make an HTTP request
/// to a provider, without committing to a specific HTTP client library.
///
/// Headers use `Cow<'static, str>` to avoid allocations for common headers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    /// Full URL to send request to
    pub url: String,
    /// HTTP headers (name, value pairs) using Cow to avoid allocations for static strings
    #[serde(with = "header_serde")]
    pub headers: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    /// Request body (JSON)
    pub body: serde_json::Value,
    /// Optional request timeout override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

// Custom serde for Cow headers
mod header_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::borrow::Cow;

    pub fn serialize<S>(
        headers: &[(Cow<'static, str>, Cow<'static, str>)],
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string_headers: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_ref(), v.as_ref()))
            .collect();
        string_headers.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Vec<(Cow<'static, str>, Cow<'static, str>)>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string_headers: Vec<(String, String)> = Vec::deserialize(deserializer)?;
        Ok(string_headers
            .into_iter()
            .map(|(k, v)| (Cow::Owned(k), Cow::Owned(v)))
            .collect())
    }
}

impl ProviderRequest {
    /// Create a new provider request.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
            body: serde_json::Value::Null,
            timeout: None,
        }
    }

    /// Add a header with owned strings.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((Cow::Owned(name.into()), Cow::Owned(value.into())));
        self
    }

    /// Add a header with static strings (zero allocation).
    pub fn with_static_header(mut self, name: &'static str, value: &'static str) -> Self {
        self.headers.push((Cow::Borrowed(name), Cow::Borrowed(value)));
        self
    }

    /// Set the body.
    pub fn with_body(mut self, body: serde_json::Value) -> Self {
        self.body = body;
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// Opaque provider-specific response.
///
/// This type encapsulates the raw HTTP response from a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// HTTP status code
    pub status: u16,
    /// Response body (JSON)
    pub body: serde_json::Value,
    /// Optional response headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,
}

impl ProviderResponse {
    /// Create a new provider response.
    pub fn new(status: u16, body: serde_json::Value) -> Self {
        Self {
            status,
            body,
            headers: None,
        }
    }

    /// Check if response is successful (2xx).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Check if response is a client error (4xx).
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status)
    }

    /// Check if response is a server error (5xx).
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status)
    }

    /// Add headers.
    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = Some(headers);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_request_builder() {
        let req = ProviderRequest::new("https://api.example.com/v1/completions")
            .with_header("Authorization", "Bearer sk-test")
            .with_header("Content-Type", "application/json")
            .with_body(serde_json::json!({"model": "test"}))
            .with_timeout(Duration::from_secs(30));

        assert_eq!(req.url, "https://api.example.com/v1/completions");
        assert_eq!(req.headers.len(), 2);
        assert_eq!(req.body["model"], "test");
        assert_eq!(req.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_provider_response_status_checks() {
        let resp = ProviderResponse::new(200, serde_json::json!({}));
        assert!(resp.is_success());
        assert!(!resp.is_client_error());
        assert!(!resp.is_server_error());

        let resp = ProviderResponse::new(404, serde_json::json!({}));
        assert!(!resp.is_success());
        assert!(resp.is_client_error());
        assert!(!resp.is_server_error());

        let resp = ProviderResponse::new(500, serde_json::json!({}));
        assert!(!resp.is_success());
        assert!(!resp.is_client_error());
        assert!(resp.is_server_error());
    }

    #[test]
    fn test_provider_request_serialization() {
        let req = ProviderRequest::new("https://api.example.com")
            .with_header("X-Test", "value")
            .with_body(serde_json::json!({"key": "value"}));

        let json = serde_json::to_string(&req).unwrap();
        let parsed: ProviderRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, parsed);
    }

    #[test]
    fn test_provider_response_serialization() {
        let resp = ProviderResponse::new(200, serde_json::json!({"result": "success"}))
            .with_headers(vec![("X-Request-ID".to_string(), "123".to_string())]);

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ProviderResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, parsed);
    }

    // Test that Provider trait is object-safe
    #[test]
    fn test_provider_object_safety() {
        fn _assert_object_safe(_: &dyn Provider) {}
    }
}
