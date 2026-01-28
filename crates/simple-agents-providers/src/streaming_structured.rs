//! Structured output support for streaming responses.
//!
//! Provides progressive parsing and healing for streaming JSON outputs.

use futures_core::Stream;
use pin_project_lite::pin_project;
use serde::de::DeserializeOwned;
use serde_json::Value;
use simple_agent_type::error::{HealingError, SimpleAgentsError};
use simple_agent_type::response::CompletionChunk;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::healing_integration::HealingIntegration;

/// Event emitted during structured streaming.
#[derive(Debug, Clone)]
pub enum StructuredEvent<T> {
    /// Partial value (progressive parsing, may be incomplete)
    Partial(T),
    /// Complete final value with healing metadata
    Complete {
        /// The final parsed value
        value: T,
        /// Confidence score (1.0 if no healing applied)
        confidence: f32,
        /// Whether healing was applied
        was_healed: bool,
    },
}

pin_project! {
    /// Stream wrapper that accumulates chunks and provides structured output.
    ///
    /// # Example
    /// ```ignore
    /// let stream = provider.execute_stream(request).await?;
    /// let structured = StructuredStream::new(stream, schema, Some(healing));
    ///
    /// while let Some(event) = structured.next().await {
    ///     match event? {
    ///         StructuredEvent::Partial(val) => println!("Partial: {:?}", val),
    ///         StructuredEvent::Complete { value, confidence, .. } => {
    ///             println!("Final: {:?} (confidence: {})", value, confidence);
    ///         }
    ///     }
    /// }
    /// ```
    pub struct StructuredStream<S, T> {
        #[pin]
        inner: S,
        accumulated: String,
        json_schema: Value,
        healing: Option<HealingIntegration>,
        completed: bool,
        _phantom: PhantomData<T>,
    }
}

impl<S, T> StructuredStream<S, T>
where
    S: Stream<Item = Result<CompletionChunk, SimpleAgentsError>>,
    T: DeserializeOwned,
{
    /// Create a new structured stream.
    pub fn new(stream: S, json_schema: Value, healing: Option<HealingIntegration>) -> Self {
        Self {
            inner: stream,
            accumulated: String::new(),
            json_schema,
            healing,
            completed: false,
            _phantom: PhantomData,
        }
    }

    /// Try to parse accumulated content as structured output.
    fn try_parse(accumulated: &str) -> Result<T, SimpleAgentsError> {
        serde_json::from_str(accumulated).map_err(|e| {
            SimpleAgentsError::Provider(simple_agent_type::error::ProviderError::InvalidResponse(
                format!("Failed to parse JSON: {}", e),
            ))
        })
    }

    /// Try to parse with healing if available.
    fn try_parse_with_healing(
        accumulated: &str,
        json_schema: &Value,
        healing: &Option<HealingIntegration>,
    ) -> Result<(T, f32, bool), SimpleAgentsError> {
        // Try native parsing first
        match Self::try_parse(accumulated) {
            Ok(value) => Ok((value, 1.0, false)),
            Err(parse_error) => {
                // Try healing if enabled
                if let Some(healing) = healing {
                    let healed = healing.heal_response(
                        accumulated,
                        json_schema,
                        &parse_error.to_string(),
                    )?;

                    let value: T = serde_json::from_value(healed.value).map_err(|e| {
                        SimpleAgentsError::Healing(HealingError::ParseFailed {
                            error_message: format!("Deserialization failed: {}", e),
                            input: accumulated.to_string(),
                        })
                    })?;

                    Ok((value, healed.metadata.confidence, true))
                } else {
                    Err(parse_error)
                }
            }
        }
    }
}

impl<S, T> Stream for StructuredStream<S, T>
where
    S: Stream<Item = Result<CompletionChunk, SimpleAgentsError>>,
    T: DeserializeOwned,
{
    type Item = Result<StructuredEvent<T>, SimpleAgentsError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if *this.completed {
            return Poll::Ready(None);
        }

        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                // Accumulate content from chunk
                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        this.accumulated.push_str(content);
                    }

                    // If stream finished, parse final result
                    if choice.finish_reason.is_some() {
                        *this.completed = true;

                        match Self::try_parse_with_healing(
                            this.accumulated,
                            this.json_schema,
                            this.healing,
                        ) {
                            Ok((value, confidence, was_healed)) => {
                                return Poll::Ready(Some(Ok(StructuredEvent::Complete {
                                    value,
                                    confidence,
                                    was_healed,
                                })));
                            }
                            Err(e) => return Poll::Ready(Some(Err(e))),
                        }
                    }
                }

                // Continue accumulating without emitting partial events
                // (progressive parsing is optional and often fails mid-stream)
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Poll::Ready(Some(Err(e))) => {
                *this.completed = true;
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                // Stream ended, try final parse
                *this.completed = true;

                if this.accumulated.is_empty() {
                    return Poll::Ready(None);
                }

                match Self::try_parse_with_healing(this.accumulated, this.json_schema, this.healing)
                {
                    Ok((value, confidence, was_healed)) => {
                        Poll::Ready(Some(Ok(StructuredEvent::Complete {
                            value,
                            confidence,
                            was_healed,
                        })))
                    }
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use serde::{Deserialize, Serialize};
    use simple_agent_type::response::{ChoiceDelta, MessageDelta};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        message: String,
    }

    #[tokio::test]
    async fn test_structured_stream_basic() {
        use futures_util::StreamExt;

        // Create mock chunks - accumulate complete JSON before finish_reason
        let chunks = vec![
            Ok(CompletionChunk {
                id: "1".to_string(),
                model: "test".to_string(),
                choices: vec![ChoiceDelta {
                    index: 0,
                    delta: MessageDelta {
                        role: None,
                        content: Some(r#"{"message":"Hello"}"#.to_string()),
                    },
                    finish_reason: None,
                }],
                created: None,
            }),
            Ok(CompletionChunk {
                id: "2".to_string(),
                model: "test".to_string(),
                choices: vec![ChoiceDelta {
                    index: 0,
                    delta: MessageDelta {
                        role: None,
                        content: None,
                    },
                    finish_reason: Some(simple_agent_type::response::FinishReason::Stop),
                }],
                created: None,
            }),
        ];

        let stream = stream::iter(chunks);
        let json_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"}
            }
        });

        let mut structured: StructuredStream<_, TestData> =
            StructuredStream::new(stream, json_schema, None);

        let mut events = vec![];
        while let Some(event) = structured.next().await {
            events.push(event.unwrap());
        }

        // Should have exactly one complete event
        assert_eq!(events.len(), 1);

        // Event should be Complete
        if let StructuredEvent::Complete {
            value,
            confidence,
            was_healed,
        } = &events[0]
        {
            assert_eq!(value.message, "Hello");
            assert_eq!(*confidence, 1.0);
            assert!(!was_healed);
        } else {
            panic!("Expected Complete event");
        }
    }
}
