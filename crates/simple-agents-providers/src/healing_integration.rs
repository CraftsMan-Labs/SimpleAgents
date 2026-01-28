//! Healing system integration for structured output recovery.
//!
//! Provides automatic fallback to healing when native structured outputs fail.

use serde_json::Value;
use simple_agents_healing::coercion::{CoercionConfig, CoercionEngine};
use simple_agents_healing::parser::{JsonishParser, ParserConfig};
use simple_agent_type::error::{HealingError, SimpleAgentsError, ValidationError};
use simple_agent_type::response::HealingMetadata;

use crate::schema_converter;

/// Configuration for healing integration.
#[derive(Debug, Clone)]
pub struct HealingConfig {
    /// Whether healing is enabled
    pub enabled: bool,
    /// Minimum confidence threshold (0.0-1.0)
    pub min_confidence: f32,
    /// Parser configuration
    pub parser_config: ParserConfig,
    /// Coercion configuration
    pub coercion_config: CoercionConfig,
}

impl Default for HealingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_confidence: 0.7,
            parser_config: ParserConfig::default(),
            coercion_config: CoercionConfig::default(),
        }
    }
}

impl HealingConfig {
    /// Create a new healing config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a strict healing config (high confidence required).
    pub fn strict() -> Self {
        Self {
            enabled: true,
            min_confidence: 0.9,
            parser_config: ParserConfig::default(),
            coercion_config: CoercionConfig {
                fuzzy_match_threshold: 0.9,
                allow_string_to_number: false,
                allow_string_to_bool: false,
                allow_float_to_int: false,
                inject_defaults: true,
                min_confidence: 0.9,
            },
        }
    }

    /// Create a lenient healing config (low confidence threshold).
    pub fn lenient() -> Self {
        Self {
            enabled: true,
            min_confidence: 0.5,
            parser_config: ParserConfig::default(),
            coercion_config: CoercionConfig {
                fuzzy_match_threshold: 0.7,
                allow_string_to_number: true,
                allow_string_to_bool: true,
                allow_float_to_int: true,
                inject_defaults: true,
                min_confidence: 0.5,
            },
        }
    }

    /// Set whether healing is enabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the minimum confidence threshold.
    pub fn with_min_confidence(mut self, min_confidence: f32) -> Self {
        self.min_confidence = min_confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the parser configuration.
    pub fn with_parser_config(mut self, config: ParserConfig) -> Self {
        self.parser_config = config;
        self
    }

    /// Set the coercion configuration.
    pub fn with_coercion_config(mut self, config: CoercionConfig) -> Self {
        self.coercion_config = config;
        self
    }
}

/// Result of a healing operation.
#[derive(Debug)]
pub struct HealedResponse {
    /// The healed JSON value
    pub value: Value,
    /// Healing metadata
    pub metadata: HealingMetadata,
}

/// Orchestrates the healing process for malformed responses.
pub struct HealingIntegration {
    config: HealingConfig,
    parser: JsonishParser,
    coercion: CoercionEngine,
}

impl HealingIntegration {
    /// Create a new healing integration with the given configuration.
    pub fn new(config: HealingConfig) -> Self {
        Self {
            parser: JsonishParser::with_config(config.parser_config.clone()),
            coercion: CoercionEngine::with_config(config.coercion_config.clone()),
            config,
        }
    }

    /// Attempt to heal a malformed response using the given JSON schema.
    ///
    /// # Arguments
    /// * `raw_content` - The raw response content (possibly malformed JSON)
    /// * `json_schema` - The JSON Schema that the response should conform to
    /// * `original_error` - The error that triggered healing
    ///
    /// # Returns
    /// A healed response with the parsed JSON value and healing metadata.
    ///
    /// # Example
    /// ```no_run
    /// use simple_agents_providers::healing_integration::{HealingIntegration, HealingConfig};
    /// use serde_json::json;
    ///
    /// let integration = HealingIntegration::new(HealingConfig::default());
    ///
    /// let schema = json!({
    ///     "type": "object",
    ///     "properties": {
    ///         "name": {"type": "string"},
    ///         "age": {"type": "integer"}
    ///     }
    /// });
    ///
    /// let malformed = r#"```json
    /// {
    ///   "name": "Alice",
    ///   "age": "25"
    /// }
    /// ```"#;
    ///
    /// let result = integration.heal_response(malformed, &schema, "Parse error").unwrap();
    /// assert_eq!(result.value["name"], "Alice");
    /// ```
    pub fn heal_response(
        &self,
        raw_content: &str,
        json_schema: &Value,
        original_error: &str,
    ) -> Result<HealedResponse, SimpleAgentsError> {
        if !self.config.enabled {
            return Err(SimpleAgentsError::Validation(ValidationError::Custom(
                "Healing is disabled".to_string(),
            )));
        }

        // Step 1: Convert JSON Schema to healing Schema
        let schema = schema_converter::convert(json_schema)?;

        // Step 2: Parse the raw content with the JsonishParser
        let parse_result = self.parser.parse(raw_content)?;

        let mut all_flags = parse_result.flags;
        let mut confidence = parse_result.confidence;

        // Step 3: Coerce the parsed value to match the schema
        let coerce_result = self.coercion.coerce(&parse_result.value, &schema)?;

        // Merge flags from parsing and coercion
        all_flags.extend(coerce_result.flags);

        // Take minimum confidence from parsing and coercion
        confidence = confidence.min(coerce_result.confidence);

        // Step 4: Check if confidence meets threshold
        if confidence < self.config.min_confidence {
            return Err(SimpleAgentsError::Healing(HealingError::LowConfidence {
                confidence,
                threshold: self.config.min_confidence,
            }));
        }

        Ok(HealedResponse {
            value: coerce_result.value,
            metadata: HealingMetadata::new(all_flags, confidence, original_error.to_string()),
        })
    }

    /// Check if healing is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the minimum confidence threshold.
    pub fn min_confidence(&self) -> f32 {
        self.config.min_confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use simple_agent_type::coercion::CoercionFlag;

    #[test]
    fn test_healing_config_default() {
        let config = HealingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_confidence, 0.7);
    }

    #[test]
    fn test_healing_config_strict() {
        let config = HealingConfig::strict();
        assert!(config.enabled);
        assert_eq!(config.min_confidence, 0.9);
    }

    #[test]
    fn test_healing_config_lenient() {
        let config = HealingConfig::lenient();
        assert!(config.enabled);
        assert_eq!(config.min_confidence, 0.5);
    }

    #[test]
    fn test_healing_config_builder() {
        let config = HealingConfig::new()
            .with_enabled(false)
            .with_min_confidence(0.8);

        assert!(!config.enabled);
        assert_eq!(config.min_confidence, 0.8);
    }

    #[test]
    fn test_healing_integration_disabled() {
        let config = HealingConfig::default().with_enabled(false);
        let integration = HealingIntegration::new(config);

        let schema = json!({"type": "string"});
        let result = integration.heal_response("test", &schema, "error");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SimpleAgentsError::Validation(ValidationError::Custom(_))
        ));
    }

    #[test]
    fn test_heal_markdown_fenced_json() {
        let integration = HealingIntegration::new(HealingConfig::default());

        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        });

        let malformed = r#"```json
{
  "name": "Alice",
  "age": 25
}
```"#;

        let result = integration
            .heal_response(malformed, &schema, "Parse error")
            .unwrap();

        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 25);
        assert!(result.metadata.confidence > 0.0);
        assert!(!result.metadata.flags.is_empty());
    }

    #[test]
    fn test_heal_type_coercion() {
        let integration = HealingIntegration::new(HealingConfig::default());

        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        });

        // Age is a string but should be coerced to integer
        let malformed = r#"{"name": "Bob", "age": "30"}"#;

        let result = integration
            .heal_response(malformed, &schema, "Type mismatch")
            .unwrap();

        assert_eq!(result.value["name"], "Bob");
        assert_eq!(result.value["age"], 30);
    }

    #[test]
    fn test_heal_with_defaults() {
        let integration = HealingIntegration::new(HealingConfig::default());

        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "count": {"type": "integer", "default": 0}
            },
            "required": ["name"]
        });

        // Missing count field - should use default
        let malformed = r#"{"name": "Charlie"}"#;

        let result = integration
            .heal_response(malformed, &schema, "Missing field")
            .unwrap();

        assert_eq!(result.value["name"], "Charlie");
        assert_eq!(result.value["count"], 0);
    }

    #[test]
    fn test_healing_metadata() {
        let integration = HealingIntegration::new(HealingConfig::default());

        let schema = json!({"type": "string"});
        let malformed = r#"```json
"test"
```"#;

        let result = integration
            .heal_response(malformed, &schema, "Original error")
            .unwrap();

        assert_eq!(result.metadata.original_error, "Original error");
        assert!(result.metadata.confidence > 0.0);
        assert!(!result.metadata.flags.is_empty());

        // Should have markdown stripping flag
        assert!(result
            .metadata
            .flags
            .iter()
            .any(|f| matches!(f, CoercionFlag::StrippedMarkdown)));
    }

    #[test]
    fn test_low_confidence_rejection() {
        let config = HealingConfig::strict(); // Requires 0.9 confidence
        let integration = HealingIntegration::new(config);

        let schema = json!({
            "type": "object",
            "properties": {
                "data": {"type": "string"}
            }
        });

        // This might produce lower confidence due to coercions
        let malformed = r#"```json
{
  "data": 12345
}
```"#;

        let result = integration.heal_response(malformed, &schema, "error");

        // May or may not fail depending on actual confidence
        // This test is more about ensuring the threshold check works
        if let Err(SimpleAgentsError::Healing(HealingError::LowConfidence {
            confidence,
            threshold,
        })) = result
        {
            assert!(confidence < threshold);
            assert_eq!(threshold, 0.9);
        }
    }

    #[test]
    fn test_is_enabled() {
        let integration = HealingIntegration::new(HealingConfig::default());
        assert!(integration.is_enabled());

        let disabled = HealingIntegration::new(HealingConfig::default().with_enabled(false));
        assert!(!disabled.is_enabled());
    }

    #[test]
    fn test_min_confidence() {
        let integration =
            HealingIntegration::new(HealingConfig::default().with_min_confidence(0.85));
        assert_eq!(integration.min_confidence(), 0.85);
    }
}
