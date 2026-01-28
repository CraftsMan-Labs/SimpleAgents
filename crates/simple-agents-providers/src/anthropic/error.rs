//! Anthropic-specific error handling.

use simple_agent_type::error::ProviderError;
use thiserror::Error;

use super::models::AnthropicErrorResponse;

/// Anthropic-specific errors
#[derive(Error, Debug, Clone)]
pub enum AnthropicError {
    /// Invalid API key
    #[error("Invalid API key")]
    InvalidApiKey,

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Overloaded (503)
    #[error("Service overloaded, please try again later")]
    Overloaded,

    /// Unknown error
    #[error("Anthropic API error: {0}")]
    Unknown(String),
}

impl AnthropicError {
    /// Parse an Anthropic error from HTTP response.
    ///
    /// # Arguments
    /// - `status`: HTTP status code
    /// - `body`: Response body text
    ///
    /// # Returns
    /// Appropriate `AnthropicError` variant based on the error details
    pub fn from_response(status: u16, body: &str) -> Self {
        // Try to parse as structured error response
        if let Ok(error_response) = serde_json::from_str::<AnthropicErrorResponse>(body) {
            return Self::from_error_type(
                &error_response.error.error_type,
                &error_response.error.message,
            );
        }

        // Fall back to status code mapping
        match status {
            401 => Self::InvalidApiKey,
            429 => Self::RateLimitExceeded,
            400 => Self::InvalidRequest(body.to_string()),
            503 => Self::Overloaded,
            _ => Self::Unknown(format!("HTTP {} - {}", status, body)),
        }
    }

    /// Create error from Anthropic error type.
    fn from_error_type(error_type: &str, message: &str) -> Self {
        match error_type {
            "authentication_error" | "invalid_request_error" if message.contains("api_key") => {
                Self::InvalidApiKey
            }
            "rate_limit_error" => Self::RateLimitExceeded,
            "invalid_request_error" => Self::InvalidRequest(message.to_string()),
            "overloaded_error" => Self::Overloaded,
            _ => Self::Unknown(format!("{}: {}", error_type, message)),
        }
    }
}

impl From<AnthropicError> for ProviderError {
    fn from(error: AnthropicError) -> Self {
        match error {
            AnthropicError::InvalidApiKey => ProviderError::InvalidApiKey,
            AnthropicError::RateLimitExceeded => ProviderError::RateLimit { retry_after: None },
            AnthropicError::InvalidRequest(msg) => ProviderError::BadRequest(msg),
            AnthropicError::Overloaded => ProviderError::ServerError(error.to_string()),
            AnthropicError::Unknown(msg) => ProviderError::BadRequest(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_authentication_error() {
        let body = r#"{
            "type": "error",
            "error": {
                "type": "authentication_error",
                "message": "Invalid api_key"
            }
        }"#;

        let error = AnthropicError::from_response(401, body);
        assert!(matches!(error, AnthropicError::InvalidApiKey));
    }

    #[test]
    fn test_parse_rate_limit_error() {
        let body = r#"{
            "type": "error",
            "error": {
                "type": "rate_limit_error",
                "message": "Rate limit exceeded"
            }
        }"#;

        let error = AnthropicError::from_response(429, body);
        assert!(matches!(error, AnthropicError::RateLimitExceeded));
    }

    #[test]
    fn test_parse_invalid_request_error() {
        let body = r#"{
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "Missing required field"
            }
        }"#;

        let error = AnthropicError::from_response(400, body);
        assert!(matches!(error, AnthropicError::InvalidRequest(_)));
    }

    #[test]
    fn test_fallback_status_mapping() {
        let error = AnthropicError::from_response(503, "Service unavailable");
        assert!(matches!(error, AnthropicError::Overloaded));
    }

    #[test]
    fn test_convert_to_provider_error() {
        let anthropic_error = AnthropicError::InvalidApiKey;
        let provider_error: ProviderError = anthropic_error.into();
        assert!(matches!(provider_error, ProviderError::InvalidApiKey));
    }
}
