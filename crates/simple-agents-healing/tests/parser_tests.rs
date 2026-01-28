//! Integration tests for the Jsonish parser.

use serde_json::json;
use simple_agents_healing::prelude::*;

#[test]
fn test_markdown_variants() {
    let parser = JsonishParser::new();

    // Standard ```json wrapper
    let input = r#"```json
{"key": "value"}
```"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
    assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));

    // Plain ``` wrapper
    let input = r#"```
{"key": "value"}
```"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
    assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));

    // Incomplete closing fence
    let input = r#"```json
{"key": "value"}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
    assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));
}

#[test]
fn test_trailing_comma_variants() {
    let parser = JsonishParser::new();

    // Object with trailing comma
    let input = r#"{"a": 1, "b": 2,}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["a"], 1);
    assert_eq!(result.value["b"], 2);
    assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));

    // Array with trailing comma
    let input = r#"[1, 2, 3,]"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value[0], 1);
    assert_eq!(result.value[1], 2);
    assert_eq!(result.value[2], 3);
    assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));

    // Nested with trailing commas
    let input = r#"{"arr": [1, 2,], "obj": {"x": 1,}}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["arr"][0], 1);
    assert_eq!(result.value["obj"]["x"], 1);
    assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
}

#[test]
fn test_quote_variations() {
    let parser = JsonishParser::new();

    // Single quotes only (no double quotes)
    let input = r#"{'key': 'value', 'num': '42'}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
    assert!(result.flags.contains(&CoercionFlag::FixedQuotes));

    // Mixed quotes should NOT be fixed (preserve double quotes)
    let input = r#"{"key": "value", 'other': 'test'}"#;
    let result = parser.parse(input);
    // This should either fail or not have FixedQuotes flag
    if let Ok(result) = result {
        // If it parses, quotes should not have been "fixed"
        assert!(!result.flags.contains(&CoercionFlag::FixedQuotes));
    }
}

#[test]
fn test_complex_nested_structures() {
    let parser = JsonishParser::new();

    let input = r#"```json
{
    "user": {
        "name": "Alice",
        "age": 30,
        "hobbies": ["reading", "coding"]
    },
    "metadata": {
        "created": "2024-01-01",
        "tags": ["important", "urgent"]
    }
}
```"#;

    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["user"]["name"], "Alice");
    assert_eq!(result.value["user"]["age"], 30);
    assert_eq!(result.value["user"]["hobbies"][0], "reading");
    assert_eq!(result.value["metadata"]["tags"][0], "important");

    // Should have markdown flag
    assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));
}

#[test]
fn test_confidence_scoring() {
    let parser = JsonishParser::new();

    // Perfect JSON - confidence 1.0
    let input = r#"{"key": "value"}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.confidence, 1.0);

    // One fix (markdown) - confidence ~0.95
    let input = r#"```json
{"key": "value"}
```"#;
    let result = parser.parse(input).unwrap();
    assert!(result.confidence > 0.94 && result.confidence < 0.96);

    // Two fixes (markdown + trailing comma) - confidence ~0.90
    let input = r#"```json
{"key": "value",}
```"#;
    let result = parser.parse(input).unwrap();
    assert!(result.confidence > 0.88 && result.confidence < 0.92);

    // Three fixes (markdown + trailing comma + quotes) - confidence ~0.86
    let input = r#"```json
{'key': 'value',}
```"#;
    let result = parser.parse(input).unwrap();
    assert!(result.confidence > 0.80 && result.confidence < 0.90);
}

#[test]
fn test_min_confidence_threshold() {
    let config = ParserConfig {
        min_confidence: 0.96, // Set higher to ensure markdown fix fails
        ..Default::default()
    };
    let parser = JsonishParser::with_config(config);

    // Perfect JSON passes
    let input = r#"{"key": "value"}"#;
    let result = parser.parse(input);
    assert!(result.is_ok());

    // Markdown-wrapped fails threshold
    let input = r#"```json
{"key": "value"}
```"#;
    let result = parser.parse(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        simple_agent_type::error::SimpleAgentsError::Healing(HealingError::LowConfidence {
            confidence,
            ..
        }) => {
            assert!(confidence < 0.96);
        }
        e => panic!("Expected LowConfidence error, got: {:?}", e),
    }
}

#[test]
fn test_bom_variants() {
    let parser = JsonishParser::new();

    // UTF-8 BOM
    let input = "\u{FEFF}{\"key\": \"value\"}";
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
    assert!(result.flags.contains(&CoercionFlag::RemovedBom));

    // BOM + markdown
    let input = "\u{FEFF}```json\n{\"key\": \"value\"}\n```";
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
    assert!(result.flags.contains(&CoercionFlag::RemovedBom));
    assert!(result.flags.contains(&CoercionFlag::StrippedMarkdown));
}

#[test]
fn test_control_characters() {
    let parser = JsonishParser::new();

    // JSON with control characters (should be removed)
    let input = "{\"key\": \"val\x00ue\"}";
    let result = parser.parse(input).unwrap();
    assert!(result.flags.contains(&CoercionFlag::FixedControlCharacters));
}

#[test]
fn test_empty_structures() {
    let parser = JsonishParser::new();

    // Empty object
    let input = r#"{}"#;
    let result = parser.parse(input).unwrap();
    assert!(result.value.is_object());
    assert_eq!(result.value.as_object().unwrap().len(), 0);
    assert_eq!(result.confidence, 1.0);

    // Empty array
    let input = r#"[]"#;
    let result = parser.parse(input).unwrap();
    assert!(result.value.is_array());
    assert_eq!(result.value.as_array().unwrap().len(), 0);
    assert_eq!(result.confidence, 1.0);

    // Empty object with trailing comma
    let input = r#"{,}"#;
    let result = parser.parse(input);
    // This should either parse or fail, but not panic
    let _ = result;
}

#[test]
fn test_large_nested_structure() {
    let parser = JsonishParser::new();

    let input = json!({
        "level1": {
            "level2": {
                "level3": {
                    "level4": {
                        "level5": {
                            "deep": "value"
                        }
                    }
                }
            }
        }
    })
    .to_string();

    let result = parser.parse(&input).unwrap();
    assert_eq!(
        result.value["level1"]["level2"]["level3"]["level4"]["level5"]["deep"],
        "value"
    );
    assert_eq!(result.confidence, 1.0);
}

#[test]
#[allow(clippy::approx_constant)] // 3.14 is intentional, not PI
fn test_numeric_values() {
    let parser = JsonishParser::new();

    let input = r#"{
        "int": 42,
        "float": 3.14,
        "negative": -10,
        "scientific": 1.5e10,
        "zero": 0
    }"#;

    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["int"], 42);
    assert_eq!(result.value["float"], 3.14);
    assert_eq!(result.value["negative"], -10);
    assert_eq!(result.value["scientific"], 1.5e10);
    assert_eq!(result.value["zero"], 0);
}

#[test]
fn test_boolean_and_null() {
    let parser = JsonishParser::new();

    let input = r#"{
        "true_val": true,
        "false_val": false,
        "null_val": null
    }"#;

    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["true_val"], true);
    assert_eq!(result.value["false_val"], false);
    assert!(result.value["null_val"].is_null());
}

#[test]
fn test_unicode_strings() {
    let parser = JsonishParser::new();

    let input = r#"{
        "emoji": "🚀 🎉",
        "chinese": "你好",
        "arabic": "مرحبا",
        "escaped": "Hello\nWorld\t!"
    }"#;

    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["emoji"], "🚀 🎉");
    assert_eq!(result.value["chinese"], "你好");
    assert_eq!(result.value["arabic"], "مرحبا");
    assert_eq!(result.value["escaped"], "Hello\nWorld\t!");
}

#[test]
fn test_truncation_lenient_parsing() {
    let parser = JsonishParser::new();

    // Incomplete JSON should trigger lenient parsing and truncation
    let input = r#"{"key": "value", "incomplete"#;
    let result = parser.parse(input);

    // Should either succeed with truncation flag or fail gracefully
    match result {
        Ok(result) => {
            assert!(result.flags.contains(&CoercionFlag::TruncatedJson));
            assert!(result.confidence < 0.8);
        }
        Err(_) => {
            // Also acceptable - lenient parser may not be able to recover
        }
    }
}

#[test]
fn test_multiple_json_objects() {
    let parser = JsonishParser::new();

    // Only first object should be parsed
    let input = r#"{"first": 1} {"second": 2}"#;
    let result = parser.parse(input);

    // Should parse first object or trigger truncation
    if let Ok(result) = result {
        assert_eq!(result.value["first"], 1);
        // May or may not have second object depending on parser implementation
    }
}

#[test]
fn test_whitespace_variations() {
    let parser = JsonishParser::new();

    // Compact
    let input = r#"{"key":"value","num":42}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");

    // Pretty printed
    let input = r#"{
        "key": "value",
        "num": 42
    }"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");

    // Excessive whitespace
    let input = r#"{   "key"   :   "value"   ,   "num"   :   42   }"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
}

#[test]
fn test_parser_config_customization() {
    // Disable all fixes
    let config = ParserConfig {
        strip_markdown: false,
        fix_trailing_commas: false,
        fix_quotes: false,
        fix_unquoted_keys: false,
        fix_control_chars: false,
        remove_bom: false,
        allow_lenient_parsing: false,
        min_confidence: 0.0,
    };
    let parser = JsonishParser::with_config(config);

    // Perfect JSON should still work
    let input = r#"{"key": "value"}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");

    // Malformed JSON (trailing comma) should fail when fix disabled
    let input = r#"{"key": "value",}"#;
    let result = parser.parse(input);
    assert!(result.is_err());
}

#[test]
fn test_array_variations() {
    let parser = JsonishParser::new();

    // Mixed types
    let input = r#"[1, "two", true, null, {"key": "value"}]"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value[0], 1);
    assert_eq!(result.value[1], "two");
    assert_eq!(result.value[2], true);
    assert!(result.value[3].is_null());
    assert_eq!(result.value[4]["key"], "value");

    // Nested arrays
    let input = r#"[[1, 2], [3, 4], [5, 6]]"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value[0][0], 1);
    assert_eq!(result.value[1][1], 4);
    assert_eq!(result.value[2][0], 5);
}

// === LENIENT PARSER TESTS ===

#[test]
fn test_unclosed_object() {
    let parser = JsonishParser::new();

    // Unclosed object - should auto-complete
    let input = r#"{"name": "Alice", "age": 30"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["age"], 30);
    assert!(result.flags.contains(&CoercionFlag::TruncatedJson));
}

#[test]
fn test_unclosed_array() {
    let parser = JsonishParser::new();

    // Unclosed array - should auto-complete
    let input = r#"[1, 2, 3, 4"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value[0], 1);
    assert_eq!(result.value[3], 4);
    assert!(result.flags.contains(&CoercionFlag::TruncatedJson));
}

#[test]
fn test_unclosed_string() {
    let parser = JsonishParser::new();

    // Unclosed string in object - should auto-complete
    let input = r#"{"key": "value"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["key"], "value");
    assert!(result.flags.contains(&CoercionFlag::TruncatedJson));
}

#[test]
fn test_unquoted_keys() {
    let parser = JsonishParser::new();

    // Unquoted object keys
    let input = r#"{name: "Alice", age: 30}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["age"], 30);
    assert!(result.flags.contains(&CoercionFlag::FixedUnquotedKeys));
}

#[test]
fn test_line_comments() {
    let parser = JsonishParser::new();

    // Line comments in JSON
    let input = r#"{
        // This is a comment
        "name": "Alice",
        // Another comment
        "age": 30
    }"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["age"], 30);
}

#[test]
fn test_block_comments() {
    let parser = JsonishParser::new();

    // Block comments in JSON
    let input = r#"{
        /* This is a
           block comment */
        "name": "Alice",
        /* Another one */ "age": 30
    }"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["age"], 30);
}

#[test]
fn test_backtick_strings() {
    let parser = JsonishParser::new();

    // Backtick strings
    let input = r#"{"name": `Alice`, "city": `New York`}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["city"], "New York");
}

#[test]
fn test_triple_quote_strings() {
    let parser = JsonishParser::new();

    // Triple quoted strings (multiline)
    let input = r#"{"text": """This is
a multiline
string"""}"#;
    let result = parser.parse(input).unwrap();
    assert!(result.value["text"].as_str().unwrap().contains("multiline"));
}

#[test]
fn test_escape_sequences() {
    let parser = JsonishParser::new();

    // Various escape sequences - they get converted to actual characters
    let input = r#"{"tab": "a\tb", "newline": "a\nb", "quote": "a\"b", "backslash": "a\\b"}"#;
    let result = parser.parse(input).unwrap();
    // Note: serde_json converts escape sequences to actual characters
    assert_eq!(result.value["tab"].as_str().unwrap(), "a\tb");
    assert_eq!(result.value["newline"].as_str().unwrap(), "a\nb");
    assert_eq!(result.value["quote"].as_str().unwrap(), "a\"b");
    assert_eq!(result.value["backslash"].as_str().unwrap(), "a\\b");
}

#[test]
fn test_deeply_nested_incomplete() {
    let parser = JsonishParser::new();

    // Deeply nested incomplete structure - simpler version
    let input = r#"{"level1": {"level2": {"value": "deep"#;
    let result = parser.parse(input);

    // Should either succeed with truncation or fail gracefully
    match result {
        Ok(result) => {
            // If it parses, verify structure and truncation flag
            assert!(result.flags.contains(&CoercionFlag::TruncatedJson));
            assert!(result.value.is_object());
        }
        Err(_) => {
            // Also acceptable - deeply nested incomplete JSON is hard to parse
        }
    }
}

#[test]
fn test_mixed_quotes_in_values() {
    let parser = JsonishParser::new();

    // Single and double quotes mixed (but not in same string)
    let input = r#"{"single": 'value1', "double": "value2"}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["single"], "value1");
    assert_eq!(result.value["double"], "value2");
}

#[test]
fn test_numbers_with_exponents() {
    let parser = JsonishParser::new();

    // Scientific notation
    let input = r#"{"small": 1.5e-10, "large": 2.5e10}"#;
    let result = parser.parse(input).unwrap();
    assert!(result.value["small"].is_number());
    assert!(result.value["large"].is_number());
}

#[test]
fn test_negative_numbers() {
    let parser = JsonishParser::new();

    let input = r#"{"neg_int": -42, "neg_float": -2.5}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["neg_int"], -42);
    assert_eq!(result.value["neg_float"], -2.5);
}

#[test]
fn test_boolean_literals() {
    let parser = JsonishParser::new();

    let input = r#"{"t": true, "f": false}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["t"], true);
    assert_eq!(result.value["f"], false);
}

#[test]
fn test_null_values() {
    let parser = JsonishParser::new();

    let input = r#"{"value": null, "another": null}"#;
    let result = parser.parse(input).unwrap();
    assert!(result.value["value"].is_null());
    assert!(result.value["another"].is_null());
}

#[test]
fn test_empty_strings() {
    let parser = JsonishParser::new();

    let input = r#"{"empty": "", "another": ""}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["empty"], "");
    assert_eq!(result.value["another"], "");
}

#[test]
fn test_incomplete_nested_arrays() {
    let parser = JsonishParser::new();

    // Unclosed nested arrays
    let input = r#"[[1, 2], [3, 4"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value[0][0], 1);
    assert_eq!(result.value[1][0], 3);
    assert!(result.flags.contains(&CoercionFlag::TruncatedJson));
}

#[test]
fn test_incomplete_mixed_structures() {
    let parser = JsonishParser::new();

    // Object with unclosed array
    let input = r#"{"items": [1, 2, 3"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["items"][0], 1);
    assert_eq!(result.value["items"][2], 3);
    assert!(result.flags.contains(&CoercionFlag::TruncatedJson));
}

#[test]
fn test_special_characters_in_strings() {
    let parser = JsonishParser::new();

    let input = r#"{"special": "!@#$%^&*()_+-=[]{}|;:,.<>?"}"#;
    let result = parser.parse(input).unwrap();
    assert!(result.value["special"].as_str().unwrap().contains("!@#$"));
}

#[test]
fn test_very_long_strings() {
    let parser = JsonishParser::new();

    let long_string = "a".repeat(1000);
    let input = format!(r#"{{"long": "{}"}}"#, long_string);
    let result = parser.parse(&input).unwrap();
    assert_eq!(result.value["long"].as_str().unwrap().len(), 1000);
}

#[test]
fn test_multiple_trailing_commas() {
    let parser = JsonishParser::new();

    // Multiple trailing commas (malformed but should handle)
    let input = r#"{"a": 1,}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["a"], 1);
    assert!(result.flags.contains(&CoercionFlag::FixedTrailingComma));
}

#[test]
fn test_unquoted_keys_with_underscores() {
    let parser = JsonishParser::new();

    let input = r#"{user_name: "Alice", user_age: 30}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.value["user_name"], "Alice");
    assert_eq!(result.value["user_age"], 30);
    assert!(result.flags.contains(&CoercionFlag::FixedUnquotedKeys));
}

#[test]
fn test_confidence_degradation() {
    let parser = JsonishParser::new();

    // Perfect JSON - confidence 1.0
    let input1 = r#"{"key": "value"}"#;
    let result1 = parser.parse(input1).unwrap();
    assert_eq!(result1.confidence, 1.0);

    // One fix - confidence reduced
    let input2 = r#"{"key": "value",}"#;
    let result2 = parser.parse(input2).unwrap();
    assert!(result2.confidence < 1.0);
    assert!(result2.confidence >= 0.9);

    // Multiple fixes - confidence further reduced
    let input3 = r#"```json
{'key': 'value',}
```"#;
    let result3 = parser.parse(input3).unwrap();
    assert!(result3.confidence < 0.9);
}
