//! JSON Schema to Healing Schema converter.
//!
//! Converts standard JSON Schema format to the internal healing Schema format
//! used by the healing system for parsing and coercion.

use serde_json::Value;
use simple_agents_healing::schema::{Field, ObjectSchema, Schema, StreamAnnotation};
use simple_agent_type::error::{SimpleAgentsError, ValidationError};

/// Convert a JSON Schema to a healing Schema.
///
/// Supports:
/// - Primitive types: string, integer, number, boolean, null
/// - Arrays with item schemas
/// - Objects with properties and required fields
/// - Nested schemas (recursive)
/// - Default values
/// - Descriptions
///
/// # Example
/// ```
/// use serde_json::json;
/// use simple_agents_providers::schema_converter::convert;
///
/// let json_schema = json!({
///     "type": "object",
///     "properties": {
///         "name": {"type": "string"},
///         "age": {"type": "integer"}
///     },
///     "required": ["name"]
/// });
///
/// let schema = convert(&json_schema).unwrap();
/// ```
pub fn convert(json_schema: &Value) -> Result<Schema, SimpleAgentsError> {
    // Handle $ref if present (simple case - just dereference)
    if let Some(ref_str) = json_schema.get("$ref").and_then(|v| v.as_str()) {
        return Err(SimpleAgentsError::Validation(ValidationError::Custom(
            format!("JSON Schema $ref not supported yet: {}", ref_str),
        )));
    }

    // Get the type field
    let type_value = json_schema.get("type");

    match type_value {
        Some(Value::String(type_str)) => convert_typed_schema(json_schema, type_str),
        Some(Value::Array(types)) => {
            // Union type - multiple possible types
            let schemas: Result<Vec<Schema>, SimpleAgentsError> = types
                .iter()
                .map(|t| {
                    if let Some(type_str) = t.as_str() {
                        let mut single_type = json_schema.clone();
                        single_type["type"] = Value::String(type_str.to_string());
                        convert(&single_type)
                    } else {
                        Err(SimpleAgentsError::Validation(ValidationError::Custom(
                            "Invalid type in array".to_string(),
                        )))
                    }
                })
                .collect();

            Ok(Schema::Union(schemas?))
        }
        None => {
            // No type specified - treat as Any
            Ok(Schema::Any)
        }
        _ => Err(SimpleAgentsError::Validation(ValidationError::Custom(
            "Invalid 'type' field in JSON Schema".to_string(),
        ))),
    }
}

/// Convert a typed JSON Schema to a healing Schema.
fn convert_typed_schema(json_schema: &Value, type_str: &str) -> Result<Schema, SimpleAgentsError> {
    match type_str {
        "string" => Ok(Schema::String),
        "integer" => Ok(Schema::Int),
        "number" => Ok(Schema::Float),
        "boolean" => Ok(Schema::Bool),
        "null" => Ok(Schema::Null),
        "array" => convert_array_schema(json_schema),
        "object" => convert_object_schema(json_schema),
        _ => Err(SimpleAgentsError::Validation(ValidationError::Custom(
            format!("Unknown JSON Schema type: {}", type_str),
        ))),
    }
}

/// Convert an array JSON Schema to a healing Schema.
fn convert_array_schema(json_schema: &Value) -> Result<Schema, SimpleAgentsError> {
    // Get the items schema
    let items = json_schema.get("items");

    match items {
        Some(item_schema) => {
            let inner_schema = convert(item_schema)?;
            Ok(Schema::Array(Box::new(inner_schema)))
        }
        None => {
            // No items schema - array of any
            Ok(Schema::Array(Box::new(Schema::Any)))
        }
    }
}

/// Convert an object JSON Schema to a healing Schema.
fn convert_object_schema(json_schema: &Value) -> Result<Schema, SimpleAgentsError> {
    let properties = json_schema.get("properties");
    let required = json_schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let allow_additional_fields = json_schema
        .get("additionalProperties")
        .map(|v| {
            // If it's a boolean, use that value
            // If it's an object, allow additional fields
            v.as_bool().unwrap_or(true)
        })
        .unwrap_or(true); // Default to allowing additional fields

    let fields = match properties {
        Some(Value::Object(props)) => {
            let mut fields = Vec::new();

            for (name, prop_schema) in props {
                let schema = convert(prop_schema)?;
                let is_required = required.contains(name);
                let description = prop_schema
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let default = prop_schema.get("default").cloned();

                fields.push(Field {
                    name: name.clone(),
                    schema,
                    required: is_required,
                    aliases: Vec::new(), // JSON Schema doesn't have aliases
                    default,
                    description,
                    stream_annotation: StreamAnnotation::default(),
                });
            }

            fields
        }
        None => Vec::new(), // Object with no properties
        _ => {
            return Err(SimpleAgentsError::Validation(ValidationError::Custom(
                "Invalid 'properties' field in object schema".to_string(),
            )))
        }
    };

    Ok(Schema::Object(ObjectSchema {
        fields,
        allow_additional_fields,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_convert_string() {
        let schema = json!({"type": "string"});
        let result = convert(&schema).unwrap();
        assert_eq!(result, Schema::String);
    }

    #[test]
    fn test_convert_integer() {
        let schema = json!({"type": "integer"});
        let result = convert(&schema).unwrap();
        assert_eq!(result, Schema::Int);
    }

    #[test]
    fn test_convert_number() {
        let schema = json!({"type": "number"});
        let result = convert(&schema).unwrap();
        assert_eq!(result, Schema::Float);
    }

    #[test]
    fn test_convert_boolean() {
        let schema = json!({"type": "boolean"});
        let result = convert(&schema).unwrap();
        assert_eq!(result, Schema::Bool);
    }

    #[test]
    fn test_convert_null() {
        let schema = json!({"type": "null"});
        let result = convert(&schema).unwrap();
        assert_eq!(result, Schema::Null);
    }

    #[test]
    fn test_convert_array() {
        let schema = json!({
            "type": "array",
            "items": {"type": "string"}
        });
        let result = convert(&schema).unwrap();
        assert!(matches!(result, Schema::Array(_)));
        if let Schema::Array(inner) = result {
            assert_eq!(*inner, Schema::String);
        }
    }

    #[test]
    fn test_convert_array_no_items() {
        let schema = json!({"type": "array"});
        let result = convert(&schema).unwrap();
        assert!(matches!(result, Schema::Array(_)));
        if let Schema::Array(inner) = result {
            assert_eq!(*inner, Schema::Any);
        }
    }

    #[test]
    fn test_convert_object_simple() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            },
            "required": ["name"]
        });
        let result = convert(&schema).unwrap();
        assert!(matches!(result, Schema::Object(_)));

        if let Schema::Object(obj) = result {
            assert_eq!(obj.fields.len(), 2);

            let name_field = obj.fields.iter().find(|f| f.name == "name").unwrap();
            assert_eq!(name_field.schema, Schema::String);
            assert!(name_field.required);

            let age_field = obj.fields.iter().find(|f| f.name == "age").unwrap();
            assert_eq!(age_field.schema, Schema::Int);
            assert!(!age_field.required);
        }
    }

    #[test]
    fn test_convert_object_with_defaults() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "default": "John"
                },
                "age": {
                    "type": "integer",
                    "default": 25
                }
            }
        });
        let result = convert(&schema).unwrap();

        if let Schema::Object(obj) = result {
            let name_field = obj.fields.iter().find(|f| f.name == "name").unwrap();
            assert_eq!(name_field.default, Some(json!("John")));

            let age_field = obj.fields.iter().find(|f| f.name == "age").unwrap();
            assert_eq!(age_field.default, Some(json!(25)));
        }
    }

    #[test]
    fn test_convert_object_with_descriptions() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The person's name"
                }
            }
        });
        let result = convert(&schema).unwrap();

        if let Schema::Object(obj) = result {
            let name_field = obj.fields.iter().find(|f| f.name == "name").unwrap();
            assert_eq!(
                name_field.description,
                Some("The person's name".to_string())
            );
        }
    }

    #[test]
    fn test_convert_nested_object() {
        let schema = json!({
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
        let result = convert(&schema).unwrap();

        if let Schema::Object(obj) = result {
            let person_field = obj.fields.iter().find(|f| f.name == "person").unwrap();
            assert!(matches!(person_field.schema, Schema::Object(_)));

            if let Schema::Object(inner_obj) = &person_field.schema {
                assert_eq!(inner_obj.fields.len(), 2);
            }
        }
    }

    #[test]
    fn test_convert_union_type() {
        let schema = json!({
            "type": ["string", "null"]
        });
        let result = convert(&schema).unwrap();
        assert!(matches!(result, Schema::Union(_)));

        if let Schema::Union(types) = result {
            assert_eq!(types.len(), 2);
            assert!(types.contains(&Schema::String));
            assert!(types.contains(&Schema::Null));
        }
    }

    #[test]
    fn test_convert_no_type() {
        let schema = json!({});
        let result = convert(&schema).unwrap();
        assert_eq!(result, Schema::Any);
    }

    #[test]
    fn test_convert_additional_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "additionalProperties": false
        });
        let result = convert(&schema).unwrap();

        if let Schema::Object(obj) = result {
            assert!(!obj.allow_additional_fields);
        }
    }

    #[test]
    fn test_convert_nested_array() {
        let schema = json!({
            "type": "array",
            "items": {
                "type": "array",
                "items": {"type": "integer"}
            }
        });
        let result = convert(&schema).unwrap();

        if let Schema::Array(outer) = result {
            if let Schema::Array(inner) = *outer {
                assert_eq!(*inner, Schema::Int);
            } else {
                panic!("Expected nested array");
            }
        }
    }
}
