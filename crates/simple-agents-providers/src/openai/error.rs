//! OpenAI-specific error handling.

use simple_agents_types::ProviderError;
use std::time::Duration;
use thiserror::Error;

/// OpenAI-specific errors
#[derive(Error, Debug)]
pub enum OpenAIError {
    /// Invalid API key
    #[error("Invalid API key")]
    InvalidApiKey,

    /// Model not found or not accessible
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimit {
        /// Time to wait before retrying
        retry_after: Option<Duration>,
    },

    /// Context length exceeded
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),

    /// Server error (5xx)
    #[error("Server error: {0}")]
    ServerError(String),

    /// Bad request (4xx)
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl OpenAIError {
    /// Parse OpenAI error from HTTP response
    ///
    /// # Arguments
    ///
    /// * `status` - HTTP status code
    /// * `body` - Response body text
    pub fn from_response(status: u16, body: &str) -> Self {
        // Try to parse as OpenAI error response
        if let Ok(error_response) = serde_json::from_str::<super::OpenAIErrorResponse>(body) {
            return Self::from_error_details(status, &error_response.error.message);
        }

        // If not JSON, still check for specific patterns in plain text
        Self::from_error_details(status, body)
    }

    /// Parse error from error details
    fn from_error_details(status: u16, message: &str) -> Self {
        let message_lower = message.to_lowercase();

        // Check for specific error patterns
        if message_lower.contains("invalid") && message_lower.contains("api key") {
            return Self::InvalidApiKey;
        }

        if message_lower.contains("model") && message_lower.contains("not found") {
            return Self::ModelNotFound(message.to_string());
        }

        if message_lower.contains("rate limit") {
            return Self::RateLimit { retry_after: None };
        }

        if message_lower.contains("context length") {
            return Self::ContextLengthExceeded(message.to_string());
        }

        // Fall back to status-based error
        match status {
            401 => Self::InvalidApiKey,
            404 => Self::ModelNotFound(message.to_string()),
            429 => Self::RateLimit { retry_after: None },
            400..=499 => Self::BadRequest(message.to_string()),
            500..=599 => Self::ServerError(message.to_string()),
            _ => Self::Unknown(message.to_string()),
        }
    }
}

/// Convert OpenAIError to ProviderError
impl From<OpenAIError> for ProviderError {
    fn from(error: OpenAIError) -> Self {
        match error {
            OpenAIError::InvalidApiKey => ProviderError::InvalidApiKey,
            OpenAIError::ModelNotFound(msg) => ProviderError::ModelNotFound(msg),
            OpenAIError::RateLimit { retry_after } => ProviderError::RateLimit { retry_after },
            OpenAIError::ContextLengthExceeded(msg) => ProviderError::BadRequest(msg),
            OpenAIError::ServerError(msg) => ProviderError::ServerError(msg),
            OpenAIError::BadRequest(msg) => ProviderError::BadRequest(msg),
            OpenAIError::Unknown(msg) => ProviderError::InvalidResponse(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_api_key_error() {
        let error = OpenAIError::from_response(401, "Incorrect API key provided");
        assert!(matches!(error, OpenAIError::InvalidApiKey));
    }

    #[test]
    fn test_model_not_found_error() {
        let error = OpenAIError::from_response(404, "Model gpt-5 not found");
        assert!(matches!(error, OpenAIError::ModelNotFound(_)));
    }

    #[test]
    fn test_rate_limit_error() {
        let error = OpenAIError::from_response(429, "Rate limit exceeded");
        assert!(matches!(error, OpenAIError::RateLimit { .. }));
    }

    #[test]
    fn test_server_error() {
        let error = OpenAIError::from_response(500, "Internal server error");
        assert!(matches!(error, OpenAIError::ServerError(_)));
    }

    #[test]
    fn test_parse_json_error() {
        let json = r#"{
            "error": {
                "message": "Invalid API key provided",
                "type": "invalid_request_error",
                "code": "invalid_api_key"
            }
        }"#;

        let error = OpenAIError::from_response(401, json);
        assert!(matches!(error, OpenAIError::InvalidApiKey));
    }

    #[test]
    fn test_context_length_exceeded() {
        let error = OpenAIError::from_response(
            400,
            "This model's maximum context length is 4096 tokens"
        );
        assert!(matches!(error, OpenAIError::ContextLengthExceeded(_)));
    }
}
