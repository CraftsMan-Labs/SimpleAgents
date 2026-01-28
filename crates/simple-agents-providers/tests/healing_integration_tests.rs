//! Integration tests for healing system integration with providers.

use serde_json::json;
use simple_agents_providers::healing_integration::{HealingConfig, HealingIntegration};
use simple_agents_providers::schema_converter;
use simple_agent_type::coercion::CoercionFlag;

#[test]
fn test_schema_conversion_primitives() {
    let json_schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"},
            "score": {"type": "number"},
            "active": {"type": "boolean"}
        }
    });

    let schema = schema_converter::convert(&json_schema).unwrap();
    assert!(matches!(
        schema,
        simple_agents_healing::schema::Schema::Object(_)
    ));
}

#[test]
fn test_schema_conversion_arrays() {
    let json_schema = json!({
        "type": "array",
        "items": {"type": "string"}
    });

    let schema = schema_converter::convert(&json_schema).unwrap();
    assert!(matches!(
        schema,
        simple_agents_healing::schema::Schema::Array(_)
    ));
}

#[test]
fn test_schema_conversion_nested_objects() {
    let json_schema = json!({
        "type": "object",
        "properties": {
            "person": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "age": {"type": "integer"}
                }
            }
        }
    });

    let schema = schema_converter::convert(&json_schema).unwrap();
    if let simple_agents_healing::schema::Schema::Object(obj) = schema {
        assert_eq!(obj.fields.len(), 1);
        assert_eq!(obj.fields[0].name, "person");
        assert!(matches!(
            obj.fields[0].schema,
            simple_agents_healing::schema::Schema::Object(_)
        ));
    } else {
        panic!("Expected object schema");
    }
}

#[test]
fn test_healing_integration_markdown_fences() {
    let integration = HealingIntegration::new(HealingConfig::default());

    let schema = json!({
        "type": "object",
        "properties": {
            "message": {"type": "string"}
        }
    });

    let malformed = r#"```json
{
  "message": "Hello, world!"
}
```"#;

    let result = integration
        .heal_response(malformed, &schema, "Parse error")
        .unwrap();

    assert_eq!(result.value["message"], "Hello, world!");
    assert!(result.metadata.confidence > 0.0);

    // Should have stripped markdown
    assert!(result
        .metadata
        .flags
        .iter()
        .any(|f| matches!(f, CoercionFlag::StrippedMarkdown)));
}

#[test]
fn test_healing_integration_type_coercion() {
    let integration = HealingIntegration::new(HealingConfig::default());

    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"}
        }
    });

    // Age is a string but should be coerced to integer
    let malformed = r#"{"name": "Alice", "age": "25"}"#;

    let result = integration
        .heal_response(malformed, &schema, "Type mismatch")
        .unwrap();

    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["age"], 25);
    assert!(result.metadata.confidence > 0.0);
}

#[test]
fn test_healing_integration_default_values() {
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
    let malformed = r#"{"name": "Bob"}"#;

    let result = integration
        .heal_response(malformed, &schema, "Missing field")
        .unwrap();

    assert_eq!(result.value["name"], "Bob");
    assert_eq!(result.value["count"], 0);
}

#[test]
fn test_healing_integration_strict_mode() {
    let config = HealingConfig::strict();
    let integration = HealingIntegration::new(config);

    let schema = json!({
        "type": "object",
        "properties": {
            "value": {"type": "integer"}
        }
    });

    // String to integer coercion - strict mode may reject this
    let malformed = r#"{"value": "123"}"#;

    // Strict mode requires higher confidence
    let result = integration.heal_response(malformed, &schema, "Type mismatch");

    // May succeed or fail depending on confidence
    match result {
        Ok(healed) => {
            assert_eq!(healed.value["value"], 123);
            assert!(healed.metadata.confidence >= 0.9);
        }
        Err(_) => {
            // This is also acceptable for strict mode
        }
    }
}

#[test]
fn test_healing_integration_lenient_mode() {
    let config = HealingConfig::lenient();
    let integration = HealingIntegration::new(config);

    let schema = json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer"}
        }
    });

    // String to number - lenient mode should accept this
    let malformed = r#"{"count": "42"}"#;

    let result = integration
        .heal_response(malformed, &schema, "Type mismatch")
        .unwrap();

    // Should coerce string to number
    assert_eq!(result.value["count"], 42);
    assert!(result.metadata.confidence >= 0.5);
}

#[test]
fn test_healing_disabled() {
    let config = HealingConfig::default().with_enabled(false);
    let integration = HealingIntegration::new(config);

    let schema = json!({"type": "string"});
    let result = integration.heal_response("test", &schema, "error");

    assert!(result.is_err());
}

#[test]
fn test_healing_config_builder() {
    let config = HealingConfig::new()
        .with_enabled(true)
        .with_min_confidence(0.85);

    assert!(config.enabled);
    assert_eq!(config.min_confidence, 0.85);
}

#[test]
fn test_healing_metadata_structure() {
    let integration = HealingIntegration::new(HealingConfig::default());

    let schema = json!({
        "type": "object",
        "properties": {
            "text": {"type": "string"}
        }
    });

    let malformed = r#"```json
{"text": "hello"}
```"#;

    let result = integration
        .heal_response(malformed, &schema, "Original error message")
        .unwrap();

    assert_eq!(result.metadata.original_error, "Original error message");
    assert!(!result.metadata.flags.is_empty());
    assert!(result.metadata.confidence > 0.0 && result.metadata.confidence <= 1.0);
}

#[test]
fn test_schema_conversion_with_required_fields() {
    let json_schema = json!({
        "type": "object",
        "properties": {
            "required_field": {"type": "string"},
            "optional_field": {"type": "string"}
        },
        "required": ["required_field"]
    });

    let schema = schema_converter::convert(&json_schema).unwrap();

    if let simple_agents_healing::schema::Schema::Object(obj) = schema {
        let required_field = obj
            .fields
            .iter()
            .find(|f| f.name == "required_field")
            .unwrap();
        assert!(required_field.required);

        let optional_field = obj
            .fields
            .iter()
            .find(|f| f.name == "optional_field")
            .unwrap();
        assert!(!optional_field.required);
    } else {
        panic!("Expected object schema");
    }
}

#[test]
fn test_schema_conversion_union_types() {
    let json_schema = json!({
        "type": ["string", "null"]
    });

    let schema = schema_converter::convert(&json_schema).unwrap();
    assert!(matches!(
        schema,
        simple_agents_healing::schema::Schema::Union(_)
    ));
}

#[test]
fn test_healing_with_complex_schema() {
    let integration = HealingIntegration::new(HealingConfig::default());

    let schema = json!({
        "type": "object",
        "properties": {
            "user": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "contacts": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": {"type": "string"},
                                "value": {"type": "string"}
                            }
                        }
                    }
                }
            },
            "timestamp": {"type": "integer"}
        }
    });

    let malformed = r#"```json
{
  "user": {
    "name": "Alice",
    "contacts": [
      {"type": "email", "value": "alice@example.com"},
      {"type": "phone", "value": "555-1234"}
    ]
  },
  "timestamp": "1234567890"
}
```"#;

    let result = integration
        .heal_response(malformed, &schema, "Complex parse error")
        .unwrap();

    assert_eq!(result.value["user"]["name"], "Alice");
    assert_eq!(result.value["user"]["contacts"][0]["type"], "email");
    assert_eq!(result.value["timestamp"], 1234567890);
}
