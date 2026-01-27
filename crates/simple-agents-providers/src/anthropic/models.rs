//! Anthropic API request and response types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Anthropic output format for structured outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicOutputFormat {
    /// JSON schema mode for structured outputs
    JsonSchema {
        /// The JSON schema definition
        json_schema: AnthropicJsonSchema,
    },
}

/// Anthropic JSON schema format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicJsonSchema {
    /// Name of the schema
    pub name: String,
    /// The JSON schema
    pub schema: Value,
    /// Whether to use strict mode (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Anthropic chat completion request
///
/// This struct borrows messages to avoid cloning during serialization.
/// System messages are extracted and sent separately.
#[derive(Debug, Serialize)]
pub struct AnthropicCompletionRequest<'a> {
    /// Model identifier (e.g., "claude-3-opus-20240229", "claude-3-sonnet-20240229")
    pub model: &'a str,

    /// List of messages in the conversation (borrowed to avoid cloning)
    /// System messages are excluded and sent via the `system` field
    pub messages: Vec<AnthropicMessage<'a>>,

    /// System prompt (extracted from system messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,

    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Temperature (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p sampling (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Whether to stream the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Stop sequences (borrowed when possible)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<&'a Vec<String>>,

    /// Output format for structured outputs (Claude Sonnet 4.5+, requires beta header)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<AnthropicOutputFormat>,
}

/// Anthropic message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage<'a> {
    /// Role (user or assistant)
    pub role: &'a str,

    /// Message content
    pub content: &'a str,
}

/// Anthropic chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicCompletionResponse {
    /// Unique identifier for the completion
    pub id: String,

    /// Object type (always "message")
    #[serde(rename = "type")]
    pub response_type: String,

    /// Role (always "assistant")
    pub role: String,

    /// Message content
    pub content: Vec<AnthropicContent>,

    /// Model used for completion
    pub model: String,

    /// Stop reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// Stop sequence that caused completion to stop
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,

    /// Token usage information
    pub usage: AnthropicUsage,
}

/// Anthropic content block
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    /// Number of tokens in the input
    pub input_tokens: u32,

    /// Number of tokens in the output
    pub output_tokens: u32,
}

/// Anthropic error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicErrorResponse {
    /// Error type
    #[serde(rename = "type")]
    pub error_type: String,

    /// Error details
    pub error: AnthropicErrorDetails,
}

/// Anthropic error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicErrorDetails {
    /// Error type
    #[serde(rename = "type")]
    pub error_type: String,

    /// Error message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_request() {
        let request = AnthropicCompletionRequest {
            model: "claude-3-opus-20240229",
            messages: vec![AnthropicMessage {
                role: "user",
                content: "Hello",
            }],
            system: Some("You are a helpful assistant".to_string()),
            max_tokens: 100,
            temperature: Some(0.7),
            top_p: None,
            stream: Some(false),
            stop_sequences: None,
            output_format: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("claude-3-opus"));
        assert!(json.contains("Hello"));
        assert!(json.contains("You are a helpful assistant"));
    }

    #[test]
    fn test_deserialize_response() {
        let json = r#"{
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Hello there!"
                }
            ],
            "model": "claude-3-opus-20240229",
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 20
            }
        }"#;

        let response: AnthropicCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "msg_123");
        assert_eq!(response.model, "claude-3-opus-20240229");
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            AnthropicContent::Text { text } => {
                assert_eq!(text, "Hello there!");
            }
        }
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 20);
    }

    #[test]
    fn test_deserialize_error_response() {
        let json = r#"{
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "Invalid API key"
            }
        }"#;

        let response: AnthropicErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.error_type, "error");
        assert_eq!(response.error.error_type, "invalid_request_error");
        assert_eq!(response.error.message, "Invalid API key");
    }
}
