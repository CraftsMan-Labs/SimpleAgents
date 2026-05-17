//! Type coercion engine for schema-aligned parsing.
//!
//! Provides sophisticated type coercion with confidence scoring and transformation tracking.
//! This module takes parsed JSON values and coerces them to match expected schemas.
//!
//! # Architecture
//!
//! ```text
//! Parsed JSON Value
//!     ↓
//! ┌─────────────────────┐
//! │  Type Coercion      │ ← String→Int, Float→Int, etc.
//! └─────────────────────┘
//!     ↓
//! ┌─────────────────────┐
//! │  Field Matching     │ ← Fuzzy match, snake_case ↔ camelCase
//! └─────────────────────┘
//!     ↓
//! ┌─────────────────────┐
//! │  Union Resolution   │ ← Pick best matching variant
//! └─────────────────────┘
//!     ↓
//! ┌─────────────────────┐
//! │  Default Injection  │ ← Fill missing optional fields
//! └─────────────────────┘
//!     ↓
//! CoercionResult<Value>
//! ```

use crate::schema::{Field, ObjectSchema, Schema};
use crate::string_utils::{jaro_winkler, to_camel_case, to_snake_case};
use serde_json::Value;
use simple_agent_type::coercion::{CoercionFlag, CoercionResult};
use simple_agent_type::error::HealingError;

/// Configuration for the coercion engine.
#[derive(Debug, Clone)]
pub struct CoercionConfig {
    /// Minimum Jaro-Winkler similarity score for fuzzy field matching (0.0-1.0)
    pub fuzzy_match_threshold: f64,
    /// Whether to allow string to number coercion
    pub allow_string_to_number: bool,
    /// Whether to allow string to boolean coercion
    pub allow_string_to_bool: bool,
    /// Whether to allow float to int coercion (truncation)
    pub allow_float_to_int: bool,
    /// Whether to inject default values for missing fields
    pub inject_defaults: bool,
    /// Minimum confidence threshold (coercion fails if below this)
    pub min_confidence: f32,
    /// When true, truncated JSON with missing required fields returns an error
    /// instead of silently injecting null. Critical for clinical/financial data.
    pub strict_required: bool,
}

impl Default for CoercionConfig {
    fn default() -> Self {
        Self {
            fuzzy_match_threshold: 0.8,
            allow_string_to_number: true,
            allow_string_to_bool: true,
            allow_float_to_int: true,
            inject_defaults: true,
            min_confidence: 0.0,
            strict_required: false,
        }
    }
}

impl CoercionConfig {
    /// Create a strict configuration suitable for clinical/financial data.
    ///
    /// Strict mode rejects truncated JSON with missing required fields rather than
    /// silently injecting null values.
    pub fn strict() -> Self {
        Self {
            strict_required: true,
            ..Default::default()
        }
    }

    /// Create a lenient configuration that allows null injection for missing fields.
    pub fn lenient() -> Self {
        Self {
            strict_required: false,
            ..Default::default()
        }
    }
}

/// The coercion engine that performs schema-aligned type transformations.
pub struct CoercionEngine {
    config: CoercionConfig,
}

impl CoercionEngine {
    /// Create a new coercion engine with default configuration.
    pub fn new() -> Self {
        Self {
            config: CoercionConfig::default(),
        }
    }

    /// Create a coercion engine with custom configuration.
    pub fn with_config(config: CoercionConfig) -> Self {
        Self { config }
    }

    /// Coerce a JSON value to match the given schema.
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to coerce
    /// * `schema` - The target schema to match
    ///
    /// # Returns
    ///
    /// A `CoercionResult` containing the coerced value, transformation flags, and confidence score.
    ///
    /// # Examples
    ///
    /// ```
    /// use simple_agents_healing::coercion::CoercionEngine;
    /// use simple_agents_healing::schema::Schema;
    /// use serde_json::json;
    ///
    /// let engine = CoercionEngine::new();
    /// let value = json!("42");
    /// let schema = Schema::Int;
    ///
    /// let result = engine.coerce(&value, &schema).unwrap();
    /// assert_eq!(result.value, json!(42));
    /// assert!(result.confidence < 1.0);  // String→Int coercion reduces confidence
    /// ```
    pub fn coerce(
        &self,
        value: &Value,
        schema: &Schema,
    ) -> Result<CoercionResult<Value>, HealingError> {
        let mut flags = Vec::new();
        let mut confidence = 1.0;

        let coerced_value = self.coerce_recursive(value, schema, &mut flags, &mut confidence)?;

        if confidence < self.config.min_confidence {
            return Err(HealingError::LowConfidence {
                confidence,
                threshold: self.config.min_confidence,
            });
        }

        Ok(CoercionResult {
            value: coerced_value,
            flags,
            confidence,
        })
    }

    /// Recursively coerce a value to match a schema.
    fn coerce_recursive(
        &self,
        value: &Value,
        schema: &Schema,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        match (value, schema) {
            // Exact type matches (no coercion needed)
            (Value::String(_), Schema::String) => Ok(value.clone()),
            (Value::Number(n), Schema::Int) if n.is_i64() => Ok(value.clone()),
            (Value::Number(n), Schema::UInt) if n.is_u64() => Ok(value.clone()),
            (Value::Number(n), Schema::Float) if n.is_f64() => Ok(value.clone()),
            (Value::Bool(_), Schema::Bool) => Ok(value.clone()),
            (Value::Null, Schema::Null) => Ok(value.clone()),
            (Value::Null, _) if schema.is_nullable() => Ok(value.clone()),

            // Schema::Any accepts anything
            (_, Schema::Any) => Ok(value.clone()),

            // Type coercion cases
            (Value::String(s), Schema::Int) => self.coerce_string_to_int(s, flags, confidence),
            (Value::String(s), Schema::UInt) => self.coerce_string_to_uint(s, flags, confidence),
            (Value::String(s), Schema::Float) => self.coerce_string_to_float(s, flags, confidence),
            (Value::String(s), Schema::Bool) => self.coerce_string_to_bool(s, flags, confidence),
            (Value::Number(n), Schema::Int) if n.is_f64() => {
                self.coerce_float_to_int(n.as_f64().unwrap(), flags, confidence)
            }
            (Value::Number(n), Schema::UInt) if n.is_f64() => {
                self.coerce_float_to_uint(n.as_f64().unwrap(), flags, confidence)
            }
            // Int to Float coercion (integers are valid numbers in JSON Schema)
            (Value::Number(n), Schema::Float) if n.is_i64() || n.is_u64() => {
                // Convert integer to float - this is lossless for most practical values
                let float_val = if let Some(i) = n.as_i64() {
                    i as f64
                } else if let Some(u) = n.as_u64() {
                    u as f64
                } else {
                    unreachable!()
                };
                Ok(Value::Number(
                    serde_json::Number::from_f64(float_val).ok_or_else(|| {
                        HealingError::ParseError {
                            input: format!("{}", n),
                            expected_type: "float".to_string(),
                        }
                    })?,
                ))
            }

            // Array coercion
            (Value::Array(arr), Schema::Array(elem_schema)) => {
                self.coerce_array(arr, elem_schema, flags, confidence)
            }

            // Object coercion
            (Value::Object(map), Schema::Object(obj_schema)) => {
                self.coerce_object(map, obj_schema, flags, confidence)
            }

            // Union resolution
            (_, Schema::Union(variants)) => self.coerce_union(value, variants, flags, confidence),

            // Type mismatch
            _ => Err(HealingError::TypeMismatch {
                expected: schema.type_name().to_string(),
                found: value_type_name(value).to_string(),
            }),
        }
    }

    /// Coerce a string to an integer.
    fn coerce_string_to_int(
        &self,
        s: &str,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        if !self.config.allow_string_to_number {
            return Err(HealingError::CoercionNotAllowed {
                from: "string".to_string(),
                to: "int".to_string(),
            });
        }

        let trimmed = s.trim();
        let parsed = trimmed
            .parse::<i64>()
            .map_err(|_| HealingError::ParseError {
                input: s.to_string(),
                expected_type: "int".to_string(),
            })?;

        flags.push(CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "int".to_string(),
        });
        *confidence *= 0.9; // Penalty for type coercion

        Ok(Value::Number(parsed.into()))
    }

    /// Coerce a string to an unsigned integer.
    fn coerce_string_to_uint(
        &self,
        s: &str,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        if !self.config.allow_string_to_number {
            return Err(HealingError::CoercionNotAllowed {
                from: "string".to_string(),
                to: "uint".to_string(),
            });
        }

        let trimmed = s.trim();
        let parsed = trimmed
            .parse::<u64>()
            .map_err(|_| HealingError::ParseError {
                input: s.to_string(),
                expected_type: "uint".to_string(),
            })?;

        flags.push(CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "uint".to_string(),
        });
        *confidence *= 0.9;

        Ok(Value::Number(parsed.into()))
    }

    /// Coerce a string to a float.
    fn coerce_string_to_float(
        &self,
        s: &str,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        if !self.config.allow_string_to_number {
            return Err(HealingError::CoercionNotAllowed {
                from: "string".to_string(),
                to: "float".to_string(),
            });
        }

        let trimmed = s.trim();
        let parsed = trimmed
            .parse::<f64>()
            .map_err(|_| HealingError::ParseError {
                input: s.to_string(),
                expected_type: "float".to_string(),
            })?;

        flags.push(CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "float".to_string(),
        });
        *confidence *= 0.9;

        serde_json::Number::from_f64(parsed)
            .map(Value::Number)
            .ok_or_else(|| HealingError::ParseError {
                input: s.to_string(),
                expected_type: "float".to_string(),
            })
    }

    /// Coerce a string to a boolean.
    fn coerce_string_to_bool(
        &self,
        s: &str,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        if !self.config.allow_string_to_bool {
            return Err(HealingError::CoercionNotAllowed {
                from: "string".to_string(),
                to: "bool".to_string(),
            });
        }

        let trimmed = s.trim().to_lowercase();
        let result = match trimmed.as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => {
                return Err(HealingError::ParseError {
                    input: s.to_string(),
                    expected_type: "bool".to_string(),
                });
            }
        };

        flags.push(CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "bool".to_string(),
        });
        *confidence *= 0.9;

        Ok(Value::Bool(result))
    }

    /// Coerce a float to an integer (truncation).
    fn coerce_float_to_int(
        &self,
        f: f64,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        if !self.config.allow_float_to_int {
            return Err(HealingError::CoercionNotAllowed {
                from: "float".to_string(),
                to: "int".to_string(),
            });
        }

        let truncated = f.trunc() as i64;

        flags.push(CoercionFlag::TypeCoercion {
            from: "float".to_string(),
            to: "int".to_string(),
        });

        // Bigger penalty if we're losing precision
        if (f - truncated as f64).abs() > 0.0001 {
            *confidence *= 0.85;
        } else {
            *confidence *= 0.95;
        }

        Ok(Value::Number(truncated.into()))
    }

    /// Coerce a float to an unsigned integer (truncation).
    fn coerce_float_to_uint(
        &self,
        f: f64,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        if !self.config.allow_float_to_int {
            return Err(HealingError::CoercionNotAllowed {
                from: "float".to_string(),
                to: "uint".to_string(),
            });
        }

        if f < 0.0 {
            return Err(HealingError::ParseError {
                input: f.to_string(),
                expected_type: "uint".to_string(),
            });
        }

        let truncated = f.trunc() as u64;

        flags.push(CoercionFlag::TypeCoercion {
            from: "float".to_string(),
            to: "uint".to_string(),
        });

        if (f - truncated as f64).abs() > 0.0001 {
            *confidence *= 0.85;
        } else {
            *confidence *= 0.95;
        }

        Ok(Value::Number(truncated.into()))
    }

    /// Coerce an array to match an array schema.
    fn coerce_array(
        &self,
        arr: &[Value],
        elem_schema: &Schema,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        let mut coerced_elements = Vec::with_capacity(arr.len());

        for elem in arr {
            let coerced = self.coerce_recursive(elem, elem_schema, flags, confidence)?;
            coerced_elements.push(coerced);
        }

        Ok(Value::Array(coerced_elements))
    }

    /// Coerce an object to match an object schema.
    fn coerce_object(
        &self,
        map: &serde_json::Map<String, Value>,
        obj_schema: &ObjectSchema,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        let mut result = serde_json::Map::new();
        let mut matched_input_keys: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for field in &obj_schema.fields {
            if let Some(value) = self.find_field_value(map, field, flags, confidence) {
                // Track which input key was consumed (exact or fuzzy)
                matched_input_keys.insert(self.resolve_input_key(map, field));
                // Recursively coerce the field value
                let coerced = self.coerce_recursive(value, &field.schema, flags, confidence)?;
                result.insert(field.name.clone(), coerced);
            } else if field.required {
                // Required field missing and no default
                if let Some(default) = &field.default {
                    flags.push(CoercionFlag::UsedDefaultValue {
                        field: field.name.clone(),
                    });
                    *confidence *= 0.9;
                    result.insert(field.name.clone(), default.clone());
                } else if flags.contains(&CoercionFlag::TruncatedJson) {
                    if self.config.strict_required {
                        return Err(HealingError::TruncatedRequiredField {
                            field_name: field.name.clone(),
                        });
                    }
                    flags.push(CoercionFlag::RequiredFieldDefaultedToNull {
                        field: field.name.clone(),
                    });
                    *confidence *= 0.7;
                    result.insert(field.name.clone(), Value::Null);
                } else {
                    return Err(HealingError::MissingField {
                        field: field.name.clone(),
                    });
                }
            } else if self.config.inject_defaults {
                // Optional field missing, inject default if available
                if let Some(default) = &field.default {
                    flags.push(CoercionFlag::UsedDefaultValue {
                        field: field.name.clone(),
                    });
                    *confidence *= 0.95;
                    result.insert(field.name.clone(), default.clone());
                }
            }
        }

        // Preserve additional fields not in the schema when allowed
        if obj_schema.allow_additional_fields {
            for (key, value) in map.iter() {
                if !matched_input_keys.contains(key) {
                    result.insert(key.clone(), value.clone());
                }
            }
        }

        Ok(Value::Object(result))
    }

    /// Resolve which input key was actually matched for a given schema field.
    fn resolve_input_key(&self, map: &serde_json::Map<String, Value>, field: &Field) -> String {
        // Exact match
        if map.contains_key(&field.name) {
            return field.name.clone();
        }
        // Aliases
        for alias in &field.aliases {
            if map.contains_key(alias) {
                return alias.clone();
            }
        }
        // Case-insensitive
        for key in map.keys() {
            if key.eq_ignore_ascii_case(&field.name) {
                return key.clone();
            }
        }
        // snake/camel conversion
        let snake = to_snake_case(&field.name);
        let camel = to_camel_case(&field.name);
        for key in map.keys() {
            if key == &snake
                || key == &camel
                || key.eq_ignore_ascii_case(&snake)
                || key.eq_ignore_ascii_case(&camel)
            {
                return key.clone();
            }
        }
        // Fuzzy match fallback
        let mut best: Option<(String, f64)> = None;
        for key in map.keys() {
            let sim = jaro_winkler(&field.name, key);
            if sim >= self.config.fuzzy_match_threshold
                && best.as_ref().map_or(true, |(_, s)| sim > *s)
            {
                best = Some((key.clone(), sim));
            }
        }
        best.map(|(k, _)| k).unwrap_or_default()
    }

    /// Find a field value in an object using fuzzy matching.
    fn find_field_value<'a>(
        &self,
        map: &'a serde_json::Map<String, Value>,
        field: &Field,
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Option<&'a Value> {
        // 1. Try exact match
        if let Some(value) = map.get(&field.name) {
            return Some(value);
        }

        // 2. Try aliases
        for alias in &field.aliases {
            if let Some(value) = map.get(alias) {
                flags.push(CoercionFlag::FuzzyFieldMatch {
                    expected: field.name.clone(),
                    found: alias.clone(),
                });
                *confidence *= 0.98;
                return Some(value);
            }
        }

        // 3. Try case-insensitive match
        for (key, value) in map.iter() {
            if key.eq_ignore_ascii_case(&field.name) {
                flags.push(CoercionFlag::FuzzyFieldMatch {
                    expected: field.name.clone(),
                    found: key.clone(),
                });
                *confidence *= 0.95;
                return Some(value);
            }
        }

        // 4. Try snake_case ↔ camelCase conversion (case-insensitive)
        let snake = to_snake_case(&field.name);
        let camel = to_camel_case(&field.name);

        for (key, value) in map.iter() {
            if key == &snake
                || key == &camel
                || key.eq_ignore_ascii_case(&snake)
                || key.eq_ignore_ascii_case(&camel)
            {
                flags.push(CoercionFlag::FuzzyFieldMatch {
                    expected: field.name.clone(),
                    found: key.clone(),
                });
                *confidence *= 0.93;
                return Some(value);
            }
        }

        // 5. Try Jaro-Winkler fuzzy matching
        let mut best_match: Option<(&String, &Value, f64)> = None;

        for (key, value) in map.iter() {
            let similarity = jaro_winkler(&field.name, key);
            if similarity >= self.config.fuzzy_match_threshold {
                if let Some((_, _, best_score)) = best_match {
                    if similarity > best_score {
                        best_match = Some((key, value, similarity));
                    }
                } else {
                    best_match = Some((key, value, similarity));
                }
            }
        }

        if let Some((key, value, similarity)) = best_match {
            flags.push(CoercionFlag::FuzzyFieldMatch {
                expected: field.name.clone(),
                found: key.clone(),
            });
            // Penalty scales with how fuzzy the match is
            *confidence *= 0.85 + (0.1 * similarity as f32);
            return Some(value);
        }

        None
    }

    /// Resolve a union type by trying each variant and picking the best match.
    fn coerce_union(
        &self,
        value: &Value,
        variants: &[Schema],
        flags: &mut Vec<CoercionFlag>,
        confidence: &mut f32,
    ) -> Result<Value, HealingError> {
        let mut results: Vec<(Value, Vec<CoercionFlag>, f32)> = Vec::new();

        // Try each variant
        for variant in variants {
            let mut variant_flags = flags.clone();
            let mut variant_confidence = *confidence;

            match self.coerce_recursive(value, variant, &mut variant_flags, &mut variant_confidence)
            {
                Ok(coerced) => {
                    results.push((coerced, variant_flags, variant_confidence));
                }
                Err(_) => continue,
            }
        }

        if results.is_empty() {
            return Err(HealingError::NoMatchingVariant {
                value: value.clone(),
            });
        }

        // Sort by confidence (highest first)
        results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // Take the best match
        let (best_value, best_flags, best_confidence) = results.into_iter().next().unwrap();

        *flags = best_flags;
        *confidence = best_confidence;

        Ok(best_value)
    }
}

impl Default for CoercionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a human-readable type name for a JSON value.
fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_i64() => "int",
        Value::Number(n) if n.is_u64() => "uint",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_exact_type_match() {
        let engine = CoercionEngine::new();

        // String
        let result = engine.coerce(&json!("hello"), &Schema::String).unwrap();
        assert_eq!(result.value, json!("hello"));
        assert_eq!(result.confidence, 1.0);
        assert!(result.flags.is_empty());

        // Int
        let result = engine.coerce(&json!(42), &Schema::Int).unwrap();
        assert_eq!(result.value, json!(42));
        assert_eq!(result.confidence, 1.0);

        // Bool
        let result = engine.coerce(&json!(true), &Schema::Bool).unwrap();
        assert_eq!(result.value, json!(true));
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_string_to_int_coercion() {
        let engine = CoercionEngine::new();

        let result = engine.coerce(&json!("42"), &Schema::Int).unwrap();
        assert_eq!(result.value, json!(42));
        assert!(result.confidence < 1.0);
        assert_eq!(result.flags.len(), 1);
        assert!(matches!(result.flags[0], CoercionFlag::TypeCoercion { .. }));
    }

    #[test]
    fn test_string_to_bool_coercion() {
        let engine = CoercionEngine::new();

        // Various true values
        for s in ["true", "1", "yes", "on", "TRUE", " true "] {
            let result = engine.coerce(&json!(s), &Schema::Bool).unwrap();
            assert_eq!(result.value, json!(true), "Failed for: {}", s);
        }

        // Various false values
        for s in ["false", "0", "no", "off", "FALSE", " false "] {
            let result = engine.coerce(&json!(s), &Schema::Bool).unwrap();
            assert_eq!(result.value, json!(false), "Failed for: {}", s);
        }
    }

    #[test]
    fn test_float_to_int_coercion() {
        let engine = CoercionEngine::new();

        // Exact float (no precision loss)
        let result = engine.coerce(&json!(42.0), &Schema::Int).unwrap();
        assert_eq!(result.value, json!(42));
        assert!(result.confidence > 0.9);

        // Float with decimals (precision loss)
        let result = engine.coerce(&json!(42.7), &Schema::Int).unwrap();
        assert_eq!(result.value, json!(42));
        assert!(result.confidence < 0.9);
    }

    #[test]
    fn test_array_coercion() {
        let engine = CoercionEngine::new();

        let input = json!(["1", "2", "3"]);
        let schema = Schema::array(Schema::Int);

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value, json!([1, 2, 3]));
        assert_eq!(result.flags.len(), 3); // Three type coercions
    }

    #[test]
    fn test_object_coercion_exact_match() {
        let engine = CoercionEngine::new();

        let input = json!({
            "name": "Alice",
            "age": 30
        });

        let schema = Schema::object(vec![
            ("name".into(), Schema::String, true),
            ("age".into(), Schema::Int, true),
        ]);

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 30);
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_object_coercion_case_insensitive() {
        let engine = CoercionEngine::new();

        let input = json!({
            "NAME": "Alice",
            "AGE": 30
        });

        let schema = Schema::object(vec![
            ("name".into(), Schema::String, true),
            ("age".into(), Schema::Int, true),
        ]);

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 30);
        assert!(result.confidence < 1.0);
        assert_eq!(result.flags.len(), 2); // Two fuzzy matches
    }

    #[test]
    fn test_object_coercion_snake_camel() {
        let engine = CoercionEngine::new();

        let input = json!({
            "first_name": "Alice",
            "lastName": "Smith"
        });

        let schema = Schema::object(vec![
            ("firstName".into(), Schema::String, true),
            ("last_name".into(), Schema::String, true),
        ]);

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["firstName"], "Alice");
        assert_eq!(result.value["last_name"], "Smith");
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_default_value_injection() {
        let engine = CoercionEngine::new();

        let input = json!({
            "name": "Alice"
        });

        let schema = Schema::Object(crate::schema::ObjectSchema {
            fields: vec![
                crate::schema::Field::required("name", Schema::String),
                crate::schema::Field::optional("age", Schema::Int).with_default(json!(25)),
            ],
            allow_additional_fields: false,
        });

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 25);
        assert!(result
            .flags
            .iter()
            .any(|f| matches!(f, CoercionFlag::UsedDefaultValue { .. })));
    }

    #[test]
    fn test_union_resolution() {
        let engine = CoercionEngine::new();

        let schema = Schema::union(vec![Schema::Int, Schema::String]);

        // Int variant
        let result = engine.coerce(&json!(42), &schema).unwrap();
        assert_eq!(result.value, json!(42));

        // String variant
        let result = engine.coerce(&json!("hello"), &schema).unwrap();
        assert_eq!(result.value, json!("hello"));

        // Ambiguous (could be either) - should pick best match
        let result = engine.coerce(&json!("123"), &schema).unwrap();
        // String is exact match (confidence 1.0), Int requires coercion (confidence 0.9)
        assert_eq!(result.value, json!("123"));
    }

    #[test]
    fn test_confidence_threshold() {
        let config = CoercionConfig {
            min_confidence: 0.95,
            ..Default::default()
        };
        let engine = CoercionEngine::with_config(config);

        let input = json!({"NAME": "Alice", "AGE": "30"});
        let schema = Schema::object(vec![
            ("name".into(), Schema::String, true),
            ("age".into(), Schema::Int, true),
        ]);

        // This should fail because fuzzy match + type coercion drops confidence below 0.95
        let result = engine.coerce(&input, &schema);
        assert!(result.is_err());
    }

    #[test]
    fn test_jaro_winkler_fuzzy_match() {
        let engine = CoercionEngine::new();

        let input = json!({
            "usrName": "Alice",  // Typo: should match userName
            "emailAdress": "alice@example.com"  // Typo: should match emailAddress
        });

        let schema = Schema::object(vec![
            ("userName".into(), Schema::String, true),
            ("emailAddress".into(), Schema::String, true),
        ]);

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["userName"], "Alice");
        assert_eq!(result.value["emailAddress"], "alice@example.com");
        assert!(result.confidence < 1.0);
    }

    #[test]
    fn test_missing_required_field() {
        let engine = CoercionEngine::new();

        let input = json!({
            "name": "Alice"
        });

        let schema = Schema::object(vec![
            ("name".into(), Schema::String, true),
            ("age".into(), Schema::Int, true), // Required but missing
        ]);

        let result = engine.coerce(&input, &schema);
        assert!(result.is_err());
    }

    // --- Task 2B: allow_additional_fields tests ---

    #[test]
    fn test_additional_fields_preserved_when_allowed() {
        let engine = CoercionEngine::new();

        let input = json!({
            "name": "Alice",
            "age": 30,
            "extra_field": "bonus",
            "score": 99
        });

        let schema = Schema::Object(ObjectSchema {
            fields: vec![
                Field::required("name", Schema::String),
                Field::required("age", Schema::Int),
            ],
            allow_additional_fields: true,
        });

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 30);
        assert_eq!(result.value["extra_field"], "bonus");
        assert_eq!(result.value["score"], 99);
    }

    #[test]
    fn test_additional_fields_dropped_when_not_allowed() {
        let engine = CoercionEngine::new();

        let input = json!({
            "name": "Alice",
            "age": 30,
            "extra_field": "bonus"
        });

        let schema = Schema::Object(ObjectSchema {
            fields: vec![
                Field::required("name", Schema::String),
                Field::required("age", Schema::Int),
            ],
            allow_additional_fields: false,
        });

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.value["age"], 30);
        assert!(result.value.get("extra_field").is_none());
    }

    #[test]
    fn test_additional_fields_nested_objects() {
        let engine = CoercionEngine::new();

        let inner_schema = Schema::Object(ObjectSchema {
            fields: vec![Field::required("id", Schema::Int)],
            allow_additional_fields: true,
        });

        let schema = Schema::Object(ObjectSchema {
            fields: vec![Field::required("data", inner_schema)],
            allow_additional_fields: true,
        });

        let input = json!({
            "data": {"id": 1, "nested_extra": "hello"},
            "top_extra": true
        });

        let result = engine.coerce(&input, &schema).unwrap();
        assert_eq!(result.value["data"]["id"], 1);
        assert_eq!(result.value["data"]["nested_extra"], "hello");
        assert_eq!(result.value["top_extra"], true);
    }

    // --- Task 2C: strict_required mode tests ---

    #[test]
    fn test_strict_required_rejects_truncated_null() {
        let config = CoercionConfig::strict();
        let engine = CoercionEngine::with_config(config);

        let input = json!({
            "name": "Alice"
        });

        let schema = Schema::Object(ObjectSchema {
            fields: vec![
                Field::required("name", Schema::String),
                Field::required("diagnosis", Schema::String),
            ],
            allow_additional_fields: false,
        });

        // Simulate truncated JSON by pre-setting the flag
        let mut flags = vec![CoercionFlag::TruncatedJson];
        let mut confidence = 1.0f32;
        let result = engine.coerce_recursive(&input, &schema, &mut flags, &mut confidence);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, HealingError::TruncatedRequiredField { ref field_name } if field_name == "diagnosis"),
            "Expected TruncatedRequiredField error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_lenient_required_injects_null_with_flag() {
        let config = CoercionConfig::lenient();
        let engine = CoercionEngine::with_config(config);

        let input = json!({
            "name": "Alice"
        });

        let schema = Schema::Object(ObjectSchema {
            fields: vec![
                Field::required("name", Schema::String),
                Field::required("diagnosis", Schema::String),
            ],
            allow_additional_fields: false,
        });

        let mut flags = vec![CoercionFlag::TruncatedJson];
        let mut confidence = 1.0f32;
        let result = engine
            .coerce_recursive(&input, &schema, &mut flags, &mut confidence)
            .unwrap();

        assert_eq!(result["name"], "Alice");
        assert_eq!(result["diagnosis"], Value::Null);
        assert!(flags.iter().any(|f| matches!(
            f,
            CoercionFlag::RequiredFieldDefaultedToNull { field } if field == "diagnosis"
        )));
        assert!(confidence < 1.0);
    }

    #[test]
    fn test_strict_config_constructor() {
        let config = CoercionConfig::strict();
        assert!(config.strict_required);

        let config = CoercionConfig::lenient();
        assert!(!config.strict_required);
    }
}
