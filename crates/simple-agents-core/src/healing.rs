//! Healing helpers and response wrappers.

use simple_agents_healing::coercion::CoercionConfig;
use simple_agents_healing::parser::ParserConfig;
use simple_agents_healing::parser::ParserResult;
use simple_agent_type::coercion::CoercionResult;
use simple_agent_type::prelude::CompletionResponse;

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

/// JSON healing response wrapper.
pub struct HealedJsonResponse {
    /// Original completion response.
    pub response: CompletionResponse,
    /// Parsed JSON value and healing metadata.
    pub parsed: ParserResult,
}

/// Schema-aligned healing response wrapper.
pub struct HealedSchemaResponse {
    /// Original completion response.
    pub response: CompletionResponse,
    /// Parsed JSON value and healing metadata.
    pub parsed: ParserResult,
    /// Schema-coerced value and healing metadata.
    pub coerced: CoercionResult<serde_json::Value>,
}
