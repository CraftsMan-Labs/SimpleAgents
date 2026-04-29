//! HTTP client wrapper with connection pooling and protocol negotiation.
//!
//! This module provides a configured HTTP client optimized for LLM API calls:
//! - Connection pooling (max 10 idle connections per host)
//! - HTTP protocol negotiated per endpoint
//! - Configurable timeouts
//! - Idle connection timeout (90 seconds)

use reqwest::Client;
use std::time::Duration;

use crate::utils::DEFAULT_TIMEOUT;

/// HTTP client wrapper with optimized configuration.
///
/// # Configuration
///
/// - **Timeout**: 30 seconds default (configurable)
/// - **Connection Pooling**: Max 10 idle connections per host
/// - **Idle Timeout**: 90 seconds before connection cleanup
/// - **HTTP/2**: Negotiated when supported by the endpoint
///
/// # Examples
///
/// ```
/// use simple_agents_providers::common::HttpClient;
/// use std::time::Duration;
///
/// let client = HttpClient::new().unwrap();
/// let custom_client = HttpClient::with_timeout(Duration::from_secs(60)).unwrap();
/// ```
#[derive(Clone, Debug)]
pub struct HttpClient {
    inner: Client,
}

impl HttpClient {
    /// Creates a new HTTP client with default configuration.
    ///
    /// Default configuration:
    /// - 30 second timeout
    /// - HTTP protocol negotiation enabled
    /// - Connection pooling (10 idle per host, 90s timeout)
    ///
    /// # Errors
    ///
    /// Returns error if the client fails to build (rare, usually system-level issues).
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// Creates a new HTTP client with custom timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Request timeout duration
    ///
    /// # Errors
    ///
    /// Returns error if the client fails to build.
    ///
    /// # Examples
    ///
    /// ```
    /// use simple_agents_providers::common::HttpClient;
    /// use std::time::Duration;
    ///
    /// let client = HttpClient::with_timeout(Duration::from_secs(60)).unwrap();
    /// ```
    pub fn with_timeout(timeout: Duration) -> Result<Self, reqwest::Error> {
        Self::with_timeout_and_no_proxy(timeout, false)
    }

    /// Creates a new HTTP client with custom timeout and optional proxy bypass.
    ///
    /// The proxy bypass is used for local OpenAI-compatible test servers where
    /// system proxy settings can otherwise route localhost traffic incorrectly.
    ///
    /// # Errors
    ///
    /// Returns error if the client fails to build.
    pub fn with_timeout_and_no_proxy(
        timeout: Duration,
        no_proxy: bool,
    ) -> Result<Self, reqwest::Error> {
        let inner = Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90));
        let inner = if no_proxy { inner.no_proxy() } else { inner }.build()?;

        Ok(Self { inner })
    }

    /// Gets a reference to the underlying reqwest client.
    ///
    /// Useful for making custom requests while maintaining connection pooling.
    pub fn inner(&self) -> &Client {
        &self.inner
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::default_with_primary_builder(Self::new)
    }
}

impl HttpClient {
    fn default_with_primary_builder<F>(primary_builder: F) -> Self
    where
        F: FnOnce() -> Result<Self, reqwest::Error>,
    {
        match primary_builder() {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "Falling back to minimal timeout HTTP client configuration"
                );
                let fallback = Client::builder().timeout(DEFAULT_TIMEOUT).build();
                match fallback {
                    Ok(inner) => Self { inner },
                    Err(fallback_error) => {
                        tracing::warn!(
                            ?fallback_error,
                            "Fallback HTTP client build failed; using reqwest default client"
                        );
                        Self {
                            inner: Client::new(),
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_creation() {
        let client = HttpClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_http_client_with_custom_timeout() {
        let client = HttpClient::with_timeout(Duration::from_secs(60));
        assert!(client.is_ok());
    }

    #[test]
    fn test_http_client_default() {
        let client = HttpClient::default();
        // Verify we can get the inner client
        let _ = client.inner();
    }

    #[test]
    fn test_http_client_clone() {
        let client = HttpClient::new().unwrap();
        let cloned = client.clone();
        // Both should work - verify we can get inner clients
        let _ = client.inner();
        let _ = cloned.inner();
    }

    #[test]
    fn test_default_fallback_path_when_primary_builder_fails() {
        let client = HttpClient::default_with_primary_builder(|| {
            let error = reqwest::Client::builder()
                .user_agent("\n")
                .build()
                .expect_err("invalid user-agent should fail client build");
            Err(error)
        });

        let _ = client.inner();
    }
}
