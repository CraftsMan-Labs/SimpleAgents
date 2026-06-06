//! Healing helpers and response wrappers.

use simple_agent_type::coercion::CoercionResult;
use simple_agent_type::error::HealingError;
use simple_agent_type::prelude::CompletionResponse;
use simple_agents_healing::coercion::CoercionConfig;
use simple_agents_healing::parser::ParserConfig;
use simple_agents_healing::parser::ParserResult;

/// Healing settings for JSON parsing and coercion.
#[derive(Debug, Clone)]
pub struct HealingSettings {
    /// Enable healing APIs.
    pub enabled: bool,
    /// Parser configuration for JSON-ish parsing.
    pub parser_config: ParserConfig,
    /// Coercion configuration for schema alignment.
    pub coercion_config: CoercionConfig,
}

impl Default for HealingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            parser_config: ParserConfig::default(),
            coercion_config: CoercionConfig::default(),
        }
    }
}

impl HealingSettings {
    /// Create a new settings struct with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable healing APIs.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Override parser configuration.
    pub fn with_parser_config(mut self, config: ParserConfig) -> Self {
        self.parser_config = config;
        self
    }

    /// Override coercion configuration.
    pub fn with_coercion_config(mut self, config: CoercionConfig) -> Self {
        self.coercion_config = config;
        self
    }
}

/// Captures a healing/parse failure alongside the raw LLM text that caused it.
#[derive(Debug, Clone)]
pub struct HealingFailure {
    /// The healing error that occurred.
    pub error: HealingError,
    /// The raw text that failed to parse or coerce.
    pub raw_text: String,
}

impl std::fmt::Display for HealingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

/// JSON healing response wrapper.
///
/// The `response` field is always populated, even when parsing fails.
/// Check `parsed` to determine whether JSON extraction succeeded.
pub struct HealedJsonResponse {
    /// Original completion response (always present).
    pub response: CompletionResponse,
    /// Parsed JSON value and healing metadata, or the failure details.
    pub parsed: Result<ParserResult, HealingFailure>,
}

impl HealedJsonResponse {
    /// Returns the parsed JSON value if parsing succeeded.
    pub fn parsed_value(&self) -> Option<&serde_json::Value> {
        self.parsed.as_ref().ok().map(|r| &r.value)
    }

    /// Returns `true` if JSON parsing failed.
    pub fn is_parse_failed(&self) -> bool {
        self.parsed.is_err()
    }

    /// Returns the raw LLM content from the response.
    pub fn raw_content(&self) -> Option<&str> {
        self.response.content()
    }
}

/// Schema-aligned healing response wrapper.
///
/// The `response` field is always populated, even when parsing or coercion fails.
/// `coerced` is `None` when `parsed` failed (cannot coerce without a parse result),
/// `Some(Err(...))` when parsing succeeded but coercion failed,
/// and `Some(Ok(...))` when both succeeded.
pub struct HealedSchemaResponse {
    /// Original completion response (always present).
    pub response: CompletionResponse,
    /// Parsed JSON value and healing metadata, or the failure details.
    pub parsed: Result<ParserResult, HealingFailure>,
    /// Schema-coerced value and healing metadata.
    /// `None` when parsing failed; `Some(Err)` when coercion failed.
    pub coerced: Option<Result<CoercionResult<serde_json::Value>, HealingFailure>>,
}

impl HealedSchemaResponse {
    /// Returns the coerced JSON value if both parsing and coercion succeeded.
    pub fn coerced_value(&self) -> Option<&serde_json::Value> {
        self.coerced.as_ref()?.as_ref().ok().map(|r| &r.value)
    }

    /// Returns `true` if both parsing and coercion succeeded.
    pub fn is_fully_resolved(&self) -> bool {
        self.parsed.is_ok() && self.coerced.as_ref().is_some_and(|c| c.is_ok())
    }

    /// Returns the raw LLM content from the response.
    pub fn raw_content(&self) -> Option<&str> {
        self.response.content()
    }
}
