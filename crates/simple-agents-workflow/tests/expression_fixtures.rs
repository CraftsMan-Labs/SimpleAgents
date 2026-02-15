use serde::Deserialize;
use serde_json::Value;
use simple_agents_workflow::evaluate_bool;

#[derive(Debug, Deserialize)]
struct FixtureCase {
    name: String,
    expression: String,
    scoped_input: Value,
    expected: Option<bool>,
    expect_error: Option<String>,
}

#[test]
fn expression_fixtures_match_expected_results() {
    let fixture_text = std::fs::read_to_string("tests/fixtures/expression_cases.json")
        .expect("fixture file should be readable");
    let fixtures: Vec<FixtureCase> =
        serde_json::from_str(&fixture_text).expect("fixture json should parse");

    for case in fixtures {
        let result = evaluate_bool(&case.expression, &case.scoped_input);
        match (case.expected, case.expect_error.as_deref()) {
            (Some(expected), None) => {
                let actual = result.unwrap_or_else(|error| {
                    panic!("fixture '{}' should pass, got error: {}", case.name, error)
                });
                assert_eq!(
                    actual, expected,
                    "fixture '{}' produced unexpected value",
                    case.name
                );
            }
            (None, Some(expected_error)) => {
                let error = result.expect_err("fixture should fail").to_string();
                assert!(
                    error.contains(expected_error),
                    "fixture '{}' expected error containing '{}', got '{}'",
                    case.name,
                    expected_error,
                    error
                );
            }
            _ => panic!(
                "fixture '{}' must define either expected or expect_error",
                case.name
            ),
        }
    }
}
