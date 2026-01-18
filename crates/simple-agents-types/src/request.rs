//! Request types for LLM completions.
//!
//! Provides OpenAI-compatible request structures with validation.

use crate::error::{Result, ValidationError};
use crate::message::Message;
use serde::{Deserialize, Serialize};

/// A completion request to an LLM provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// List of messages in the conversation
    pub messages: Vec<Message>,
    /// Model identifier (e.g., "gpt-4", "claude-3-opus")
    pub model: String,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Sampling temperature (0.0-2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Nucleus sampling threshold (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Enable streaming responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Number of completions to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Presence penalty (-2.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Frequency penalty (-2.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// User identifier (for abuse detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

impl CompletionRequest {
    /// Create a new builder.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::request::CompletionRequest;
    /// use simple_agents_types::message::Message;
    ///
    /// let request = CompletionRequest::builder()
    ///     .model("gpt-4")
    ///     .message(Message::user("Hello!"))
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> CompletionRequestBuilder {
        CompletionRequestBuilder::default()
    }

    /// Validate the request.
    ///
    /// # Validation Rules
    /// - Messages: 1-1000 items, each < 1MB
    /// - Model: alphanumeric + `-_./` only
    /// - Temperature: 0.0-2.0
    /// - Top_p: 0.0-1.0
    /// - No null bytes (security)
    pub fn validate(&self) -> Result<()> {
        // Validate messages
        if self.messages.is_empty() {
            return Err(ValidationError::Empty {
                field: "messages".to_string(),
            }
            .into());
        }

        if self.messages.len() > 1000 {
            return Err(ValidationError::TooLong {
                field: "messages".to_string(),
                max: 1000,
            }
            .into());
        }

        // Validate each message content size (max 1MB)
        const MAX_MESSAGE_SIZE: usize = 1024 * 1024;
        for (i, msg) in self.messages.iter().enumerate() {
            if msg.content.len() > MAX_MESSAGE_SIZE {
                return Err(ValidationError::TooLong {
                    field: format!("messages[{}].content", i),
                    max: MAX_MESSAGE_SIZE,
                }
                .into());
            }

            // Security: no null bytes
            if msg.content.contains('\0') {
                return Err(ValidationError::InvalidFormat {
                    field: format!("messages[{}].content", i),
                    reason: "contains null bytes".to_string(),
                }
                .into());
            }
        }

        // Validate total request size (max 10MB)
        const MAX_TOTAL_REQUEST_SIZE: usize = 10 * 1024 * 1024;
        let total_size: usize = self.messages.iter().map(|m| m.content.len()).sum();
        if total_size > MAX_TOTAL_REQUEST_SIZE {
            return Err(ValidationError::TooLong {
                field: "total_request_size".to_string(),
                max: MAX_TOTAL_REQUEST_SIZE,
            }
            .into());
        }

        // Validate model
        if self.model.is_empty() {
            return Err(ValidationError::Empty {
                field: "model".to_string(),
            }
            .into());
        }

        // Model must be alphanumeric + `-_./`
        if !self
            .model
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
        {
            return Err(ValidationError::InvalidFormat {
                field: "model".to_string(),
                reason: "must be alphanumeric with -_./ only".to_string(),
            }
            .into());
        }

        // Validate temperature
        if let Some(temp) = self.temperature {
            if !(0.0..=2.0).contains(&temp) {
                return Err(ValidationError::OutOfRange {
                    field: "temperature".to_string(),
                    min: 0.0,
                    max: 2.0,
                }
                .into());
            }
        }

        // Validate top_p
        if let Some(top_p) = self.top_p {
            if !(0.0..=1.0).contains(&top_p) {
                return Err(ValidationError::OutOfRange {
                    field: "top_p".to_string(),
                    min: 0.0,
                    max: 1.0,
                }
                .into());
            }
        }

        // Validate presence_penalty
        if let Some(penalty) = self.presence_penalty {
            if !(-2.0..=2.0).contains(&penalty) {
                return Err(ValidationError::OutOfRange {
                    field: "presence_penalty".to_string(),
                    min: -2.0,
                    max: 2.0,
                }
                .into());
            }
        }

        // Validate frequency_penalty
        if let Some(penalty) = self.frequency_penalty {
            if !(-2.0..=2.0).contains(&penalty) {
                return Err(ValidationError::OutOfRange {
                    field: "frequency_penalty".to_string(),
                    min: -2.0,
                    max: 2.0,
                }
                .into());
            }
        }

        Ok(())
    }
}

/// Builder for CompletionRequest.
#[derive(Debug, Default, Clone)]
pub struct CompletionRequestBuilder {
    messages: Vec<Message>,
    model: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    stream: Option<bool>,
    n: Option<u32>,
    stop: Option<Vec<String>>,
    presence_penalty: Option<f32>,
    frequency_penalty: Option<f32>,
    user: Option<String>,
}

impl CompletionRequestBuilder {
    /// Set the model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Add a message.
    pub fn message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// Set all messages at once.
    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Set max_tokens.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature.
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set top_p.
    pub fn top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Enable streaming.
    pub fn stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }

    /// Set number of completions.
    pub fn n(mut self, n: u32) -> Self {
        self.n = Some(n);
        self
    }

    /// Set stop sequences.
    pub fn stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Set presence penalty.
    pub fn presence_penalty(mut self, penalty: f32) -> Self {
        self.presence_penalty = Some(penalty);
        self
    }

    /// Set frequency penalty.
    pub fn frequency_penalty(mut self, penalty: f32) -> Self {
        self.frequency_penalty = Some(penalty);
        self
    }

    /// Set user identifier.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Build and validate the request.
    pub fn build(self) -> Result<CompletionRequest> {
        let model = self.model.ok_or_else(|| ValidationError::Empty {
            field: "model".to_string(),
        })?;

        let request = CompletionRequest {
            messages: self.messages,
            model,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            top_p: self.top_p,
            stream: self.stream,
            n: self.n,
            stop: self.stop,
            presence_penalty: self.presence_penalty,
            frequency_penalty: self.frequency_penalty,
            user: self.user,
        };

        request.validate()?;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .build()
            .unwrap();

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].content, "Hello");
    }

    #[test]
    fn test_builder_all_fields() {
        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .max_tokens(100)
            .temperature(0.7)
            .top_p(0.9)
            .stream(true)
            .n(1)
            .stop(vec!["END".to_string()])
            .presence_penalty(0.5)
            .frequency_penalty(0.5)
            .user("test-user")
            .build()
            .unwrap();

        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.top_p, Some(0.9));
        assert_eq!(request.stream, Some(true));
        assert_eq!(request.n, Some(1));
        assert_eq!(request.stop, Some(vec!["END".to_string()]));
        assert_eq!(request.presence_penalty, Some(0.5));
        assert_eq!(request.frequency_penalty, Some(0.5));
        assert_eq!(request.user, Some("test-user".to_string()));
    }

    #[test]
    fn test_builder_missing_model() {
        let result = CompletionRequest::builder()
            .message(Message::user("Hello"))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_empty_messages() {
        let result = CompletionRequest::builder().model("gpt-4").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_invalid_temperature() {
        let result = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .temperature(3.0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_invalid_top_p() {
        let result = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .top_p(1.5)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_invalid_model_chars() {
        let result = CompletionRequest::builder()
            .model("gpt-4!")
            .message(Message::user("Hello"))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_serialization() {
        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .temperature(0.7)
            .build()
            .unwrap();

        let json = serde_json::to_string(&request).unwrap();
        let parsed: CompletionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, parsed);
    }

    #[test]
    fn test_optional_fields_not_serialized() {
        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .build()
            .unwrap();

        let json = serde_json::to_value(&request).unwrap();
        assert!(!json.get("max_tokens").is_some());
        assert!(!json.get("temperature").is_some());
    }

    #[test]
    fn test_validation_total_request_size_limit() {
        // Create a request that exceeds 10MB total
        let large_content = "x".repeat(2 * 1024 * 1024); // 2MB per message
        let result = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user(large_content.clone()))
            .message(Message::user(large_content.clone()))
            .message(Message::user(large_content.clone()))
            .message(Message::user(large_content.clone()))
            .message(Message::user(large_content.clone()))
            .message(Message::user(large_content.clone())) // 6 * 2MB = 12MB > 10MB
            .build();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::SimpleAgentsError::Validation(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn test_validation_total_request_size_within_limit() {
        // Create a request that's under 10MB total
        let content = "x".repeat(1024 * 1024); // 1MB per message
        let result = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user(content.clone()))
            .message(Message::user(content.clone()))
            .message(Message::user(content.clone())) // 3MB < 10MB
            .build();

        assert!(result.is_ok());
    }
}
