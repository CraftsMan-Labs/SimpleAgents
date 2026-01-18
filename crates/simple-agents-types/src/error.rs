//! Error types for SimpleAgents.
//!
//! Comprehensive error hierarchy for all failure modes.

use std::time::Duration;
use thiserror::Error;

/// Main error type for SimpleAgents operations.
#[derive(Error, Debug)]
pub enum SimpleAgentsError {
    /// Provider-specific error
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    /// Healing/coercion error
    #[error("Healing error: {0}")]
    Healing(#[from] HealingError),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Cache error
    #[error("Cache error: {0}")]
    Cache(String),

    /// Routing error
    #[error("Routing error: {0}")]
    Routing(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type alias using SimpleAgentsError.
pub type Result<T> = std::result::Result<T, SimpleAgentsError>;

/// Provider-specific errors.
#[derive(Error, Debug, Clone)]
pub enum ProviderError {
    /// Rate limit exceeded
    #[error("Rate limit exceeded (retry after {retry_after:?})")]
    RateLimit {
        /// Optional duration to wait before retrying
        retry_after: Option<Duration>,
    },

    /// Invalid API key
    #[error("Invalid API key")]
    InvalidApiKey,

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Request timeout
    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    /// Server error (5xx)
    #[error("Server error: {0}")]
    ServerError(String),

    /// Bad request (4xx)
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Unsupported feature
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    /// Invalid response format
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

impl ProviderError {
    /// Check if this error is retryable.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::error::ProviderError;
    /// use std::time::Duration;
    ///
    /// let err = ProviderError::RateLimit { retry_after: None };
    /// assert!(err.is_retryable());
    ///
    /// let err = ProviderError::InvalidApiKey;
    /// assert!(!err.is_retryable());
    /// ```
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. } | Self::Timeout(_) | Self::ServerError(_)
        )
    }
}

/// Healing and coercion errors.
#[derive(Error, Debug, Clone)]
pub enum HealingError {
    /// JSON parsing failed
    #[error("Failed to parse JSON: {error_message}")]
    ParseFailed {
        /// Error message
        error_message: String,
        /// Input that failed to parse
        input: String,
    },

    /// Type coercion failed
    #[error("Type coercion failed: cannot convert {from} to {to}")]
    CoercionFailed {
        /// Source type
        from: String,
        /// Target type
        to: String,
    },

    /// Missing required field
    #[error("Missing required field: {field}")]
    MissingField {
        /// Field name
        field: String,
    },

    /// Confidence below threshold
    #[error("Confidence {confidence} below threshold {threshold}")]
    LowConfidence {
        /// Actual confidence score
        confidence: f32,
        /// Required threshold
        threshold: f32,
    },

    /// Invalid JSON structure
    #[error("Invalid JSON structure: {0}")]
    InvalidStructure(String),

    /// Exceeded maximum healing attempts
    #[error("Exceeded maximum healing attempts ({0})")]
    MaxAttemptsExceeded(u32),
}

/// Validation errors.
#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    /// Empty field
    #[error("Field cannot be empty: {field}")]
    Empty {
        /// Field name
        field: String,
    },

    /// Value too short
    #[error("Field too short: {field} (minimum: {min})")]
    TooShort {
        /// Field name
        field: String,
        /// Minimum length
        min: usize,
    },

    /// Value too long
    #[error("Field too long: {field} (maximum: {max})")]
    TooLong {
        /// Field name
        field: String,
        /// Maximum length
        max: usize,
    },

    /// Value out of range
    #[error("Value out of range: {field} (must be {min}-{max})")]
    OutOfRange {
        /// Field name
        field: String,
        /// Minimum value
        min: f32,
        /// Maximum value
        max: f32,
    },

    /// Invalid format
    #[error("Invalid format: {field} ({reason})")]
    InvalidFormat {
        /// Field name
        field: String,
        /// Reason for rejection
        reason: String,
    },

    /// Generic validation error
    #[error("{0}")]
    Custom(String),
}

impl ValidationError {
    /// Create a custom validation error.
    pub fn new(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_error_retryable() {
        assert!(ProviderError::RateLimit { retry_after: None }.is_retryable());
        assert!(ProviderError::Timeout(Duration::from_secs(30)).is_retryable());
        assert!(ProviderError::ServerError("500".to_string()).is_retryable());

        assert!(!ProviderError::InvalidApiKey.is_retryable());
        assert!(!ProviderError::ModelNotFound("gpt-5".to_string()).is_retryable());
        assert!(!ProviderError::BadRequest("invalid".to_string()).is_retryable());
    }

    #[test]
    fn test_error_conversion() {
        let validation_err = ValidationError::new("test");
        let agents_err: SimpleAgentsError = validation_err.into();
        assert!(matches!(agents_err, SimpleAgentsError::Validation(_)));

        let provider_err = ProviderError::InvalidApiKey;
        let agents_err: SimpleAgentsError = provider_err.into();
        assert!(matches!(agents_err, SimpleAgentsError::Provider(_)));
    }

    #[test]
    fn test_error_display() {
        let err = ProviderError::RateLimit {
            retry_after: Some(Duration::from_secs(60)),
        };
        let display = format!("{}", err);
        assert!(display.contains("Rate limit"));
        assert!(display.contains("60s"));

        let err = ValidationError::Empty {
            field: "model".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("model"));
        assert!(display.contains("empty"));
    }

    #[test]
    fn test_healing_error_types() {
        let err = HealingError::ParseFailed {
            error_message: "unexpected token".to_string(),
            input: "{invalid}".to_string(),
        };
        assert!(format!("{}", err).contains("parse"));

        let err = HealingError::CoercionFailed {
            from: "string".to_string(),
            to: "number".to_string(),
        };
        assert!(format!("{}", err).contains("coercion"));
    }
}
