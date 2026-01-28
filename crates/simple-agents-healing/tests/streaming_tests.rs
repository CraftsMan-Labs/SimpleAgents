use simple_agents_healing::streaming::{PartialExtractor, StreamingParser};

#[test]
fn test_streaming_empty_object() {
    let mut parser = StreamingParser::new();
    parser.feed("{}");

    let result = parser.finalize().unwrap();
    assert!(result.value.is_object());
    assert_eq!(result.value.as_object().unwrap().len(), 0);
}

#[test]
fn test_streaming_empty_array() {
    let mut parser = StreamingParser::new();
    parser.feed("[]");

    let result = parser.finalize().unwrap();
    assert!(result.value.is_array());
    assert_eq!(result.value.as_array().unwrap().len(), 0);
}

#[test]
fn test_streaming_single_string() {
    let mut parser = StreamingParser::new();
    parser.feed(r#""hello world""#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value, "hello world");
}

#[test]
fn test_streaming_single_number() {
    let mut parser = StreamingParser::new();
    parser.feed("42");

    let result = parser.finalize().unwrap();
    assert_eq!(result.value, 42);
}

#[test]
fn test_streaming_boolean_true() {
    let mut parser = StreamingParser::new();
    parser.feed("true");

    let result = parser.finalize().unwrap();
    assert_eq!(result.value, true);
}

#[test]
fn test_streaming_boolean_false() {
    let mut parser = StreamingParser::new();
    parser.feed("false");

    let result = parser.finalize().unwrap();
    assert_eq!(result.value, false);
}

#[test]
fn test_streaming_null() {
    let mut parser = StreamingParser::new();
    parser.feed("null");

    let result = parser.finalize().unwrap();
    assert!(result.value.is_null());
}

#[test]
fn test_streaming_complex_object() {
    let mut parser = StreamingParser::new();

    parser.feed(r#"{"#);
    parser.feed(r#""id": 1,"#);
    parser.feed(r#""name": "Alice","#);
    parser.feed(r#""email": "alice@example.com","#);
    parser.feed(r#""age": 30,"#);
    parser.feed(r#""active": true,"#);
    parser.feed(r#""tags": ["rust", "python"],"#);
    parser.feed(r#""metadata": {"created": "2024-01-01"}"#);
    parser.feed(r#"}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["id"], 1);
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["email"], "alice@example.com");
    assert_eq!(result.value["age"], 30);
    assert_eq!(result.value["active"], true);
    assert_eq!(result.value["tags"][0], "rust");
    assert_eq!(result.value["tags"][1], "python");
    assert_eq!(result.value["metadata"]["created"], "2024-01-01");
}

#[test]
fn test_streaming_deeply_nested() {
    let mut parser = StreamingParser::new();

    // Don't split in the middle of a string value
    parser.feed(r#"{"a": {"b": {"c": {"d": "#);
    parser.feed(r#"{"e": "value"}}}}}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["a"]["b"]["c"]["d"]["e"], "value");
}

#[test]
fn test_streaming_large_array() {
    let mut parser = StreamingParser::new();

    parser.feed("[");
    for i in 0..100 {
        if i > 0 {
            parser.feed(",");
        }
        parser.feed(&format!(r#"{{"id": {}}}"#, i));
    }
    parser.feed("]");

    let result = parser.finalize().unwrap();
    assert!(result.value.is_array());
    let arr = result.value.as_array().unwrap();
    assert_eq!(arr.len(), 100);
    assert_eq!(arr[0]["id"], 0);
    assert_eq!(arr[99]["id"], 99);
}

#[test]
fn test_streaming_mixed_types_array() {
    let mut parser = StreamingParser::new();

    parser.feed(r#"[42, "hello", true, null, {"key": "value"}, [1, 2, 3]]"#);

    let result = parser.finalize().unwrap();
    let arr = result.value.as_array().unwrap();
    assert_eq!(arr[0], 42);
    assert_eq!(arr[1], "hello");
    assert_eq!(arr[2], true);
    assert!(arr[3].is_null());
    assert_eq!(arr[4]["key"], "value");
    assert_eq!(arr[5][0], 1);
}

#[test]
fn test_streaming_with_whitespace() {
    let mut parser = StreamingParser::new();

    parser.feed("  {  \n");
    parser.feed(r#"  "name"  :  "#);
    parser.feed(r#"  "Alice"  ,  "#);
    parser.feed(r#"  "age"  :  30  "#);
    parser.feed("  }  ");

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["age"], 30);
}

#[test]
fn test_streaming_unicode() {
    let mut parser = StreamingParser::new();

    // Don't split in the middle of a string value
    parser.feed(r#"{"greeting": "你好", "#);
    parser.feed(r#""emoji": "🦀"}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["greeting"], "你好");
    assert_eq!(result.value["emoji"], "🦀");
}

#[test]
fn test_streaming_escaped_characters() {
    let mut parser = StreamingParser::new();

    // Don't split in the middle of a string value
    parser.feed(r#"{"text": "Line 1\nLine 2\tTab", "#);
    parser.feed(r#""quote": "She said \"Hello\""}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["text"], "Line 1\nLine 2\tTab");
    assert_eq!(result.value["quote"], r#"She said "Hello""#);
}

#[test]
fn test_streaming_scientific_notation() {
    let mut parser = StreamingParser::new();

    parser.feed(r#"{"small": 1.23e-10, "#);
    parser.feed(r#""large": 4.56e20}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["small"], 1.23e-10);
    assert_eq!(result.value["large"], 4.56e20);
}

#[test]
fn test_streaming_negative_numbers() {
    let mut parser = StreamingParser::new();

    parser.feed(r#"{"int": -42, "float": -2.5}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["int"], -42);
    assert_eq!(result.value["float"], -2.5);
}

#[test]
fn test_partial_extractor_progressive() {
    let mut extractor = PartialExtractor::new();

    // Simulate progressive JSON building
    extractor.feed(r#"{"#);
    // First chunk - incomplete

    extractor.feed(r#""name": "Alice""#);
    // Second chunk - still incomplete

    extractor.feed(r#"}"#);
    // Final chunk - complete
    let result = extractor.finalize().unwrap();
    assert_eq!(result["name"], "Alice");
}

#[test]
fn test_streaming_recover_from_reset() {
    let mut parser = StreamingParser::new();

    parser.feed(r#"{"name": "Alice"}"#);
    let result1 = parser.try_parse().unwrap();
    assert_eq!(result1.value["name"], "Alice");

    // Clear and parse new data
    parser.clear();
    parser.feed(r#"{"id": 42}"#);
    let result2 = parser.try_parse().unwrap();
    assert_eq!(result2.value["id"], 42);
}

#[test]
fn test_streaming_multiple_objects_sequential() {
    // Parse first object
    let mut parser1 = StreamingParser::new();
    parser1.feed(r#"{"a": 1}"#);
    let result1 = parser1.finalize().unwrap();
    assert_eq!(result1.value["a"], 1);

    // Parse second object
    let mut parser2 = StreamingParser::new();
    parser2.feed(r#"{"b": 2}"#);
    let result2 = parser2.finalize().unwrap();
    assert_eq!(result2.value["b"], 2);
}

#[test]
fn test_streaming_very_long_string() {
    let mut parser = StreamingParser::new();
    let long_string = "x".repeat(10000);

    parser.feed(r#"{"data": ""#);
    parser.feed(&long_string);
    parser.feed(r#""}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["data"], long_string);
}

#[test]
fn test_streaming_concurrent_feeds() {
    // Test that we can handle rapid successive feeds
    let mut parser = StreamingParser::new();

    let chunks = vec![
        r#"{"items": ["#,
        r#"{"id": 1},"#,
        r#"{"id": 2},"#,
        r#"{"id": 3}"#,
        r#"]}"#,
    ];

    for chunk in chunks {
        parser.feed(chunk);
    }

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["items"][0]["id"], 1);
    assert_eq!(result.value["items"][1]["id"], 2);
    assert_eq!(result.value["items"][2]["id"], 3);
}

#[test]
fn test_streaming_with_comments_via_healing() {
    let mut parser = StreamingParser::new();

    // JSON with comments (should be stripped by healing parser)
    parser.feed(r#"{"#);
    parser.feed(
        r#""name": "Alice", // This is a name
"#,
    );
    parser.feed(r#""age": 30}"#);

    let result = parser.finalize().unwrap();
    assert_eq!(result.value["name"], "Alice");
    assert_eq!(result.value["age"], 30);
}
