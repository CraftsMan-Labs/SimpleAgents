//! HTTP client wrapper with connection pooling and HTTP/2 support.
//!
//! This module provides a configured HTTP client optimized for LLM API calls:
//! - Connection pooling (max 10 idle connections per host)
//! - HTTP/2 multiplexing enabled
//! - Configurable timeouts
//! - Idle connection timeout (90 seconds)

use reqwest::Client;
use std::time::Duration;

/// HTTP client wrapper with optimized configuration.
///
/// # Configuration
///
/// - **Timeout**: 30 seconds default (configurable)
/// - **Connection Pooling**: Max 10 idle connections per host
/// - **Idle Timeout**: 90 seconds before connection cleanup
/// - **HTTP/2**: Enabled via prior knowledge
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
    /// - HTTP/2 enabled
    /// - Connection pooling (10 idle per host, 90s timeout)
    ///
    /// # Errors
    ///
    /// Returns error if the client fails to build (rare, usually system-level issues).
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_timeout(Duration::from_secs(30))
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
        let inner = Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .http2_prior_knowledge()
            .build()?;

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
        Self::new().expect("Failed to create default HTTP client")
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
}
