//! Schema definition system for type validation and coercion.
//!
//! Provides a simple yet extensible schema definition system for describing expected types.
//! Future work will include derive macros for automatic schema generation from Rust types.

use serde::{Deserialize, Serialize};

/// Describes the expected structure and types for parsed JSON.
///
/// # Examples
///
/// ```
/// use simple_agents_healing::schema::Schema;
///
/// // Simple string schema
/// let name_schema = Schema::String;
///
/// // Integer with range
/// let age_schema = Schema::Int;
///
/// // Object with fields
/// let person_schema = Schema::object(vec![
///     ("name".into(), Schema::String, true),
///     ("age".into(), Schema::Int, true),
///     ("email".into(), Schema::String, false),  // optional
/// ]);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Schema {
    /// String type
    String,
    /// Signed integer (i64)
    Int,
    /// Unsigned integer (u64)
    UInt,
    /// Floating point number (f64)
    Float,
    /// Boolean
    Bool,
    /// Null value
    Null,
    /// Array of elements (homogeneous)
    Array(Box<Schema>),
    /// Object with named fields
    Object(ObjectSchema),
    /// Union of multiple possible types (tagged or untagged)
    Union(Vec<Schema>),
    /// Any valid JSON value (no validation)
    Any,
}

/// Schema for an object type with named fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectSchema {
    /// Field definitions
    pub fields: Vec<Field>,
    /// Whether to allow additional fields not in schema
    pub allow_additional_fields: bool,
}

/// Field definition in an object schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// Field name as it appears in the schema
    pub name: String,
    /// Expected type for this field
    pub schema: Schema,
    /// Whether this field is required (true) or optional (false)
    pub required: bool,
    /// Alternative names this field might have (aliases)
    pub aliases: Vec<String>,
    /// Default value if field is missing (JSON string representation)
    pub default: Option<serde_json::Value>,
    /// Description of the field (for documentation)
    pub description: Option<String>,
    /// Streaming annotation (controls emission timing)
    #[serde(default)]
    pub stream_annotation: StreamAnnotation,
}

/// Streaming annotation for field-level emission control.
///
/// Controls when a field value should be emitted during streaming parsing.
///
/// # Examples
///
/// ```
/// use simple_agents_healing::schema::{StreamAnnotation, Field, Schema};
///
/// // Emit as soon as available (default)
/// let normal_field = Field {
///     name: "name".to_string(),
///     schema: Schema::String,
///     required: true,
///     aliases: vec![],
///     default: None,
///     description: None,
///     stream_annotation: StreamAnnotation::Normal,
/// };
///
/// // Don't emit until non-null
/// let id_field = Field {
///     name: "id".to_string(),
///     schema: Schema::Int,
///     required: true,
///     aliases: vec![],
///     default: None,
///     description: None,
///     stream_annotation: StreamAnnotation::NotNull,
/// };
///
/// // Only emit when complete
/// let status_field = Field {
///     name: "status".to_string(),
///     schema: Schema::String,
///     required: true,
///     aliases: vec![],
///     default: None,
///     description: None,
///     stream_annotation: StreamAnnotation::Done,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StreamAnnotation {
    /// Emit field as soon as it's available (default)
    #[default]
    Normal,
    /// Don't emit until value is non-null (@@stream.not_null)
    NotNull,
    /// Only emit when the entire structure is complete (@@stream.done)
    Done,
}

impl Schema {
    /// Create a simple object schema with fields.
    ///
    /// # Arguments
    ///
    /// * `fields` - List of (name, schema, required) tuples
    ///
    /// # Examples
    ///
    /// ```
    /// use simple_agents_healing::schema::Schema;
    ///
    /// let schema = Schema::object(vec![
    ///     ("name".into(), Schema::String, true),
    ///     ("age".into(), Schema::Int, true),
    /// ]);
    /// ```
    pub fn object(fields: Vec<(String, Schema, bool)>) -> Self {
        Schema::Object(ObjectSchema {
            fields: fields
                .into_iter()
                .map(|(name, schema, required)| Field {
                    name,
                    schema,
                    required,
                    aliases: Vec::new(),
                    default: None,
                    description: None,
                    stream_annotation: StreamAnnotation::Normal,
                })
                .collect(),
            allow_additional_fields: false,
        })
    }

    /// Create an array schema with element type.
    ///
    /// # Examples
    ///
    /// ```
    /// use simple_agents_healing::schema::Schema;
    ///
    /// let string_array = Schema::array(Schema::String);
    /// let int_array = Schema::array(Schema::Int);
    /// ```
    pub fn array(element_schema: Schema) -> Self {
        Schema::Array(Box::new(element_schema))
    }

    /// Create a union schema (sum type).
    ///
    /// # Examples
    ///
    /// ```
    /// use simple_agents_healing::schema::Schema;
    ///
    /// // String or Int
    /// let schema = Schema::union(vec![Schema::String, Schema::Int]);
    /// ```
    pub fn union(variants: Vec<Schema>) -> Self {
        Schema::Union(variants)
    }

    /// Check if this schema represents a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            Schema::String
                | Schema::Int
                | Schema::UInt
                | Schema::Float
                | Schema::Bool
                | Schema::Null
        )
    }

    /// Check if this schema is nullable (includes Null in a union).
    pub fn is_nullable(&self) -> bool {
        match self {
            Schema::Null => true,
            Schema::Union(variants) => variants.iter().any(|v| v.is_nullable()),
            _ => false,
        }
    }

    /// Get a human-readable type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Schema::String => "string",
            Schema::Int => "int",
            Schema::UInt => "uint",
            Schema::Float => "float",
            Schema::Bool => "bool",
            Schema::Null => "null",
            Schema::Array(_) => "array",
            Schema::Object(_) => "object",
            Schema::Union(_) => "union",
            Schema::Any => "any",
        }
    }
}

impl Field {
    /// Create a new required field.
    pub fn required(name: impl Into<String>, schema: Schema) -> Self {
        Field {
            name: name.into(),
            schema,
            required: true,
            aliases: Vec::new(),
            default: None,
            description: None,
            stream_annotation: StreamAnnotation::Normal,
        }
    }

    /// Create a new optional field.
    pub fn optional(name: impl Into<String>, schema: Schema) -> Self {
        Field {
            name: name.into(),
            schema,
            required: false,
            aliases: Vec::new(),
            default: None,
            description: None,
            stream_annotation: StreamAnnotation::Normal,
        }
    }

    /// Add an alias to this field.
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    /// Add a default value for this field.
    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self
    }

    /// Add a description to this field.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the streaming annotation for this field.
    ///
    /// # Examples
    ///
    /// ```
    /// use simple_agents_healing::schema::{Field, Schema, StreamAnnotation};
    ///
    /// // Don't emit until non-null
    /// let id_field = Field::required("id", Schema::Int)
    ///     .with_stream_annotation(StreamAnnotation::NotNull);
    ///
    /// // Only emit when complete
    /// let status_field = Field::required("status", Schema::String)
    ///     .with_stream_annotation(StreamAnnotation::Done);
    /// ```
    pub fn with_stream_annotation(mut self, annotation: StreamAnnotation) -> Self {
        self.stream_annotation = annotation;
        self
    }
}

impl ObjectSchema {
    /// Create a new object schema.
    pub fn new(fields: Vec<Field>) -> Self {
        ObjectSchema {
            fields,
            allow_additional_fields: false,
        }
    }

    /// Allow additional fields beyond those defined in the schema.
    pub fn allow_additional(mut self) -> Self {
        self.allow_additional_fields = true;
        self
    }

    /// Find a field by name (exact match).
    pub fn get_field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get all field names (including aliases).
    pub fn all_field_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for field in &self.fields {
            names.push(field.name.clone());
            names.extend(field.aliases.clone());
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_schemas() {
        assert!(Schema::String.is_primitive());
        assert!(Schema::Int.is_primitive());
        assert!(!Schema::array(Schema::String).is_primitive());
    }

    #[test]
    fn test_object_schema_creation() {
        let schema = Schema::object(vec![
            ("name".into(), Schema::String, true),
            ("age".into(), Schema::Int, false),
        ]);

        if let Schema::Object(obj) = schema {
            assert_eq!(obj.fields.len(), 2);
            assert_eq!(obj.fields[0].name, "name");
            assert!(obj.fields[0].required);
            assert_eq!(obj.fields[1].name, "age");
            assert!(!obj.fields[1].required);
        } else {
            panic!("Expected Object schema");
        }
    }

    #[test]
    fn test_field_builder() {
        let field = Field::required("username", Schema::String)
            .with_alias("user_name")
            .with_description("The user's login name");

        assert_eq!(field.name, "username");
        assert!(field.required);
        assert_eq!(field.aliases, vec!["user_name"]);
        assert!(field.description.is_some());
    }

    #[test]
    fn test_nullable_schema() {
        assert!(Schema::Null.is_nullable());
        assert!(!Schema::String.is_nullable());
        assert!(Schema::union(vec![Schema::String, Schema::Null]).is_nullable());
    }

    #[test]
    fn test_type_names() {
        assert_eq!(Schema::String.type_name(), "string");
        assert_eq!(Schema::Int.type_name(), "int");
        assert_eq!(Schema::array(Schema::Bool).type_name(), "array");
        assert_eq!(
            Schema::object(vec![("x".into(), Schema::Float, true)]).type_name(),
            "object"
        );
    }
}
