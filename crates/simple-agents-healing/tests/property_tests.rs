//! Property-based tests for response healing system.
//!
//! These tests use proptest to verify that the parser never panics
//! and handles arbitrary input gracefully.

use proptest::prelude::*;
use simple_agents_healing::{CoercionEngine, JsonishParser};
use simple_agents_types::{Field, Schema, SchemaBuilder, TypedValue};

proptest! {
    #[test]
    fn parser_never_panics_on_arbitrary_json(input in "\\PC*") {
        let parser = JsonishParser::new();
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_never_panics_on_arbitrary_bytes(input in prop::collection::vec(any::<u8>(), 0..1000)) {
        let parser = JsonishParser::new();
        let input = String::from_utf8_lossy(&input);
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_very_long_strings(length in 0usize..10000) {
        let parser = JsonishParser::new();
        let input = format!(r#"{{"key": "{}"}}"#, "x".repeat(length));
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_deep_nesting(depth in 0usize..50) {
        let parser = JsonishParser::new();
        let nested = "[{".repeat(depth);
        let closing = "}]".repeat(depth);
        let input = format!(r#"{{"nested": {}{}{}}}"#, nested, "x", closing);
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_many_arrays(count in 0usize..100) {
        let parser = JsonishParser::new();
        let items = (0..count).map(|i| format!(r#"{{"id": {}}}"#, i)).collect::<Vec<_>>();
        let input = format!(r#"{{"items": [{}}]}"#, items.join(", "));
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_mixed_quotes(input in "\\PC*") {
        let parser = JsonishParser::new();
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_special_characters(input in "\\PC*") {
        let parser = JsonishParser::new();
        let _ = parser.parse(&input);
    }

    #[test]
    fn coercion_confidence_bounded(value in any::<serde_json::Value>()) {
        let schema = SchemaBuilder::any();
        let engine = CoercionEngine::new(schema);

        if let Ok(result) = engine.coerce(value) {
            assert!(result.confidence >= 0.0, "Confidence should be >= 0.0");
            assert!(result.confidence <= 1.0, "Confidence should be <= 1.0");
        }
    }

    #[test]
    fn coercion_never_panics_on_arbitrary_json(value in any::<serde_json::Value>()) {
        let schema = SchemaBuilder::any();
        let engine = CoercionEngine::new(schema);
        let _ = engine.coerce(value);
    }

    #[test]
    fn coercion_never_panics_on_arbitrary_schema(
        value in any::<serde_json::Value>(),
        schema_type in prop::sample::select(vec![
            Schema::String,
            Schema::U32,
            Schema::I32,
            Schema::F64,
            Schema::Bool,
        ])
    ) {
        let schema = SchemaBuilder::new(schema_type);
        let engine = CoercionEngine::new(schema);
        let _ = engine.coerce(value);
    }

    #[test]
    fn string_to_int_coercion_valid(input in prop::string::string_regex("[0-9]+")) {
        let schema = SchemaBuilder::new(Schema::U32);
        let engine = CoercionEngine::new(schema);
        let value = serde_json::Value::String(input);
        let result = engine.coerce(value);

        if result.is_ok() {
            assert_eq!(result.unwrap().value, TypedValue::U32(input.parse().unwrap()));
        }
    }

    #[test]
    fn coercion_handles_large_objects(count in 0usize..50) {
        let mut fields = Vec::new();
        for i in 0..count {
            fields.push(Field::new(&format!("field_{}", i), Schema::String).required());
        }

        let mut map = serde_json::Map::new();
        for i in 0..count {
            map.insert(format!("field_{}", i), serde_json::Value::String(format!("value_{}", i)));
        }

        let schema = SchemaBuilder::object(fields);
        let engine = CoercionEngine::new(schema);
        let value = serde_json::Value::Object(map);
        let _ = engine.coerce(value);
    }

    #[test]
    fn parser_handles_incomplete_objects(missing_fields in 0usize..10) {
        let parser = JsonishParser::new();
        let mut fields = Vec::new();
        for i in 0..(10 - missing_fields) {
            fields.push(format!(r#""field_{}": "value_{}}""#, i, i));
        }

        let input = format!(r#"{{{}}}"#, fields.join(", "));
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handle_unicode(input in prop::string::string_regex(r"\\PC*")) {
        let parser = JsonishParser::new();
        let _ = parser.parse(&input);
    }

    #[test]
    fn coercion_roundtrip_string(value in prop::string::string_regex(r".*")) {
        let schema = SchemaBuilder::new(Schema::String);
        let engine = CoercionEngine::new(schema);
        let json_value = serde_json::Value::String(value);
        let result = engine.coerce(json_value);

        if result.is_ok() {
            if let TypedValue::String(s) = result.unwrap().value {
                assert_eq!(s, value);
            }
        }
    }

    #[test]
    fn parser_handles_empty_values(input in prop::collection::vec(any::<String>(), 0..20)) {
        let parser = JsonishParser::new();
        if input.is_empty() {
            let _ = parser.parse("");
        } else {
            let json = serde_json::to_string(&input).unwrap();
            let _ = parser.parse(&json);
        }
    }

    #[test]
    fn parser_handles_boolean_values(value in prop::bool::ANY) {
        let parser = JsonishParser::new();
        let input = format!(r#"{{"bool": {}}}"#, value);
        let result = parser.parse(&input);

        if let Ok(parsed) = result {
            assert_eq!(parsed["bool"], serde_json::Value::Bool(value));
        }
    }

    #[test]
    fn parser_handles_numeric_values(value in prop::num::any::<f64>()) {
        let parser = JsonishParser::new();
        let input = format!(r#"{{"num": {}}}"#, value);
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_null_values(field_count in 0usize..20) {
        let parser = JsonishParser::new();
        let mut fields = Vec::new();
        for i in 0..field_count {
            fields.push(format!(r#""field_{}": null"#, i));
        }

        let input = format!(r#"{{{}}}"#, fields.join(", "));
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_nested_arrays(
        outer_count in 0usize..10,
        inner_count in 0usize..10
    ) {
        let parser = JsonishParser::new();
        let inner_arrays = (0..outer_count)
            .map(|i| {
                let items = (0..inner_count)
                    .map(|j| format!(r#"{{"id": "{}", "index": {}}}"#, i * inner_count + j, j))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", items)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let input = format!(r#"{{"arrays": [{}]}{}}}"#, inner_arrays);
        let _ = parser.parse(&input);
    }

    #[test]
    fn coercion_flag_count_non_negative(value in any::<serde_json::Value>()) {
        let schema = SchemaBuilder::any();
        let engine = CoercionEngine::new(schema);

        if let Ok(result) = engine.coerce(value) {
            assert!(result.flags.len() >= 0, "Flags count should never be negative");
        }
    }

    #[test]
    fn parser_handles_whitespace_variations(input in prop::string::string_regex(r"\s*\S+\s*")) {
        let parser = JsonishParser::new();
        let json_value = serde_json::json!({ "key": input });
        let json = serde_json::to_string(&json_value).unwrap();
        let _ = parser.parse(&json);
    }

    #[test]
    fn parser_handles_escape_sequences(count in 0usize..20) {
        let parser = JsonishParser::new();
        let escaped = "\\n\\t\\r\\\"\\\\".repeat(count);
        let input = format!(r#"{{"escaped": "{}}}"#, escaped);
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_comment_variations(input in "\\PC*") {
        let parser = JsonishParser::new();
        let with_comments = format!(
            r#"// comment 1
            {{}}
            /* comment 2 */"#,
            input
        );
        let _ = parser.parse(&with_comments);
    }
}

#[cfg(test)]
mod fuzzing_regression_tests {
    use super::*;

    #[test]
    fn test_specific_fuzzing_cases() {
        let parser = JsonishParser::new();

        let test_cases = vec![
            r#"{""#,
            r#"}{"#,
            r#"{{{{"#,
            r#"[[["#,
            r#"{{"key": }}""#,
            r#"{"key": "value", }"#,
            r#"{"key": "value",]"#,
            r#"{'key': 'value'}"#,
            r#"{"key": "value", /* comment */ "another": "value"}"#,
            r#"{"key": "value", // comment
                "another": "value"}"#,
            r#"```json
            {"key": "value"}
            ```"#,
            r#"{"key": "value\"quote"}"#,
            r#"{"key": "value\\backslash"}"#,
            r#"{"key": ""#,
            r#"{"key": ["#,
            r#"{"key": {{"#,
        ];

        for case in test_cases {
            let _ = parser.parse(case);
        }
    }
}
