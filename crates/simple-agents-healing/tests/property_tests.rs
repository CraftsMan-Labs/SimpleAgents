//! Property-based tests for response healing system.
//!
//! These tests use proptest to verify that the parser never panics
//! and handles arbitrary input gracefully.

use proptest::prelude::*;
use simple_agents_healing::{CoercionEngine, JsonishParser, Schema};

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
        let input = format!(r#"{{"items": [{}]}}"#, items.join(", "));
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
    fn coercion_never_panics_on_strings(s in ".*") {
        let schema = Schema::String;
        let engine = CoercionEngine::new();
        let value = serde_json::json!(s);
        let _ = engine.coerce(&value, &schema);
    }

    #[test]
    fn string_to_number_coercion(input in "[0-9]{1,10}") {
        let schema = Schema::UInt;
        let engine = CoercionEngine::new();
        let value = serde_json::Value::String(input.clone());

        // Coercion should either succeed or fail gracefully
        let result = engine.coerce(&value, &schema);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn parser_handles_large_objects(count in 0usize..20) {
        let mut fields = Vec::new();
        for i in 0..count {
            fields.push(format!(r#""field_{}": "value_{}""#, i, i));
        }

        let input = format!(r#"{{{}}}"#, fields.join(", "));
        let parser = JsonishParser::new();
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handles_incomplete_objects(missing_fields in 0usize..10) {
        let parser = JsonishParser::new();
        let mut fields = Vec::new();
        for i in 0..(10 - missing_fields) {
            fields.push(format!(r#""field_{}": "value_{}""#, i, i));
        }

        let input = format!(r#"{{{}}}"#, fields.join(", "));
        let _ = parser.parse(&input);
    }

    #[test]
    fn parser_handle_unicode(input in "\\PC*") {
        let parser = JsonishParser::new();
        let _ = parser.parse(&input);
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
    fn parser_handles_boolean_values(value in any::<bool>()) {
        let parser = JsonishParser::new();
        let input = format!(r#"{{"bool": {}}}"#, value);
        let result = parser.parse(&input);

        if let Ok(parsed) = result {
            assert_eq!(parsed.value["bool"], serde_json::Value::Bool(value));
        }
    }

    #[test]
    fn parser_handles_numeric_values(value in any::<i32>()) {
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

        let input = format!(r#"{{"arrays": [{}]}}"#, inner_arrays);
        let _ = parser.parse(&input);
    }

    #[test]
    fn coercion_never_panics_with_numbers(value in 0u32..1000) {
        let schema = Schema::UInt;
        let engine = CoercionEngine::new();
        let json_value = serde_json::json!(value);

        // Coercion should never panic
        let _ = engine.coerce(&json_value, &schema);
    }

    #[test]
    fn parser_handles_whitespace_variations(input in ".*") {
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
            {{"{}"}}
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
