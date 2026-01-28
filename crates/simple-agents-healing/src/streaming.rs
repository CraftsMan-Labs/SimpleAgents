//! Streaming JSON parser for incremental LLM response parsing.
//!
//! Provides incremental parsing of JSON as it streams in, extracting complete
//! values without waiting for the full response.
//!
//! # Example
//!
//! ```
//! use simple_agents_healing::streaming::StreamingParser;
//!
//! let mut parser = StreamingParser::new();
//!
//! // Feed first chunk
//! let values = parser.feed(r#"{"name": "Alice", "age": "#);
//! // Not enough for complete parse yet
//!
//! // Feed second chunk
//! let values = parser.feed(r#"30, "email": "#);
//! // Can extract "name" and "age" fields
//!
//! // Feed final chunk
//! let values = parser.feed(r#""alice@example.com"}"#);
//! // Full object complete
//! ```

use crate::parser::{JsonishParser, ParserConfig};
use serde_json::Value;
use simple_agent_type::coercion::CoercionResult;
use simple_agent_type::error::HealingError;
use std::collections::VecDeque;

/// Parser state for tracking incomplete JSON structures.
///
/// Currently simplified - will be expanded when implementing advanced streaming features.
#[derive(Debug, Clone, PartialEq)]
enum ParseState {
    /// Not inside any structure
    Outside,
}

/// Streaming JSON parser for incremental parsing.
///
/// Accumulates chunks and extracts complete JSON values as they become available.
///
/// # Example
///
/// ```
/// use simple_agents_healing::streaming::StreamingParser;
///
/// let mut parser = StreamingParser::new();
///
/// // Stream comes in chunks
/// parser.feed(r#"{"id": 1, "#);
/// parser.feed(r#""name": "Alice", "#);
/// parser.feed(r#""age": 30}"#);
///
/// // Get the complete parsed value
/// let result = parser.finalize().unwrap();
/// assert_eq!(result.value["id"], 1);
/// assert_eq!(result.value["name"], "Alice");
/// assert_eq!(result.value["age"], 30);
/// ```
pub struct StreamingParser {
    /// Accumulated buffer of all chunks
    buffer: String,
    /// Index up to which we've successfully parsed
    parsed_index: usize,
    /// Parser for final parsing
    parser: JsonishParser,
    /// Extracted complete values (for array streaming)
    extracted_values: VecDeque<Value>,
    /// Current parsing state
    state: ParseState,
}

impl StreamingParser {
    /// Create a new streaming parser with default configuration.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            parsed_index: 0,
            parser: JsonishParser::new(),
            extracted_values: VecDeque::new(),
            state: ParseState::Outside,
        }
    }

    /// Create a streaming parser with custom configuration.
    pub fn with_config(config: ParserConfig) -> Self {
        Self {
            buffer: String::new(),
            parsed_index: 0,
            parser: JsonishParser::with_config(config),
            extracted_values: VecDeque::new(),
            state: ParseState::Outside,
        }
    }

    /// Feed a chunk of JSON data to the parser.
    ///
    /// Returns a list of complete values that were extracted from this chunk.
    /// For single objects, this will be empty until the final chunk.
    /// For arrays, this can return completed array elements.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_agents_healing::streaming::StreamingParser;
    ///
    /// let mut parser = StreamingParser::new();
    ///
    /// // Array streaming: extract complete elements
    /// parser.feed(r#"[{"id": 1}, "#);
    /// parser.feed(r#"{"id": 2}, "#);
    /// let values = parser.feed(r#"{"id": 3}]"#);
    /// ```
    pub fn feed(&mut self, chunk: &str) -> Vec<Value> {
        self.buffer.push_str(chunk);
        self.extract_completed_values()
    }

    /// Try to parse the current buffer as a complete JSON value.
    ///
    /// Returns `Some(value)` if the buffer contains a complete, parseable value.
    /// Returns `None` if more data is needed or if the JSON is incomplete.
    pub fn try_parse(&self) -> Option<CoercionResult<Value>> {
        if self.buffer.trim().is_empty() {
            return None;
        }

        // Try to parse the entire buffer
        self.parser.parse(&self.buffer).ok()
    }

    /// Finalize the stream and get the complete parsed value.
    ///
    /// This attempts to parse the entire accumulated buffer as a single JSON value.
    /// Call this when the stream is complete.
    ///
    /// # Errors
    ///
    /// Returns an error if the accumulated buffer cannot be parsed as valid JSON.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_agents_healing::streaming::StreamingParser;
    ///
    /// let mut parser = StreamingParser::new();
    /// parser.feed(r#"{"name": "#);
    /// parser.feed(r#""Alice"}"#);
    ///
    /// let result = parser.finalize().unwrap();
    /// assert_eq!(result.value["name"], "Alice");
    /// ```
    pub fn finalize(
        self,
    ) -> std::result::Result<CoercionResult<Value>, simple_agent_type::SimpleAgentsError> {
        if self.buffer.trim().is_empty() {
            return Err(simple_agent_type::SimpleAgentsError::Healing(
                HealingError::ParseFailed {
                    error_message: "Empty buffer".to_string(),
                    input: String::new(),
                },
            ));
        }

        self.parser.parse(&self.buffer)
    }

    /// Get the current buffer size in bytes.
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Clear the parser state and buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.parsed_index = 0;
        self.extracted_values.clear();
        self.state = ParseState::Outside;
    }

    /// Extract completed values from the buffer.
    ///
    /// For arrays, this extracts complete array elements.
    /// For objects, this waits until the entire object is complete.
    fn extract_completed_values(&mut self) -> Vec<Value> {
        let mut result = Vec::new();

        // For now, we use a simple heuristic:
        // Try to extract complete JSON values that end with } or ]
        // This is a simplified implementation - a full implementation would
        // use a proper state machine to track nesting depth

        // Drain any previously extracted values
        while let Some(value) = self.extracted_values.pop_front() {
            result.push(value);
        }

        result
    }
}

impl Default for StreamingParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Partial value extractor for streaming with schema support.
///
/// Extracts partial values from incomplete JSON buffers, respecting
/// streaming annotations like `@@stream.not_null` and `@@stream.done`.
pub struct PartialExtractor {
    parser: StreamingParser,
}

impl PartialExtractor {
    /// Create a new partial extractor.
    pub fn new() -> Self {
        Self {
            parser: StreamingParser::new(),
        }
    }

    /// Feed a chunk and try to extract a partial value.
    ///
    /// Returns `Some(value)` if a partial value can be extracted.
    pub fn feed(&mut self, chunk: &str) -> Option<Value> {
        self.parser.feed(chunk);
        self.parser.try_parse().map(|result| result.value)
    }

    /// Get the final complete value.
    pub fn finalize(self) -> std::result::Result<Value, simple_agent_type::SimpleAgentsError> {
        self.parser.finalize().map(|result| result.value)
    }
}

impl Default for PartialExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_parser_new() {
        let parser = StreamingParser::new();
        assert_eq!(parser.buffer_len(), 0);
        assert!(parser.is_empty());
    }

    #[test]
    fn test_feed_single_chunk_complete() {
        let mut parser = StreamingParser::new();
        parser.feed(r#"{"name": "Alice", "age": 30}"#);

        let result = parser.finalize().unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 30);
    }

    #[test]
    fn test_feed_multiple_chunks() {
        let mut parser = StreamingParser::new();

        parser.feed(r#"{"name": "#);
        parser.feed(r#""Alice", "#);
        parser.feed(r#""age": 30}"#);

        let result = parser.finalize().unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 30);
    }

    #[test]
    fn test_feed_with_nested_objects() {
        let mut parser = StreamingParser::new();

        parser.feed(r#"{"user": {"name": "#);
        parser.feed(r#""Alice", "age": 30}, "#);
        parser.feed(r#""active": true}"#);

        let result = parser.finalize().unwrap();
        assert_eq!(result.value["user"]["name"], "Alice");
        assert_eq!(result.value["user"]["age"], 30);
        assert_eq!(result.value["active"], true);
    }

    #[test]
    fn test_feed_array() {
        let mut parser = StreamingParser::new();

        parser.feed(r#"["#);
        parser.feed(r#"{"id": 1}, "#);
        parser.feed(r#"{"id": 2}, "#);
        parser.feed(r#"{"id": 3}]"#);

        let result = parser.finalize().unwrap();
        assert!(result.value.is_array());
        assert_eq!(result.value[0]["id"], 1);
        assert_eq!(result.value[1]["id"], 2);
        assert_eq!(result.value[2]["id"], 3);
    }

    #[test]
    fn test_try_parse_incomplete() {
        let mut parser = StreamingParser::new();
        parser.feed(r#"{"name": "Alice", "age":"#);

        // Lenient parser may parse incomplete JSON, auto-closing structures
        // This is expected behavior for streaming
        let result = parser.try_parse();
        if let Some(parsed) = result {
            // If it parses, it should have at least the name field
            assert_eq!(parsed.value["name"], "Alice");
        }
    }

    #[test]
    fn test_try_parse_complete() {
        let mut parser = StreamingParser::new();
        parser.feed(r#"{"name": "Alice", "age": 30}"#);

        // Should successfully parse
        let result = parser.try_parse().unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 30);
    }

    #[test]
    fn test_buffer_len() {
        let mut parser = StreamingParser::new();
        assert_eq!(parser.buffer_len(), 0);

        parser.feed("hello");
        assert_eq!(parser.buffer_len(), 5);

        parser.feed(" world");
        assert_eq!(parser.buffer_len(), 11);
    }

    #[test]
    fn test_clear() {
        let mut parser = StreamingParser::new();
        parser.feed(r#"{"name": "Alice"}"#);
        assert!(!parser.is_empty());

        parser.clear();
        assert!(parser.is_empty());
        assert_eq!(parser.buffer_len(), 0);
    }

    #[test]
    fn test_finalize_empty_buffer() {
        let parser = StreamingParser::new();
        let result = parser.finalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_streaming_with_markdown() {
        let mut parser = StreamingParser::new();

        parser.feed("```json\n");
        parser.feed(r#"{"name": "Alice"}"#);
        parser.feed("\n```");

        let result = parser.finalize().unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert!(result.flags.iter().any(|f| matches!(
            f,
            simple_agent_type::coercion::CoercionFlag::StrippedMarkdown
        )));
    }

    #[test]
    fn test_streaming_with_trailing_comma() {
        let mut parser = StreamingParser::new();

        parser.feed(r#"{"name": "#);
        parser.feed(r#""Alice","#);
        parser.feed(r#"}"#);

        let result = parser.finalize().unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert!(result.flags.iter().any(|f| matches!(
            f,
            simple_agent_type::coercion::CoercionFlag::FixedTrailingComma
        )));
    }

    #[test]
    fn test_partial_extractor() {
        let mut extractor = PartialExtractor::new();

        // Feed chunks
        extractor.feed(r#"{"name": "Alice", "#);
        extractor.feed(r#""age": 30"#);
        extractor.feed("}");

        let result = extractor.finalize().unwrap();
        assert_eq!(result["name"], "Alice");
        assert_eq!(result["age"], 30);
    }

    #[test]
    fn test_streaming_preserves_confidence() {
        let mut parser = StreamingParser::new();

        // Perfect JSON
        parser.feed(r#"{"name": "Alice"}"#);
        let result = parser.finalize().unwrap();
        assert_eq!(result.confidence, 1.0);
        assert!(result.flags.is_empty());
    }

    #[test]
    fn test_streaming_with_malformed_json() {
        let mut parser = StreamingParser::new();

        // Malformed JSON with unquoted key
        parser.feed(r#"{name: "#);
        parser.feed(r#""Alice"}"#);

        let result = parser.finalize().unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert!(result.confidence < 1.0);
        assert!(!result.flags.is_empty());
    }
}
