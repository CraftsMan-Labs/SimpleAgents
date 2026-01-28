//! Benchmarks for JSON healing parser performance.
//!
//! Run with:
//! ```bash
//! cargo bench --bench parser_benchmarks
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use simple_agents_healing::parser::JsonishParser;
use simple_agents_healing::streaming::StreamingParser;

/// Benchmark standard JSON parsing (fast path)
fn bench_parse_standard_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_standard_json");

    // Create large test case separately to avoid borrowing temporary
    let large_json = format!(
        r#"{{
            "users": [{}]
        }}"#,
        (0..100)
            .map(|i| format!(
                r#"{{"id": {}, "name": "User {}", "email": "user{}@example.com"}}"#,
                i, i, i
            ))
            .collect::<Vec<_>>()
            .join(",")
    );

    let test_cases: Vec<(&str, &str)> = vec![
        (
            "small",
            r#"{"name": "Alice", "age": 30, "email": "alice@example.com"}"#,
        ),
        (
            "medium",
            r#"{
                "name": "Alice",
                "age": 30,
                "email": "alice@example.com",
                "address": {
                    "street": "123 Main St",
                    "city": "Springfield",
                    "zip": "12345"
                },
                "hobbies": ["reading", "gaming", "cooking"]
            }"#,
        ),
        ("large", &large_json),
    ];

    for (name, json) in test_cases {
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

/// Benchmark parsing with markdown stripping
fn bench_parse_markdown_wrapped(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_markdown_wrapped");

    let test_cases = vec![
        (
            "small",
            r#"```json
            {"name": "Alice", "age": 30}
            ```"#,
        ),
        (
            "medium",
            r#"```json
            {
                "name": "Alice",
                "age": 30,
                "hobbies": ["reading", "gaming"],
                "address": {"city": "Springfield"}
            }
            ```"#,
        ),
        (
            "with_language_tag",
            r#"```json
            {"key": "value", "number": 42}
            ```"#,
        ),
    ];

    for (name, json) in test_cases {
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

/// Benchmark parsing with trailing comma fixes
fn bench_parse_trailing_commas(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_trailing_commas");

    let test_cases = vec![
        ("object_trailing", r#"{"name": "Alice", "age": 30,}"#),
        ("array_trailing", r#"{"hobbies": ["reading", "gaming",]}"#),
        (
            "nested_trailing",
            r#"{
                "user": {"name": "Alice", "age": 30,},
                "hobbies": ["reading", "gaming",],
            }"#,
        ),
    ];

    for (name, json) in test_cases {
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

/// Benchmark lenient parsing with incomplete JSON
fn bench_parse_incomplete_json(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_incomplete_json");

    let test_cases = vec![
        ("unclosed_string", r#"{"name": "Alice"#),
        ("unclosed_object", r#"{"name": "Alice", "age": 30"#),
        ("unclosed_array", r#"{"hobbies": ["reading", "gaming""#),
        (
            "unclosed_nested",
            r#"{"user": {"name": "Alice", "address": {"city": "Springfield""#,
        ),
    ];

    for (name, json) in test_cases {
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

/// Benchmark streaming parser incremental feeding
fn bench_streaming_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_parser");

    // Simulate streaming chunks
    let chunks = vec![
        r#"{"name": "Alice", "#,
        r#""age": 30, "#,
        r#""email": "alice@example.com", "#,
        r#""hobbies": ["reading", "#,
        r#""gaming", "cooking"]}"#,
    ];

    group.throughput(Throughput::Bytes(
        chunks.iter().map(|s| s.len()).sum::<usize>() as u64,
    ));

    group.bench_function("incremental_feed", |b| {
        b.iter(|| {
            let mut parser = StreamingParser::new();
            for chunk in &chunks {
                parser.feed(black_box(chunk));
            }
            parser.finalize()
        });
    });

    group.finish();
}

/// Benchmark parsing with comments
fn bench_parse_with_comments(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_with_comments");

    let test_cases = vec![
        (
            "line_comments",
            r#"{
                // User information
                "name": "Alice",
                "age": 30 // Age in years
            }"#,
        ),
        (
            "block_comments",
            r#"{
                /* User profile */
                "name": "Alice",
                /* Age field */ "age": 30
            }"#,
        ),
        (
            "mixed_comments",
            r#"{
                // Basic info
                "name": "Alice",
                /* Age */ "age": 30,
                // Hobbies list
                "hobbies": ["reading", "gaming"] /* End of hobbies */
            }"#,
        ),
    ];

    for (name, json) in test_cases {
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

/// Benchmark parsing different string delimiters
fn bench_parse_string_delimiters(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_string_delimiters");

    let test_cases = vec![
        ("double_quotes", r#"{"name": "Alice", "city": "NYC"}"#),
        ("single_quotes", r#"{'name': 'Alice', 'city': 'NYC'}"#),
        (
            "triple_quotes",
            r#"{"description": """A long
            multi-line
            description"""}"#,
        ),
        ("backticks", r#"{"code": `const x = 42;`, "lang": "js"}"#),
    ];

    for (name, json) in test_cases {
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

/// Benchmark parsing deeply nested structures
fn bench_parse_deep_nesting(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_deep_nesting");

    for depth in [5, 10, 20, 50].iter() {
        let nested_start = "{\"nested\": ".repeat(*depth);
        let nested_end = "}".repeat(*depth);
        let json = format!(r#"{}42{}"#, nested_start, nested_end);

        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(depth), &json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

/// Benchmark parsing large arrays
fn bench_parse_large_arrays(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_large_arrays");

    for size in [10, 50, 100, 500].iter() {
        let items = (0..*size)
            .map(|i| format!(r#"{{"id": {}, "value": "item_{}}}"#, i, i))
            .collect::<Vec<_>>();
        let json = format!(r#"{{"items": [{}]}}"#, items.join(","));

        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &json, |b, json| {
            let parser = JsonishParser::new();
            b.iter(|| parser.parse(black_box(json)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_standard_json,
    bench_parse_markdown_wrapped,
    bench_parse_trailing_commas,
    bench_parse_incomplete_json,
    bench_streaming_parser,
    bench_parse_with_comments,
    bench_parse_string_delimiters,
    bench_parse_deep_nesting,
    bench_parse_large_arrays,
);
criterion_main!(benches);
