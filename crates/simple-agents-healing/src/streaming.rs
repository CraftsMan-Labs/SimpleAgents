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

/// Returns `true` for errors that indicate a hard limit violation (input too
/// large, nesting too deep) as opposed to "not enough data yet" parse failures.
fn is_hard_parse_error(error: &simple_agent_type::SimpleAgentsError) -> bool {
    match error {
        simple_agent_type::SimpleAgentsError::Healing(HealingError::ParseFailed {
            error_message,
            ..
        }) => {
            error_message.contains("exceeds maximum allowed")
        }
        _ => false,
    }
}

/// Parser state for tracking incomplete JSON structures.
#[derive(Debug, Clone, PartialEq)]
enum ParseState {
    /// Not inside any structure yet
    Outside,
    /// Inside a top-level array (streaming elements)
    InArray,
    /// Inside a top-level object (wait for completion)
    InObject,
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
    /// Returns `Ok(Some(value))` if the buffer contains a complete, parseable value.
    /// Returns `Ok(None)` if more data is needed (buffer empty or incomplete).
    /// Returns `Err(...)` if the buffer contains data that is definitively invalid
    /// (e.g. exceeds input size limits or nesting depth).
    pub fn try_parse(
        &self,
    ) -> std::result::Result<Option<CoercionResult<Value>>, simple_agent_type::SimpleAgentsError>
    {
        if self.buffer.trim().is_empty() {
            return Ok(None);
        }

        match self.parser.parse(&self.buffer) {
            Ok(result) => Ok(Some(result)),
            Err(e) => {
                if is_hard_parse_error(&e) {
                    Err(e)
                } else {
                    Ok(None)
                }
            }
        }
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
    /// For arrays, this extracts complete array elements as they become available.
    /// For objects, this waits until the entire object is complete.
    fn extract_completed_values(&mut self) -> Vec<Value> {
        let mut result = Vec::new();

        // Drain any previously extracted values first
        while let Some(value) = self.extracted_values.pop_front() {
            result.push(value);
        }

        // Determine top-level structure if we haven't yet
        if self.state == ParseState::Outside {
            let trimmed_start = self.buffer[self.parsed_index..].trim_start();
            if trimmed_start.starts_with('[') {
                self.state = ParseState::InArray;
                // Advance parsed_index past the opening bracket
                let offset = self.buffer.len() - trimmed_start.len();
                self.parsed_index = offset + 1; // skip '['
            } else if trimmed_start.starts_with('{') {
                self.state = ParseState::InObject;
            } else {
                return result;
            }
        }

        if self.state == ParseState::InArray {
            self.extract_array_elements(&mut result);
        }

        result
    }

    /// Extract complete array elements from the buffer using depth/string tracking.
    ///
    /// Scans from `parsed_index` forward, tracking brace/bracket depth and string
    /// state. When depth returns to 0 after a value and we encounter a comma or `]`,
    /// we have a complete element to extract.
    fn extract_array_elements(&mut self, output: &mut Vec<Value>) {
        let bytes = self.buffer.as_bytes();
        let len = bytes.len();

        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;
        let mut element_start: Option<usize> = None;
        let mut i = self.parsed_index;

        while i < len {
            let b = bytes[i];

            if escaped {
                escaped = false;
                i += 1;
                continue;
            }

            if in_string {
                match b {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
                i += 1;
                continue;
            }

            match b {
                b'"' => {
                    in_string = true;
                    if element_start.is_none() {
                        element_start = Some(i);
                    }
                }
                b'{' | b'[' => {
                    if element_start.is_none() {
                        element_start = Some(i);
                    }
                    depth += 1;
                }
                b'}' | b']' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        // End of a nested structure at depth-0 in the array
                        // The element runs from element_start to i (inclusive)
                        if let Some(start) = element_start {
                            let element_str = &self.buffer[start..=i];
                            if let Ok(value) = serde_json::from_str::<Value>(element_str) {
                                output.push(value);
                            }
                            element_start = None;
                            self.parsed_index = i + 1;
                        }
                    }
                }
                b']' if depth == 0 => {
                    // Closing bracket of the top-level array
                    // If there's a pending element (primitive), flush it
                    if let Some(start) = element_start {
                        let element_str = self.buffer[start..i].trim();
                        if !element_str.is_empty() {
                            if let Ok(value) = serde_json::from_str::<Value>(element_str) {
                                output.push(value);
                            }
                        }
                    }
                    self.parsed_index = i + 1;
                    return;
                }
                b',' if depth == 0 => {
                    // Comma at top-level array depth → end of a primitive element
                    if let Some(start) = element_start {
                        let element_str = self.buffer[start..i].trim();
                        if !element_str.is_empty() {
                            if let Ok(value) = serde_json::from_str::<Value>(element_str) {
                                output.push(value);
                            }
                        }
                        element_start = None;
                        self.parsed_index = i + 1;
                    } else {
                        self.parsed_index = i + 1;
                    }
                }
                b if !b.is_ascii_whitespace() && element_start.is_none() && depth == 0 => {
                    element_start = Some(i);
                }
                _ => {}
            }

            i += 1;
        }
    }
}

impl Default for StreamingParser {
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
        let result = parser.try_parse().unwrap();
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
        let result = parser.try_parse().unwrap().unwrap();
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

    // --- Task 2A: Incremental array extraction tests ---

    #[test]
    fn test_feed_incremental_chunked_array() {
        let mut parser = StreamingParser::new();

        let values1 = parser.feed(r#"[{"a":1},"#);
        assert_eq!(values1.len(), 1);
        assert_eq!(values1[0]["a"], 1);

        let values2 = parser.feed(r#"{"b":2}]"#);
        assert_eq!(values2.len(), 1);
        assert_eq!(values2[0]["b"], 2);
    }

    #[test]
    fn test_feed_incremental_nested_objects() {
        let mut parser = StreamingParser::new();

        let values = parser.feed(r#"[{"outer":{"inner":1}},{"outer":{"inner":2}}]"#);
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["outer"]["inner"], 1);
        assert_eq!(values[1]["outer"]["inner"], 2);
    }

    #[test]
    fn test_feed_incremental_strings_with_commas() {
        let mut parser = StreamingParser::new();

        // Strings containing commas/brackets should NOT cause premature splits
        let values = parser.feed(r#"[{"msg":"hello, world"},{"msg":"a[b]c"}]"#);
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["msg"], "hello, world");
        assert_eq!(values[1]["msg"], "a[b]c");
    }

    #[test]
    fn test_feed_incremental_empty_array() {
        let mut parser = StreamingParser::new();

        let values = parser.feed(r#"[]"#);
        assert!(values.is_empty());
    }

    #[test]
    fn test_feed_incremental_single_element_array() {
        let mut parser = StreamingParser::new();

        let values = parser.feed(r#"[{"id":42}]"#);
        assert_eq!(values.len(), 1);
        assert_eq!(values[0]["id"], 42);
    }

    #[test]
    fn test_feed_incremental_primitives() {
        let mut parser = StreamingParser::new();

        let values1 = parser.feed(r#"[1, 2,"#);
        assert_eq!(values1.len(), 2);
        assert_eq!(values1[0], 1);
        assert_eq!(values1[1], 2);

        let values2 = parser.feed(r#" 3]"#);
        assert_eq!(values2.len(), 1);
        assert_eq!(values2[0], 3);
    }

    #[test]
    fn test_feed_incremental_partial_element() {
        let mut parser = StreamingParser::new();

        // First chunk has incomplete element
        let values1 = parser.feed(r#"[{"name":"Al"#);
        assert!(values1.is_empty());

        // Complete the element
        let values2 = parser.feed(r#"ice"},{"name":"Bob"}]"#);
        assert_eq!(values2.len(), 2);
        assert_eq!(values2[0]["name"], "Alice");
        assert_eq!(values2[1]["name"], "Bob");
    }
}
