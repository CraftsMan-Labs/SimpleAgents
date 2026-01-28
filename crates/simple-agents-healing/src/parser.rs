//! JSON-ish parser for handling malformed LLM outputs.
//!
//! Implements a three-phase parsing strategy:
//!
//! 1. **Strip & Fix**: Remove markdown, fix trailing commas, normalize quotes
//! 2. **Standard Parse**: Try `serde_json` (fast path)
//! 3. **Lenient Parse**: Character-by-character state machine for incomplete/malformed JSON
//!
//! # Example
//!
//! ```
//! use simple_agents_healing::parser::JsonishParser;
//!
//! let parser = JsonishParser::new();
//!
//! // Parse markdown-wrapped JSON
//! let malformed = r#"```json
//! {"key": "value", "num": 42,}
//! ```"#;
//!
//! let result = parser.parse(malformed).unwrap();
//! assert_eq!(result.value["key"], "value");
//! assert_eq!(result.value["num"], 42);
//! ```

use serde_json::Value;
use simple_agent_type::coercion::{CoercionFlag, CoercionResult};
use simple_agent_type::error::{HealingError, Result};
use std::fmt;
use tracing::{debug, trace, warn};

/// Parser configuration options.
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Enable markdown stripping (```json ... ```)
    pub strip_markdown: bool,
    /// Enable trailing comma fixes
    pub fix_trailing_commas: bool,
    /// Enable quote normalization (single → double)
    pub fix_quotes: bool,
    /// Enable unquoted key fixes ({key: "value"} → {"key": "value"})
    pub fix_unquoted_keys: bool,
    /// Enable control character fixes
    pub fix_control_chars: bool,
    /// Enable BOM removal
    pub remove_bom: bool,
    /// Enable lenient parsing (state machine for incomplete JSON)
    pub allow_lenient_parsing: bool,
    /// Minimum confidence threshold (0.0-1.0)
    pub min_confidence: f32,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            strip_markdown: true,
            fix_trailing_commas: true,
            fix_quotes: true,
            fix_unquoted_keys: true,
            fix_control_chars: true,
            remove_bom: true,
            allow_lenient_parsing: true,
            min_confidence: 0.5,
        }
    }
}

/// Result of parsing with coercion tracking.
pub type ParserResult = CoercionResult<Value>;

/// Three-phase JSON parser for malformed LLM outputs.
///
/// # Phases
///
/// 1. **Strip & Fix**: Quick string transformations
/// 2. **Standard Parse**: Attempt `serde_json::from_str` (fast path)
/// 3. **Lenient Parse**: State machine for incomplete/malformed JSON
///
/// # Example
///
/// ```
/// use simple_agents_healing::parser::JsonishParser;
///
/// let parser = JsonishParser::new();
/// let result = parser.parse(r#"{"key": "value",}"#).unwrap();
/// assert!(result.flags.iter().any(|f| matches!(f,
///     simple_agent_type::coercion::CoercionFlag::FixedTrailingComma)));
/// ```
pub struct JsonishParser {
    config: ParserConfig,
}

impl JsonishParser {
    /// Create a new parser with default configuration.
    pub fn new() -> Self {
        Self {
            config: ParserConfig::default(),
        }
    }

    /// Create a parser with custom configuration.
    pub fn with_config(config: ParserConfig) -> Self {
        Self { config }
    }

    /// Parse potentially malformed JSON.
    ///
    /// Returns a [`ParserResult`] containing the parsed value, flags indicating
    /// transformations applied, and a confidence score.
    ///
    /// # Errors
    ///
    /// Returns [`HealingError::ParseFailed`] if all parsing phases fail.
    /// Returns [`HealingError::LowConfidence`] if confidence is below threshold.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_agents_healing::parser::JsonishParser;
    ///
    /// let parser = JsonishParser::new();
    ///
    /// // Perfect JSON - no healing needed
    /// let result = parser.parse(r#"{"key": "value"}"#).unwrap();
    /// assert_eq!(result.confidence, 1.0);
    /// assert!(result.flags.is_empty());
    ///
    /// // Malformed JSON - healing applied
    /// let result = parser.parse(r#"{"key": "value",}"#).unwrap();
    /// assert!(result.confidence < 1.0);
    /// assert!(!result.flags.is_empty());
    /// ```
    pub fn parse(&self, input: &str) -> Result<ParserResult> {
        trace!("Starting JSON parse: {} bytes", input.len());

        let mut flags = Vec::new();
        let mut confidence = 1.0;

        // Phase 1: Strip & Fix
        let cleaned = self.strip_and_fix(input, &mut flags, &mut confidence)?;

        // Phase 2: Try standard parsing
        if let Ok(value) = serde_json::from_str::<Value>(&cleaned) {
            debug!("Standard parse succeeded with {} flags", flags.len());
            let result = CoercionResult {
                value,
                flags,
                confidence,
            };

            return self.check_confidence(result);
        }

        // Phase 3: Lenient parsing (if enabled)
        if self.config.allow_lenient_parsing {
            warn!("Standard parse failed, attempting lenient parse");
            let value = self.lenient_parse(&cleaned, &mut flags, &mut confidence)?;

            let result = CoercionResult {
                value,
                flags,
                confidence,
            };

            return self.check_confidence(result);
        }

        // All parsing failed
        Err(HealingError::ParseFailed {
            error_message: "Could not parse JSON".to_string(),
            input: input.to_string(),
        }
        .into())
    }

    /// Phase 1: Strip markdown and fix common issues.
    fn strip_and_fix(
        &self,
        input: &str,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<String> {
        let mut output = input.to_string();

        // Remove BOM
        if self.config.remove_bom && output.starts_with('\u{FEFF}') {
            output = output.trim_start_matches('\u{FEFF}').to_string();
            flags.push(CoercionFlag::RemovedBom);
            *confidence *= 0.99; // Minimal impact
        }

        // Strip markdown code blocks
        if self.config.strip_markdown {
            if let Some(stripped) = self.strip_markdown(&output) {
                output = stripped;
                flags.push(CoercionFlag::StrippedMarkdown);
                *confidence *= 0.95;
            }
        }

        // Fix trailing commas
        if self.config.fix_trailing_commas && (output.contains(",}") || output.contains(",]")) {
            output = output.replace(",}", "}").replace(",]", "]");
            flags.push(CoercionFlag::FixedTrailingComma);
            *confidence *= 0.95;
        }

        // Fix single quotes (only if no double quotes present)
        if self.config.fix_quotes && output.contains('\'') && !output.contains('"') {
            output = output.replace('\'', "\"");
            flags.push(CoercionFlag::FixedQuotes);
            *confidence *= 0.90;
        }

        // Fix control characters
        if self.config.fix_control_chars {
            let original_len = output.len();
            output = output
                .chars()
                .filter(|c| !c.is_control() || c.is_whitespace())
                .collect();
            if output.len() != original_len {
                flags.push(CoercionFlag::FixedControlCharacters);
                *confidence *= 0.90;
            }
        }

        // Fix unquoted keys (basic)
        if self.config.fix_unquoted_keys {
            // This is a simplified version - full implementation would need a parser
            if let Some(fixed) = self.fix_unquoted_keys_simple(&output) {
                output = fixed;
                flags.push(CoercionFlag::FixedUnquotedKeys);
                *confidence *= 0.85;
            }
        }

        Ok(output)
    }

    /// Strip markdown code fences.
    fn strip_markdown(&self, input: &str) -> Option<String> {
        let trimmed = input.trim();

        // Check for ```json ... ``` or ``` ... ```
        if trimmed.starts_with("```") {
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() >= 2 {
                // Remove first line (```json or ```) and last line (```)
                let start = 1; // Always skip first line (```json or ```)

                let end = if lines.last().map(|l| l.trim()) == Some("```") {
                    lines.len() - 1
                } else {
                    lines.len()
                };

                if end > start {
                    let content = lines[start..end].join("\n");
                    return Some(content);
                }
            }
        }

        None
    }

    /// Simple unquoted key fixer (handles common cases).
    fn fix_unquoted_keys_simple(&self, _input: &str) -> Option<String> {
        // This is a simplified implementation
        // Full version would need proper parsing
        // For now, we skip this fix if regex feature is not enabled
        #[cfg(feature = "regex-support")]
        {
            let pattern = regex::Regex::new(r"([{,]\s*)([a-zA-Z_][a-zA-Z0-9_]*)\s*:").ok()?;

            if pattern.is_match(_input) {
                let result = pattern.replace_all(_input, r#"$1"$2":"#).to_string();
                return Some(result);
            }
        }

        None
    }
}

/// State machine for lenient JSON parsing.
///
/// Tracks parsing state and handles incomplete/malformed JSON structures.
#[derive(Debug)]
struct LenientParserState {
    /// Stack of nested collections (objects/arrays)
    stack: Vec<CollectionState>,
    /// Current state
    state: ParseState,
    /// Completed values (for multiple top-level values)
    completed: Vec<Value>,
    /// Flags to track transformations
    flags: Vec<CoercionFlag>,
    /// Current string being built
    current_string: String,
    /// Current number being built
    current_number: String,
    /// Current key in object
    current_key: Option<String>,
    /// Whether we're escaping the next character
    is_escaped: bool,
}

/// Parser state for the state machine.
#[derive(Debug, Clone, PartialEq)]
enum ParseState {
    /// Expecting a value (start of parsing or after comma)
    ExpectValue,
    /// Inside a string (with delimiter type)
    InString(StringDelimiter),
    /// Inside a number
    InNumber,
    /// Expecting object key or }
    ExpectKey,
    /// Expecting : after key
    ExpectColon,
    /// After value in object, expecting , or }
    AfterValue,
    /// After value in array, expecting , or ]
    AfterArrayValue,
    /// Inside a line comment (//)
    InLineComment,
    /// Inside a block comment (/* */)
    InBlockComment,
}

/// String delimiter types.
#[derive(Debug, Clone, Copy, PartialEq)]
enum StringDelimiter {
    /// Double quotes: "..."
    Double,
    /// Single quotes: '...'
    Single,
    /// Triple double quotes: """..."""
    TripleDouble,
    /// Triple single quotes: '''...'''
    TripleSingle,
    /// Backtick: `...`
    Backtick,
    /// Unquoted (for keys): key
    Unquoted,
}

/// Collection being built (object or array).
#[derive(Debug)]
enum CollectionState {
    /// Object: keys and values
    Object {
        keys: Vec<String>,
        values: Vec<Value>,
    },
    /// Array: values only
    Array { values: Vec<Value> },
}

impl LenientParserState {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            state: ParseState::ExpectValue,
            completed: Vec::new(),
            flags: Vec::new(),
            current_string: String::new(),
            current_number: String::new(),
            current_key: None,
            is_escaped: false,
        }
    }

    /// Process a single character.
    ///
    /// Returns the number of characters to advance (usually 1, but may be more for multi-char tokens).
    fn process_char(&mut self, ch: char, next: Option<char>, next2: Option<char>) -> Result<usize> {
        // Handle escape sequences in strings
        if self.is_escaped {
            self.current_string.push(ch);
            self.is_escaped = false;
            return Ok(1);
        }

        match &self.state {
            ParseState::ExpectValue => self.handle_expect_value(ch, next, next2),
            ParseState::InString(delim) => self.handle_in_string(ch, next, next2, *delim),
            ParseState::InNumber => self.handle_in_number(ch),
            ParseState::ExpectKey => self.handle_expect_key(ch, next, next2),
            ParseState::ExpectColon => self.handle_expect_colon(ch),
            ParseState::AfterValue => self.handle_after_value(ch, next),
            ParseState::AfterArrayValue => self.handle_after_array_value(ch, next),
            ParseState::InLineComment => self.handle_line_comment(ch),
            ParseState::InBlockComment => self.handle_block_comment(ch, next),
        }
    }

    fn handle_expect_value(
        &mut self,
        ch: char,
        next: Option<char>,
        next2: Option<char>,
    ) -> Result<usize> {
        match ch {
            // Whitespace - skip
            ' ' | '\t' | '\n' | '\r' => Ok(1),

            // Start object
            '{' => {
                self.stack.push(CollectionState::Object {
                    keys: Vec::new(),
                    values: Vec::new(),
                });
                self.state = ParseState::ExpectKey;
                Ok(1)
            }

            // Start array
            '[' => {
                self.stack
                    .push(CollectionState::Array { values: Vec::new() });
                Ok(1)
            }

            // Triple quote strings
            '"' if next == Some('"') && next2 == Some('"') => {
                self.state = ParseState::InString(StringDelimiter::TripleDouble);
                self.current_string.clear();
                Ok(3)
            }
            '\'' if next == Some('\'') && next2 == Some('\'') => {
                self.state = ParseState::InString(StringDelimiter::TripleSingle);
                self.current_string.clear();
                Ok(3)
            }

            // Regular strings
            '"' => {
                self.state = ParseState::InString(StringDelimiter::Double);
                self.current_string.clear();
                Ok(1)
            }
            '\'' => {
                self.state = ParseState::InString(StringDelimiter::Single);
                self.current_string.clear();
                Ok(1)
            }
            '`' => {
                self.state = ParseState::InString(StringDelimiter::Backtick);
                self.current_string.clear();
                Ok(1)
            }

            // Comments
            '/' if next == Some('/') => {
                self.state = ParseState::InLineComment;
                Ok(2)
            }
            '/' if next == Some('*') => {
                self.state = ParseState::InBlockComment;
                Ok(2)
            }

            // Numbers
            '-' | '0'..='9' => {
                self.state = ParseState::InNumber;
                self.current_number.clear();
                self.current_number.push(ch);
                Ok(1)
            }

            // Boolean/null literals
            't' | 'f' | 'n' => self.handle_literal(ch),

            // Unexpected character
            _ => {
                trace!(
                    "Skipping unexpected character '{}' when expecting value",
                    ch
                );
                Ok(1) // Skip it
            }
        }
    }

    fn handle_in_string(
        &mut self,
        ch: char,
        next: Option<char>,
        next2: Option<char>,
        delim: StringDelimiter,
    ) -> Result<usize> {
        match (ch, delim) {
            // Escape sequence
            ('\\', _) => {
                self.is_escaped = true;
                self.current_string.push(ch);
                Ok(1)
            }

            // End of triple quote string
            ('"', StringDelimiter::TripleDouble) if next == Some('"') && next2 == Some('"') => {
                self.finish_string();
                Ok(3)
            }
            ('\'', StringDelimiter::TripleSingle) if next == Some('\'') && next2 == Some('\'') => {
                self.finish_string();
                Ok(3)
            }

            // End of regular string
            ('"', StringDelimiter::Double)
            | ('\'', StringDelimiter::Single)
            | ('`', StringDelimiter::Backtick) => {
                self.finish_string();
                Ok(1)
            }

            // End unquoted string (whitespace, comma, colon, brace, bracket)
            (c, StringDelimiter::Unquoted)
                if c.is_whitespace() || matches!(c, ',' | ':' | '}' | ']') =>
            {
                self.finish_string();
                Ok(0) // Don't consume this character
            }

            // Regular character in string
            _ => {
                self.current_string.push(ch);
                Ok(1)
            }
        }
    }

    fn finish_string(&mut self) {
        // Check if this is a boolean or null literal (from unquoted strings)
        let value = match self.current_string.as_str() {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "null" => Value::Null,
            s => Value::String(s.to_string()),
        };

        self.push_value(value);
        self.current_string.clear();
    }

    fn handle_in_number(&mut self, ch: char) -> Result<usize> {
        match ch {
            // Valid number characters
            '0'..='9' | '.' | 'e' | 'E' | '+' | '-' => {
                self.current_number.push(ch);
                Ok(1)
            }

            // End of number
            _ => {
                self.finish_number()?;
                Ok(0) // Don't consume this character
            }
        }
    }

    fn finish_number(&mut self) -> Result<()> {
        let num_str = self.current_number.trim();

        // Try parsing as integer first
        if let Ok(i) = num_str.parse::<i64>() {
            self.push_value(Value::Number(serde_json::Number::from(i)));
        }
        // Try as float
        else if let Ok(f) = num_str.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(f) {
                self.push_value(Value::Number(num));
            } else {
                // Invalid float
                self.push_value(Value::String(num_str.to_string()));
            }
        }
        // Not a valid number - treat as string
        else {
            self.push_value(Value::String(num_str.to_string()));
        }

        self.current_number.clear();
        Ok(())
    }

    fn handle_literal(&mut self, ch: char) -> Result<usize> {
        // Only accept true, false, null as literals
        // Don't accept arbitrary unquoted strings at top level
        match ch {
            't' => {
                self.state = ParseState::InString(StringDelimiter::Unquoted);
                self.current_string.clear();
                self.current_string.push(ch);
                Ok(1)
            }
            'f' => {
                self.state = ParseState::InString(StringDelimiter::Unquoted);
                self.current_string.clear();
                self.current_string.push(ch);
                Ok(1)
            }
            'n' => {
                self.state = ParseState::InString(StringDelimiter::Unquoted);
                self.current_string.clear();
                self.current_string.push(ch);
                Ok(1)
            }
            _ => Ok(1), // Skip unknown characters
        }
    }

    fn handle_expect_key(
        &mut self,
        ch: char,
        next: Option<char>,
        _next2: Option<char>,
    ) -> Result<usize> {
        match ch {
            // Whitespace - skip
            ' ' | '\t' | '\n' | '\r' => Ok(1),

            // Empty object
            '}' => {
                self.finish_collection()?;
                Ok(1)
            }

            // Comments
            '/' if next == Some('/') => {
                self.state = ParseState::InLineComment;
                Ok(2)
            }
            '/' if next == Some('*') => {
                self.state = ParseState::InBlockComment;
                Ok(2)
            }

            // Quoted key
            '"' => {
                self.state = ParseState::InString(StringDelimiter::Double);
                self.current_string.clear();
                Ok(1)
            }
            '\'' => {
                self.state = ParseState::InString(StringDelimiter::Single);
                self.current_string.clear();
                Ok(1)
            }

            // Unquoted key
            'a'..='z' | 'A'..='Z' | '_' => {
                self.flags.push(CoercionFlag::FixedUnquotedKeys);
                self.state = ParseState::InString(StringDelimiter::Unquoted);
                self.current_string.clear();
                self.current_string.push(ch);
                Ok(1)
            }

            _ => {
                warn!("Unexpected character '{}' when expecting object key", ch);
                Ok(1)
            }
        }
    }

    fn handle_expect_colon(&mut self, ch: char) -> Result<usize> {
        match ch {
            ' ' | '\t' | '\n' | '\r' => Ok(1),
            ':' => {
                self.state = ParseState::ExpectValue;
                Ok(1)
            }
            _ => {
                warn!("Expected ':' but got '{}'", ch);
                // Be lenient - assume missing colon
                self.state = ParseState::ExpectValue;
                Ok(0)
            }
        }
    }

    fn handle_after_value(&mut self, ch: char, next: Option<char>) -> Result<usize> {
        match ch {
            ' ' | '\t' | '\n' | '\r' => Ok(1),
            ',' => {
                self.state = ParseState::ExpectKey;
                Ok(1)
            }
            '}' => {
                self.finish_collection()?;
                Ok(1)
            }
            '/' if next == Some('/') => {
                self.state = ParseState::InLineComment;
                Ok(2)
            }
            '/' if next == Some('*') => {
                self.state = ParseState::InBlockComment;
                Ok(2)
            }
            _ => {
                warn!("Expected ',' or '}}' but got '{}'", ch);
                Ok(1)
            }
        }
    }

    fn handle_after_array_value(&mut self, ch: char, next: Option<char>) -> Result<usize> {
        match ch {
            ' ' | '\t' | '\n' | '\r' => Ok(1),
            ',' => {
                self.state = ParseState::ExpectValue;
                Ok(1)
            }
            ']' => {
                self.finish_collection()?;
                Ok(1)
            }
            '/' if next == Some('/') => {
                self.state = ParseState::InLineComment;
                Ok(2)
            }
            '/' if next == Some('*') => {
                self.state = ParseState::InBlockComment;
                Ok(2)
            }
            _ => {
                warn!("Expected ',' or ']' but got '{}'", ch);
                Ok(1)
            }
        }
    }

    fn handle_line_comment(&mut self, ch: char) -> Result<usize> {
        if ch == '\n' {
            // End of line comment - return to previous state
            self.state = ParseState::ExpectValue;
        }
        Ok(1)
    }

    fn handle_block_comment(&mut self, ch: char, next: Option<char>) -> Result<usize> {
        if ch == '*' && next == Some('/') {
            // End of block comment
            self.state = ParseState::ExpectValue;
            Ok(2)
        } else {
            Ok(1)
        }
    }

    fn push_value(&mut self, value: Value) {
        match self.stack.last_mut() {
            Some(CollectionState::Object { keys, values }) => {
                if let Some(key) = self.current_key.take() {
                    // This is a value in an object
                    keys.push(key);
                    values.push(value);
                    self.state = ParseState::AfterValue;
                } else {
                    // This is a key
                    if let Value::String(s) = value {
                        self.current_key = Some(s);
                        self.state = ParseState::ExpectColon;
                    }
                }
            }
            Some(CollectionState::Array { values }) => {
                values.push(value);
                self.state = ParseState::AfterArrayValue;
            }
            None => {
                // Top-level value - only accept valid JSON types
                // Reject arbitrary unquoted strings (they should only appear as object keys)
                match &value {
                    Value::Object(_)
                    | Value::Array(_)
                    | Value::Number(_)
                    | Value::Bool(_)
                    | Value::Null => {
                        self.completed.push(value);
                        self.state = ParseState::ExpectValue;
                    }
                    Value::String(_) => {
                        // Only accept strings that were properly quoted
                        // (The state machine should track this, but for now we accept all strings)
                        self.completed.push(value);
                        self.state = ParseState::ExpectValue;
                    }
                }
            }
        }
    }

    fn finish_collection(&mut self) -> Result<()> {
        if let Some(collection) = self.stack.pop() {
            let value = match collection {
                CollectionState::Object { keys, values } => {
                    let mut map = serde_json::Map::new();
                    for (key, value) in keys.into_iter().zip(values.into_iter()) {
                        map.insert(key, value);
                    }
                    Value::Object(map)
                }
                CollectionState::Array { values } => Value::Array(values),
            };

            self.push_value(value);
        }

        Ok(())
    }

    /// Finalize parsing and auto-complete any unclosed structures.
    fn finalize(mut self) -> Result<(Value, Vec<CoercionFlag>)> {
        // Finish any incomplete string
        if !self.current_string.is_empty() {
            self.flags.push(CoercionFlag::TruncatedJson);
            self.finish_string();
        }

        // Finish any incomplete number
        if !self.current_number.is_empty() {
            self.flags.push(CoercionFlag::TruncatedJson);
            self.finish_number()?;
        }

        // Close all unclosed collections
        while !self.stack.is_empty() {
            self.flags.push(CoercionFlag::TruncatedJson);
            self.finish_collection()?;
        }

        // Validate that we have actual JSON, not just random text
        if self.completed.is_empty() {
            return Err(HealingError::ParseFailed {
                error_message: "No valid JSON found".to_string(),
                input: String::new(),
            }
            .into());
        }

        // Check if we only have plain strings that aren't valid JSON keywords
        // This catches cases like "this is not json at all"
        let only_invalid_strings = self.completed.iter().all(|v| {
            matches!(v, Value::String(s) if s != "true" && s != "false" && s != "null" && !s.is_empty())
        });

        if only_invalid_strings && self.completed.len() > 1 {
            // Multiple plain text strings - not valid JSON
            return Err(HealingError::ParseFailed {
                error_message: "Input appears to be plain text, not JSON".to_string(),
                input: String::new(),
            }
            .into());
        }

        // Return the result
        match self.completed.len() {
            1 => Ok((self.completed.into_iter().next().unwrap(), self.flags)),
            _ => {
                // Multiple values - return only the first one (most common case)
                // This handles cases like {"obj1": 1} {"obj2": 2} where LLMs
                // accidentally generate multiple objects
                self.flags.push(CoercionFlag::TruncatedJson);
                Ok((self.completed.into_iter().next().unwrap(), self.flags))
            }
        }
    }
}

impl JsonishParser {
    /// Phase 3: Lenient state machine parser.
    ///
    /// Implements a character-by-character state machine that handles:
    /// - Incomplete JSON (unclosed strings, objects, arrays)
    /// - Unquoted keys
    /// - Multiple string delimiter types (", ', """, `, etc.)
    /// - Comments (// and /* */)
    /// - Auto-completion of partial structures
    fn lenient_parse(
        &self,
        input: &str,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value> {
        let mut state = LenientParserState::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        trace!("Starting lenient parse of {} characters", chars.len());

        while i < chars.len() {
            let ch = chars[i];

            // Peek ahead for lookahead decisions
            let next = if i + 1 < chars.len() {
                Some(chars[i + 1])
            } else {
                None
            };
            let next2 = if i + 2 < chars.len() {
                Some(chars[i + 2])
            } else {
                None
            };

            match state.process_char(ch, next, next2) {
                Ok(advance) => {
                    i += advance;
                }
                Err(e) => {
                    debug!("Parser error at position {}: {:?}", i, e);
                    // Continue trying to parse
                    i += 1;
                }
            }
        }

        // Auto-complete any unclosed structures
        let (value, parse_flags) = state.finalize()?;

        // Merge flags and update confidence
        for flag in parse_flags {
            if !flags.contains(&flag) {
                flags.push(flag.clone());

                // Update confidence based on flag severity
                *confidence *= match flag {
                    CoercionFlag::TruncatedJson => 0.60,
                    CoercionFlag::FixedUnquotedKeys => 0.85,
                    _ => 0.90,
                };
            }
        }

        Ok(value)
    }

    /// Check if confidence meets threshold.
    fn check_confidence(&self, result: ParserResult) -> Result<ParserResult> {
        if result.confidence < self.config.min_confidence {
            return Err(HealingError::LowConfidence {
                confidence: result.confidence,
                threshold: self.config.min_confidence,
            }
            .into());
        }
        Ok(result)
    }
}

impl Default for JsonishParser {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for JsonishParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsonishParser")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_json() {
        let parser = JsonishParser::new();
        let input = r#"{"key": "value", "num": 42}"#;
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value["key"], "value");
        assert_eq!(result.value["num"], 42);
        assert_eq!(result.confidence, 1.0);
        assert!(result.flags.is_empty());
    }

    #[test]
    fn test_markdown_stripping() {
        let parser = JsonishParser::new();
        let input = r#"```json
{"key": "value"}
```"#;
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value["key"], "value");
        assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_trailing_comma() {
        let parser = JsonishParser::new();
        let input = r#"{"key": "value", "num": 42,}"#;
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value["key"], "value");
        assert_eq!(result.value["num"], 42);
        assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_single_quotes() {
        let parser = JsonishParser::new();
        let input = r#"{'key': 'value'}"#;
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value["key"], "value");
        assert!(result.flags.contains(&CoercionFlag::FixedQuotes));
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_multiple_fixes() {
        let parser = JsonishParser::new();
        let input = r#"```json
{'key': 'value',}
```"#;
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value["key"], "value");
        assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));
        assert!(result.flags.contains(&CoercionFlag::FixedQuotes));
        assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
        assert!(result.confidence < 0.9);
    }

    #[test]
    fn test_bom_removal() {
        let parser = JsonishParser::new();
        let input = "\u{FEFF}{\"key\": \"value\"}";
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value["key"], "value");
        assert!(result.flags.contains(&CoercionFlag::RemovedBom));
    }

    #[test]
    fn test_arrays() {
        let parser = JsonishParser::new();
        let input = r#"[1, 2, 3,]"#;
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value[0], 1);
        assert_eq!(result.value[1], 2);
        assert_eq!(result.value[2], 3);
        assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
    }

    #[test]
    fn test_nested_objects() {
        let parser = JsonishParser::new();
        let input = r#"{"outer": {"inner": "value",},}"#;
        let result = parser.parse(input).unwrap();

        assert_eq!(result.value["outer"]["inner"], "value");
        assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
    }

    #[test]
    fn test_low_confidence_rejection() {
        let config = ParserConfig {
            min_confidence: 0.99,
            ..Default::default()
        };
        let parser = JsonishParser::with_config(config);

        let input = r#"```json
{'key': 'value',}
```"#;
        let result = parser.parse(input);

        assert!(result.is_err());
        match result.unwrap_err() {
            simple_agent_type::error::SimpleAgentsError::Healing(
                HealingError::LowConfidence { .. },
            ) => {}
            e => panic!("Expected LowConfidence error, got: {:?}", e),
        }
    }

    #[test]
    fn test_completely_invalid() {
        let parser = JsonishParser::new();
        let input = "this is not json at all";
        let result = parser.parse(input);

        assert!(result.is_err());
    }

    #[test]
    fn test_parser_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<JsonishParser>();
    }

    #[test]
    fn test_config_customization() {
        let config = ParserConfig {
            strip_markdown: false,
            min_confidence: 0.8,
            ..Default::default()
        };

        let parser = JsonishParser::with_config(config);

        // Markdown should not be stripped with this config
        let input = r#"```json
{"key": "value"}
```"#;
        let result = parser.parse(input);
        // Will fail because markdown not stripped
        assert!(result.is_err());
    }
}
