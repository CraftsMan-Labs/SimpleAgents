//! Provider error types and error handling.
//!
//! This module defines the error hierarchy for provider interactions,
//! following OpenAI-compatible error codes and patterns.

use std::time::Duration;
use thiserror::Error;

/// Transport-level errors for HTTP interactions with LLM APIs.
///
/// Distinct from `simple_agent_type::ProviderError` which is the canonical,
/// public-facing provider error type. `TransportError` captures lower-level
/// HTTP transport concerns (connection, serialization, timeouts).
///
/// # Examples
///
/// ```
/// use simple_agents_providers::common::{TransportError, RetryableError};
/// use std::time::Duration;
///
/// let err = TransportError::RateLimit {
///     retry_after: Some(Duration::from_secs(5)),
/// };
///
/// assert!(err.is_retryable());
/// assert_eq!(err.retry_after(), Some(Duration::from_secs(5)));
/// ```
#[derive(Error, Debug, Clone)]
pub enum TransportError {
    /// Invalid API key (401)
    #[error("Invalid API key")]
    InvalidApiKey,

    /// Rate limit exceeded (429)
    ///
    /// Contains optional retry-after duration from provider headers.
    #[error("Rate limit exceeded (retry after {retry_after:?})")]
    RateLimit {
        /// Duration to wait before retrying (from retry-after header)
        retry_after: Option<Duration>,
    },

    /// Model not found (404)
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Server error (500-599)
    #[error("Server error: {0}")]
    ServerError(String),

    /// Request timeout
    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    /// Network error (connection failed, DNS, etc.)
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Invalid request (400)
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Trait for determining if an error is retryable.
///
/// Implements retry detection logic per CODING_GUIDELINES.md:296-318
pub trait RetryableError {
    /// Returns true if this error should trigger a retry.
    fn is_retryable(&self) -> bool;

    /// Returns the duration to wait before retrying (if specified by provider).
    fn retry_after(&self) -> Option<Duration>;
}

impl RetryableError for TransportError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            TransportError::Timeout(_)
                | TransportError::RateLimit { .. }
                | TransportError::ServerError(_)
                | TransportError::NetworkError(_)
        )
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            TransportError::RateLimit { retry_after } => *retry_after,
            _ => None,
        }
    }
}

/// Convert reqwest errors to TransportError.
impl From<reqwest::Error> for TransportError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            TransportError::Timeout(Duration::from_secs(30))
        } else if err.is_connect() {
            TransportError::NetworkError(format!("Connection failed: {}", err))
        } else {
            TransportError::NetworkError(err.to_string())
        }
    }
}

/// Convert serde_json errors to TransportError.
impl From<serde_json::Error> for TransportError {
    fn from(err: serde_json::Error) -> Self {
        TransportError::SerializationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        let timeout = TransportError::Timeout(Duration::from_secs(30));
        assert!(timeout.is_retryable());
        assert_eq!(timeout.retry_after(), None);

        let rate_limit = TransportError::RateLimit {
            retry_after: Some(Duration::from_secs(5)),
        };
        assert!(rate_limit.is_retryable());
        assert_eq!(rate_limit.retry_after(), Some(Duration::from_secs(5)));

        let server_error = TransportError::ServerError("Internal error".to_string());
        assert!(server_error.is_retryable());

        let network_error = TransportError::NetworkError("Connection reset".to_string());
        assert!(network_error.is_retryable());
    }

    #[test]
    fn test_non_retryable_errors() {
        let invalid_key = TransportError::InvalidApiKey;
        assert!(!invalid_key.is_retryable());

        let model_not_found = TransportError::ModelNotFound("gpt-5".to_string());
        assert!(!model_not_found.is_retryable());

        let invalid_request = TransportError::InvalidRequest("Missing model".to_string());
        assert!(!invalid_request.is_retryable());
    }

    #[test]
    fn test_error_display() {
        let err = TransportError::RateLimit {
            retry_after: Some(Duration::from_secs(10)),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Rate limit exceeded"));
        assert!(msg.contains("10s"));
    }

    #[test]
    fn test_retry_after_extraction() {
        let err = TransportError::RateLimit {
            retry_after: Some(Duration::from_secs(15)),
        };
        assert_eq!(err.retry_after(), Some(Duration::from_secs(15)));

        let err2 = TransportError::Timeout(Duration::from_secs(30));
        assert_eq!(err2.retry_after(), None);
    }
}
