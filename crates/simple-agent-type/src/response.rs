//! Response types for LLM completions.
//!
//! Provides OpenAI-compatible response structures.

use crate::coercion::CoercionFlag;
use crate::message::Message;
use serde::{Deserialize, Serialize};

/// Metadata about healing/coercion applied to a response.
///
/// When native structured output parsing fails and healing is enabled,
/// this metadata tracks all transformations applied to recover the response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealingMetadata {
    /// All coercion flags applied during healing
    pub flags: Vec<CoercionFlag>,
    /// Confidence score (0.0-1.0) of the healed response
    pub confidence: f32,
    /// The original parsing error that triggered healing
    pub original_error: String,
}

impl HealingMetadata {
    /// Create new healing metadata.
    pub fn new(flags: Vec<CoercionFlag>, confidence: f32, original_error: String) -> Self {
        Self {
            flags,
            confidence: confidence.clamp(0.0, 1.0),
            original_error,
        }
    }

    /// Check if any major coercions were applied.
    pub fn has_major_coercions(&self) -> bool {
        self.flags.iter().any(|f| f.is_major())
    }

    /// Check if confidence meets a threshold.
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }
}

/// A completion response from an LLM provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Unique response identifier
    pub id: String,
    /// Model used for completion
    pub model: String,
    /// List of completion choices
    pub choices: Vec<CompletionChoice>,
    /// Token usage statistics
    pub usage: Usage,
    /// Unix timestamp of creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    /// Provider that generated this response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Healing metadata (present if response was healed after parse failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healing_metadata: Option<HealingMetadata>,
}

impl CompletionResponse {
    /// Get the content of the first choice (convenience method).
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::response::{CompletionResponse, CompletionChoice, Usage, FinishReason};
    /// use simple_agent_type::message::Message;
    ///
    /// let response = CompletionResponse {
    ///     id: "resp_123".to_string(),
    ///     model: "gpt-4".to_string(),
    ///     choices: vec![CompletionChoice {
    ///         index: 0,
    ///         message: Message::assistant("Hello!"),
    ///         finish_reason: FinishReason::Stop,
    ///         logprobs: None,
    ///     }],
    ///     usage: Usage {
    ///         prompt_tokens: 10,
    ///         completion_tokens: 5,
    ///         total_tokens: 15,
    ///     },
    ///     created: None,
    ///     provider: None,
    ///     healing_metadata: None,
    /// };
    ///
    /// assert_eq!(response.content(), Some("Hello!"));
    /// ```
    pub fn content(&self) -> Option<&str> {
        self.choices
            .first()
            .map(|choice| choice.message.content.as_str())
    }

    /// Get the first choice.
    pub fn first_choice(&self) -> Option<&CompletionChoice> {
        self.choices.first()
    }

    /// Check if this response was healed after a parsing failure.
    ///
    /// Returns `true` if healing metadata is present, indicating the response
    /// required transformation to be parseable.
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::response::{CompletionResponse, CompletionChoice, Usage, FinishReason, HealingMetadata};
    /// use simple_agent_type::message::Message;
    /// use simple_agent_type::coercion::CoercionFlag;
    ///
    /// let mut response = CompletionResponse {
    ///     id: "resp_123".to_string(),
    ///     model: "gpt-4".to_string(),
    ///     choices: vec![],
    ///     usage: Usage::new(10, 5),
    ///     created: None,
    ///     provider: None,
    ///     healing_metadata: None,
    /// };
    ///
    /// assert!(!response.was_healed());
    ///
    /// response.healing_metadata = Some(HealingMetadata::new(
    ///     vec![CoercionFlag::StrippedMarkdown],
    ///     0.9,
    ///     "Parse error".to_string(),
    /// ));
    ///
    /// assert!(response.was_healed());
    /// ```
    pub fn was_healed(&self) -> bool {
        self.healing_metadata.is_some()
    }

    /// Get the confidence score of the response.
    ///
    /// Returns 1.0 if the response was not healed (perfect confidence),
    /// otherwise returns the confidence score from healing metadata.
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::response::{CompletionResponse, CompletionChoice, Usage, FinishReason, HealingMetadata};
    /// use simple_agent_type::message::Message;
    /// use simple_agent_type::coercion::CoercionFlag;
    ///
    /// let mut response = CompletionResponse {
    ///     id: "resp_123".to_string(),
    ///     model: "gpt-4".to_string(),
    ///     choices: vec![],
    ///     usage: Usage::new(10, 5),
    ///     created: None,
    ///     provider: None,
    ///     healing_metadata: None,
    /// };
    ///
    /// assert_eq!(response.confidence(), 1.0);
    ///
    /// response.healing_metadata = Some(HealingMetadata::new(
    ///     vec![CoercionFlag::StrippedMarkdown],
    ///     0.8,
    ///     "Parse error".to_string(),
    /// ));
    ///
    /// assert_eq!(response.confidence(), 0.8);
    /// ```
    pub fn confidence(&self) -> f32 {
        self.healing_metadata
            .as_ref()
            .map(|m| m.confidence)
            .unwrap_or(1.0)
    }
}

/// A single completion choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionChoice {
    /// Index of this choice
    pub index: u32,
    /// The message content
    pub message: Message,
    /// Why the completion finished
    pub finish_reason: FinishReason,
    /// Log probabilities (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// Reason why a completion finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural stop point reached
    Stop,
    /// Maximum token length reached
    Length,
    /// Content filtered by provider
    ContentFilter,
    /// Tool/function calls generated
    ToolCalls,
}

/// Token usage statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens in the prompt
    pub prompt_tokens: u32,
    /// Tokens in the completion
    pub completion_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
}

impl Usage {
    /// Create a new Usage with calculated total.
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// A chunk of a streaming completion response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionChunk {
    /// Unique response identifier
    pub id: String,
    /// Model used for completion
    pub model: String,
    /// List of choice deltas
    pub choices: Vec<ChoiceDelta>,
    /// Unix timestamp of creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
}

/// A delta in a streaming choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceDelta {
    /// Index of this choice
    pub index: u32,
    /// The message delta
    pub delta: MessageDelta,
    /// Why the completion finished (only in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

/// Incremental message content in a stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDelta {
    /// Role (only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<crate::message::Role>,
    /// Incremental content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_response_content() {
        let response = CompletionResponse {
            id: "resp_123".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("Hello!"),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage::new(10, 5),
            created: Some(1234567890),
            provider: Some("openai".to_string()),
            healing_metadata: None,
        };

        assert_eq!(response.content(), Some("Hello!"));
        assert_eq!(response.first_choice().unwrap().index, 0);
        assert!(!response.was_healed());
        assert_eq!(response.confidence(), 1.0);
    }

    #[test]
    fn test_completion_response_empty_choices() {
        let response = CompletionResponse {
            id: "resp_123".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: Usage::new(10, 0),
            created: None,
            provider: None,
            healing_metadata: None,
        };

        assert_eq!(response.content(), None);
        assert_eq!(response.first_choice(), None);
    }

    #[test]
    fn test_usage_calculation() {
        let usage = Usage::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_finish_reason_serialization() {
        let json = serde_json::to_string(&FinishReason::Stop).unwrap();
        assert_eq!(json, "\"stop\"");

        let json = serde_json::to_string(&FinishReason::Length).unwrap();
        assert_eq!(json, "\"length\"");

        let json = serde_json::to_string(&FinishReason::ContentFilter).unwrap();
        assert_eq!(json, "\"content_filter\"");

        let json = serde_json::to_string(&FinishReason::ToolCalls).unwrap();
        assert_eq!(json, "\"tool_calls\"");
    }

    #[test]
    fn test_response_serialization() {
        let response = CompletionResponse {
            id: "resp_123".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("Hello!"),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage::new(10, 5),
            created: None,
            provider: None,
            healing_metadata: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: CompletionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, parsed);
    }

    #[test]
    fn test_streaming_chunk() {
        let chunk = CompletionChunk {
            id: "resp_123".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: Some(crate::message::Role::Assistant),
                    content: Some("Hello".to_string()),
                },
                finish_reason: None,
            }],
            created: Some(1234567890),
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let parsed: CompletionChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(chunk, parsed);
    }

    #[test]
    fn test_message_delta() {
        let delta = MessageDelta {
            role: Some(crate::message::Role::Assistant),
            content: Some("Hi".to_string()),
        };

        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json.get("role").and_then(|v| v.as_str()), Some("assistant"));
        assert_eq!(json.get("content").and_then(|v| v.as_str()), Some("Hi"));
    }

    #[test]
    fn test_optional_fields_not_serialized() {
        let response = CompletionResponse {
            id: "resp_123".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: Usage::new(10, 5),
            created: None,
            provider: None,
            healing_metadata: None,
        };

        let json = serde_json::to_value(&response).unwrap();
        assert!(json.get("created").is_none());
        assert!(json.get("provider").is_none());
        assert!(json.get("healing_metadata").is_none());
    }

    #[test]
    fn test_healing_metadata() {
        use crate::coercion::CoercionFlag;

        let metadata = HealingMetadata::new(
            vec![CoercionFlag::StrippedMarkdown],
            0.9,
            "Parse error".to_string(),
        );

        assert_eq!(metadata.confidence, 0.9);
        assert!(!metadata.has_major_coercions());
        assert!(metadata.is_confident(0.8));
        assert!(!metadata.is_confident(0.95));

        let major_metadata = HealingMetadata::new(
            vec![CoercionFlag::TruncatedJson],
            0.7,
            "Parse error".to_string(),
        );

        assert!(major_metadata.has_major_coercions());
    }

    #[test]
    fn test_healing_metadata_confidence_clamped() {
        let metadata = HealingMetadata::new(vec![], 1.5, "error".to_string());
        assert_eq!(metadata.confidence, 1.0);

        let metadata = HealingMetadata::new(vec![], -0.5, "error".to_string());
        assert_eq!(metadata.confidence, 0.0);
    }

    #[test]
    fn test_response_with_healing_metadata() {
        use crate::coercion::CoercionFlag;

        let metadata = HealingMetadata::new(
            vec![
                CoercionFlag::StrippedMarkdown,
                CoercionFlag::FixedTrailingComma,
            ],
            0.85,
            "JSON parse error".to_string(),
        );

        let response = CompletionResponse {
            id: "resp_123".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: Usage::new(10, 5),
            created: None,
            provider: None,
            healing_metadata: Some(metadata),
        };

        assert!(response.was_healed());
        assert_eq!(response.confidence(), 0.85);

        let json = serde_json::to_string(&response).unwrap();
        let parsed: CompletionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, parsed);
        assert!(parsed.was_healed());
        assert_eq!(parsed.confidence(), 0.85);
    }
}
