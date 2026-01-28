//! Streaming support for Anthropic API via Server-Sent Events (SSE).
//!
//! Anthropic uses SSE format with named event types:
//! - `message_start`: Initial message metadata
//! - `content_block_start`: Start of a content block
//! - `content_block_delta`: Incremental content update
//! - `content_block_stop`: End of content block
//! - `message_delta`: Top-level message changes
//! - `message_stop`: End of message
//! - `ping`: Keepalive event (ignored)
//!
//! This module handles parsing SSE events and transforming them to unified format.

use bytes::Bytes;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use simple_agent_type::prelude::*;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::models::{AnthropicContent, AnthropicUsage};

/// Anthropic streaming event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicStreamEvent {
    /// Message start event - contains initial message metadata
    MessageStart { message: StreamMessage },
    /// Content block start event
    ContentBlockStart {
        index: u32,
        content_block: StreamContentBlock,
    },
    /// Content block delta event - incremental update
    ContentBlockDelta { index: u32, delta: StreamDelta },
    /// Content block stop event
    ContentBlockStop { index: u32 },
    /// Message delta event - top-level changes
    MessageDelta {
        delta: MessageDeltaData,
        usage: Option<AnthropicUsage>,
    },
    /// Message stop event - end of stream
    MessageStop,
    /// Ping event - keepalive (ignored)
    Ping,
    /// Error event
    Error { error: AnthropicStreamError },
}

/// Message metadata from message_start event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: String,
    pub content: Vec<AnthropicContent>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// Content block type from content_block_start
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Delta types for content_block_delta
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

/// Message delta data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// Stream error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicStreamError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// State for tracking streaming response (internal)
#[derive(Default)]
struct StreamState {
    id: Option<String>,
    model: Option<String>,
    content_buffer: String,
    finish_reason: Option<FinishReason>,
}

/// Parse SSE line to extract Anthropic stream event
///
/// Format:
/// ```text
/// event: message_start
/// data: {"type":"message_start",...}
/// ```
pub fn parse_sse_line(event_type: &str, data: &str) -> Option<Result<AnthropicStreamEvent>> {
    let event_type = event_type.trim();
    let data = data.trim();

    // Skip empty data
    if data.is_empty() {
        return None;
    }

    // Parse based on event type
    match event_type {
        "message_start" => match serde_json::from_str::<serde_json::Value>(data) {
            Ok(v) => match serde_json::from_value(v["message"].clone()) {
                Ok(msg) => Some(Ok(AnthropicStreamEvent::MessageStart { message: msg })),
                Err(_) => None,
            },
            Err(_) => None,
        },
        "content_block_start" => serde_json::from_str(data).map(Ok).ok(),
        "content_block_delta" => serde_json::from_str(data).map(Ok).ok(),
        "content_block_stop" => serde_json::from_str(data).map(Ok).ok(),
        "message_delta" => serde_json::from_str(data).map(Ok).ok(),
        "message_stop" => Some(Ok(AnthropicStreamEvent::MessageStop)),
        "ping" => None,
        "error" => serde_json::from_str(data).map(Ok).ok(),
        _ => None,
    }
}

/// Transform Anthropic stream event to unified CompletionChunk (internal)
fn transform_stream_event(
    event: AnthropicStreamEvent,
    state: &mut StreamState,
) -> Option<Result<CompletionChunk>> {
    match event {
        AnthropicStreamEvent::MessageStart { message } => {
            state.id = Some(message.id);
            state.model = Some(message.model);
            None
        }
        AnthropicStreamEvent::ContentBlockDelta { delta, .. } => {
            match delta {
                StreamDelta::TextDelta { text } => {
                    state.content_buffer.push_str(&text);
                    Some(Ok(CompletionChunk {
                        id: state.id.clone().unwrap_or_default(),
                        model: state.model.clone().unwrap_or_default(),
                        choices: vec![ChoiceDelta {
                            index: 0,
                            delta: MessageDelta {
                                role: Some(Role::Assistant),
                                content: Some(text),
                            },
                            finish_reason: None,
                        }],
                        created: None,
                    }))
                }
                StreamDelta::InputJsonDelta { .. } => {
                    // Tool use deltas not yet supported
                    None
                }
            }
        }
        AnthropicStreamEvent::MessageDelta { delta, .. } => {
            if let Some(stop_reason) = delta.stop_reason {
                state.finish_reason = Some(match stop_reason.as_str() {
                    "end_turn" => FinishReason::Stop,
                    "max_tokens" => FinishReason::Length,
                    "stop_sequence" => FinishReason::Stop,
                    "tool_use" => FinishReason::ToolCalls,
                    _ => FinishReason::Stop,
                });
            }
            None
        }
        AnthropicStreamEvent::MessageStop => {
            if state.finish_reason.is_some() {
                Some(Ok(CompletionChunk {
                    id: state.id.clone().unwrap_or_default(),
                    model: state.model.clone().unwrap_or_default(),
                    choices: vec![ChoiceDelta {
                        index: 0,
                        delta: MessageDelta {
                            role: None,
                            content: None,
                        },
                        finish_reason: state.finish_reason,
                    }],
                    created: None,
                }))
            } else {
                None
            }
        }
        AnthropicStreamEvent::Error { error } => Some(Err(SimpleAgentsError::Provider(
            ProviderError::InvalidResponse(error.message),
        ))),
        _ => None,
    }
}

/// Stream wrapper for processing Anthropic SSE events
///
/// Converts raw bytes into parsed `CompletionChunk` objects.
pub struct AnthropicSseStream {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    buffer: String,
    state: StreamState,
    done: bool,
}

impl AnthropicSseStream {
    pub fn new(stream: impl Stream<Item = reqwest::Result<Bytes>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(stream),
            buffer: String::new(),
            state: StreamState::default(),
            done: false,
        }
    }
}

impl Stream for AnthropicSseStream {
    type Item = Result<CompletionChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }

        loop {
            // Try to extract a complete event from buffer
            if let Some((event_type, data)) = self.extract_sse_event() {
                if let Some(result) = parse_sse_line(&event_type, &data) {
                    match result {
                        Ok(event) => {
                            if let Some(chunk) = transform_stream_event(event, &mut self.state) {
                                return Poll::Ready(Some(chunk));
                            }
                            // Continue to next event
                        }
                        Err(e) => {
                            self.done = true;
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                }
                // Continue to next event
            }

            // Need more data - poll inner stream
            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => match std::str::from_utf8(&bytes) {
                    Ok(s) => self.buffer.push_str(s),
                    Err(e) => {
                        self.done = true;
                        return Poll::Ready(Some(Err(SimpleAgentsError::Provider(
                            ProviderError::InvalidResponse(format!(
                                "Invalid UTF-8 in stream: {}",
                                e
                            )),
                        ))));
                    }
                },
                Poll::Ready(Some(Err(e))) => {
                    self.done = true;
                    return Poll::Ready(Some(Err(SimpleAgentsError::Network(format!(
                        "Stream error: {}",
                        e
                    )))));
                }
                Poll::Ready(None) => {
                    self.done = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl AnthropicSseStream {
    /// Extract complete SSE event from buffer
    fn extract_sse_event(&mut self) -> Option<(String, String)> {
        let mut event_type = None;
        let mut data = None;
        let mut buffer_end = 0;

        let lines: Vec<&str> = self.buffer.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(stripped) = line.strip_prefix("event:") {
                event_type = Some(stripped.trim().to_string());
            } else if let Some(stripped) = line.strip_prefix("data:") {
                data = Some(stripped.trim().to_string());
            }

            // Check if we have a complete event (blank line separates events)
            if event_type.is_some() && data.is_some() {
                // Check if next line is blank (event separator)
                if i + 1 < lines.len() && lines[i + 1].trim().is_empty() {
                    buffer_end = lines[i + 1].len() + 1;
                    break;
                }
            }
        }

        if let (Some(evt), Some(dat)) = (event_type, data) {
            self.buffer = self.buffer[buffer_end..].to_string();
            Some((evt, dat))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_start() {
        let data = r#"{"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant","content":[],"model":"claude-3-opus-20240229","stop_reason":null,"stop_sequence":null}}"#;
        let event_type = "message_start";

        let result = parse_sse_line(event_type, data);
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn test_parse_content_block_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event_type = "content_block_delta";

        let result = parse_sse_line(event_type, data);
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn test_parse_message_stop() {
        let data = r#"{"type":"message_stop"}"#;
        let event_type = "message_stop";

        let result = parse_sse_line(event_type, data);
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn test_parse_ping() {
        let data = r#"{"type":"ping"}"#;
        let event_type = "ping";

        let result = parse_sse_line(event_type, data);
        assert!(result.is_none());
    }

    #[test]
    fn test_transform_text_delta() {
        let mut state = StreamState {
            id: Some("msg_123".to_string()),
            model: Some("claude-3-opus".to_string()),
            content_buffer: String::new(),
            finish_reason: None,
        };

        let event = AnthropicStreamEvent::ContentBlockDelta {
            index: 0,
            delta: StreamDelta::TextDelta {
                text: "Hello".to_string(),
            },
        };

        let result = transform_stream_event(event, &mut state);
        assert!(result.is_some());

        let chunk = result.unwrap().unwrap();
        assert_eq!(chunk.id, "msg_123");
        assert_eq!(chunk.model, "claude-3-opus");
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
        assert_eq!(state.content_buffer, "Hello");
    }

    #[test]
    fn test_transform_message_start() {
        let mut state = StreamState::default();

        let event = AnthropicStreamEvent::MessageStart {
            message: StreamMessage {
                id: "msg_123".to_string(),
                message_type: "message".to_string(),
                role: "assistant".to_string(),
                content: vec![],
                model: "claude-3-opus".to_string(),
                stop_reason: None,
                stop_sequence: None,
            },
        };

        let result = transform_stream_event(event, &mut state);
        assert!(result.is_none());
        assert_eq!(state.id, Some("msg_123".to_string()));
        assert_eq!(state.model, Some("claude-3-opus".to_string()));
    }
}
