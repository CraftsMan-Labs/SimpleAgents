//! Streaming support for OpenAI API via Server-Sent Events (SSE).

use bytes::Bytes;
use futures_util::Stream;
use simple_agent_type::prelude::*;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::models::OpenAIStreamChunk;

/// Parse Server-Sent Events (SSE) stream from OpenAI.
///
/// OpenAI uses SSE format:
/// ```text
/// data: {"id":"chatcmpl-123",...}
/// data: {"id":"chatcmpl-123",...}
/// data: [DONE]
/// ```
///
/// This function:
/// 1. Splits by lines
/// 2. Extracts JSON from "data: " prefix
/// 3. Stops at "[DONE]" sentinel
pub fn parse_sse_line(line: &str) -> Option<Result<OpenAIStreamChunk>> {
    let line = line.trim();

    // Skip empty lines
    if line.is_empty() {
        return None;
    }

    // Skip non-data lines (comments, event types, etc.)
    if !line.starts_with("data: ") {
        return None;
    }

    // Extract JSON after "data: " prefix
    let json = &line[6..];

    // Check for stream termination
    if json == "[DONE]" {
        return None;
    }

    // Parse JSON chunk
    match serde_json::from_str::<OpenAIStreamChunk>(json) {
        Ok(chunk) => Some(Ok(chunk)),
        Err(e) => Some(Err(SimpleAgentsError::Provider(
            ProviderError::InvalidResponse(format!("Failed to parse SSE chunk: {}", e)),
        ))),
    }
}

/// Transform OpenAI stream chunk to unified format.
///
/// Maps OpenAI-specific fields to the standardized `CompletionChunk`.
pub fn transform_openai_chunk(chunk: OpenAIStreamChunk) -> Result<CompletionChunk> {
    let choices = chunk
        .choices
        .into_iter()
        .map(|choice| {
            let role = choice.delta.role.as_ref().and_then(|r| match r.as_str() {
                "assistant" => Some(Role::Assistant),
                "user" => Some(Role::User),
                "system" => Some(Role::System),
                _ => None,
            });

            ChoiceDelta {
                index: choice.index,
                delta: MessageDelta {
                    role,
                    content: choice.delta.content,
                },
                finish_reason: choice.finish_reason.as_ref().map(|s| match s.as_str() {
                    "stop" => FinishReason::Stop,
                    "length" => FinishReason::Length,
                    "content_filter" => FinishReason::ContentFilter,
                    "tool_calls" => FinishReason::ToolCalls,
                    _ => FinishReason::Stop,
                }),
            }
        })
        .collect();

    Ok(CompletionChunk {
        id: chunk.id,
        model: chunk.model,
        choices,
        created: Some(chunk.created as i64),
    })
}

/// Stream wrapper for processing SSE chunks.
///
/// Converts raw bytes into parsed `CompletionChunk` objects.
pub struct SseStream {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    buffer: String,
}

impl SseStream {
    pub fn new(stream: impl Stream<Item = reqwest::Result<Bytes>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(stream),
            buffer: String::new(),
        }
    }
}

impl Stream for SseStream {
    type Item = Result<CompletionChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to parse a complete line from buffer
            if let Some(newline_pos) = self.buffer.find('\n') {
                let line = self.buffer[..newline_pos].to_string();
                self.buffer.drain(..=newline_pos);

                // Parse SSE line
                if let Some(result) = parse_sse_line(&line) {
                    return Poll::Ready(Some(result.and_then(transform_openai_chunk)));
                }
                // If parse_sse_line returns None, continue to next line
                continue;
            }

            // Need more data - poll the inner stream
            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    // Add new data to buffer
                    match std::str::from_utf8(&bytes) {
                        Ok(s) => self.buffer.push_str(s),
                        Err(e) => {
                            return Poll::Ready(Some(Err(SimpleAgentsError::Provider(
                                ProviderError::InvalidResponse(format!(
                                    "Invalid UTF-8 in stream: {}",
                                    e
                                )),
                            ))))
                        }
                    }
                    // Continue loop to try parsing again
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(SimpleAgentsError::Network(format!(
                        "Stream error: {}",
                        e
                    )))))
                }
                Poll::Ready(None) => {
                    // Stream ended - check if there's a final line without newline
                    if !self.buffer.is_empty() {
                        let line = self.buffer.clone();
                        self.buffer.clear();
                        if let Some(result) = parse_sse_line(&line) {
                            return Poll::Ready(Some(result.and_then(transform_openai_chunk)));
                        }
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_line_valid() {
        let line = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;

        let result = parse_sse_line(line);
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn test_parse_sse_line_done() {
        let line = "data: [DONE]";
        let result = parse_sse_line(line);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_line_empty() {
        let line = "";
        let result = parse_sse_line(line);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_line_comment() {
        let line = ": this is a comment";
        let result = parse_sse_line(line);
        assert!(result.is_none());
    }

    #[test]
    fn test_transform_chunk() {
        let chunk = OpenAIStreamChunk {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1677652288,
            model: "gpt-4".to_string(),
            choices: vec![super::super::models::OpenAIStreamChoice {
                index: 0,
                delta: super::super::models::OpenAIDelta {
                    role: Some("assistant".to_string()),
                    content: Some("Hello".to_string()),
                },
                finish_reason: None,
            }],
            system_fingerprint: None,
        };

        let result = transform_openai_chunk(chunk);
        assert!(result.is_ok());

        let unified = result.unwrap();
        assert_eq!(unified.id, "chatcmpl-123");
        assert_eq!(unified.model, "gpt-4");
        assert_eq!(unified.choices.len(), 1);
        assert_eq!(unified.choices[0].delta.content, Some("Hello".to_string()));
    }
}
