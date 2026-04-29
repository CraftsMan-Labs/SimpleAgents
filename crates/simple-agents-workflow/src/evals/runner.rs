use std::path::Path;

use futures::stream::{self, StreamExt};
use serde_json::Value;

use crate::yaml_runner::{
    workflow_execution, YamlWorkflowExecutionFlags, YamlWorkflowExecutionRequest,
    YamlWorkflowRunOutput, YamlWorkflowSource,
};

use super::dataset::{load_dataset, load_eval_suite, resolve_relative_path, validate_suite};
use super::models::{
    EvalCaseResult, EvalComparisonConfig, EvalComparisonMode, EvalDatasetRecord, EvalError,
    EvalErrorInfo, EvalReport, EvalRunStatus, EvalSuiteRunRequest, EvalSummary,
};

pub async fn run_eval_suite(request: EvalSuiteRunRequest<'_>) -> Result<EvalReport, EvalError> {
    let suite = load_eval_suite(request.suite_path)?;
    validate_suite(&suite)?;

    let suite_dir = request
        .suite_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let workflow_path = resolve_relative_path(suite_dir, suite.workflow_path.as_path());
    let dataset_path = resolve_relative_path(suite_dir, suite.dataset_path.as_path());
    let records = load_dataset(dataset_path.as_path())?;
    let options = suite.workflow_options.clone().unwrap_or_default();
    let flags = suite
        .execution
        .unwrap_or_else(YamlWorkflowExecutionFlags::default);

    let max_concurrency = suite.max_concurrency.max(1);
    let mut indexed_cases = stream::iter(records.iter().enumerate())
        .map(|(index, record)| {
            let workflow_path = &workflow_path;
            let options = &options;
            let comparison = &suite.comparison;
            async move {
            let execution_request = YamlWorkflowExecutionRequest {
                source: YamlWorkflowSource::File(workflow_path.as_path()),
                workflow_input: &record.input,
                executor: request.executor,
                custom_worker: request.custom_worker,
                options,
                flags,
            };

            let case = match workflow_execution::run(execution_request).await {
                Ok(output) => compare_record(record, output, comparison),
                Err(error) => EvalCaseResult {
                    case_id: record.id.clone(),
                    status: EvalRunStatus::Error,
                    first_failed_node: None,
                    first_failed_path: None,
                    expected: None,
                    actual: None,
                    workflow_output: None,
                    error: Some(EvalErrorInfo {
                        code: "workflow_run_failed".to_string(),
                        message: error.to_string(),
                    }),
                },
            };
            (index, case)
            }
        })
        .buffered(max_concurrency)
        .collect::<Vec<_>>()
        .await;
    indexed_cases.sort_by_key(|(index, _)| *index);
    let cases = indexed_cases
        .into_iter()
        .map(|(_, case)| case)
        .collect::<Vec<_>>();

    Ok(build_report(suite.id, cases))
}

fn compare_record(
    record: &EvalDatasetRecord,
    workflow_output: YamlWorkflowRunOutput,
    comparison: &EvalComparisonConfig,
) -> EvalCaseResult {
    let actual_output = serde_json::to_value(&workflow_output).unwrap_or(Value::Null);
    let mismatch = match comparison.mode {
        EvalComparisonMode::Exact => first_mismatch(&record.expected_output, &actual_output, "$"),
        EvalComparisonMode::Paths => comparison.paths.iter().find_map(|path| {
            let expected = select_path(&record.expected_output, path);
            let actual = select_path(&actual_output, path);
            match (expected, actual) {
                (Some(expected), Some(actual)) if expected == actual => None,
                _ => Some(Mismatch {
                    path: path.clone(),
                    expected: expected.cloned(),
                    actual: actual.cloned(),
                }),
            }
        }),
    };

    let status = if mismatch.is_some() {
        EvalRunStatus::Failed
    } else {
        EvalRunStatus::Passed
    };
    let first_failed_node = mismatch
        .as_ref()
        .and_then(|mismatch| node_id_from_output_path(&mismatch.path));

    EvalCaseResult {
        case_id: record.id.clone(),
        status,
        first_failed_node,
        first_failed_path: mismatch.as_ref().map(|mismatch| mismatch.path.clone()),
        expected: mismatch
            .as_ref()
            .and_then(|mismatch| mismatch.expected.clone()),
        actual: mismatch
            .as_ref()
            .and_then(|mismatch| mismatch.actual.clone()),
        workflow_output: Some(workflow_output),
        error: None,
    }
}

fn build_report(suite_id: String, cases: Vec<EvalCaseResult>) -> EvalReport {
    let total_cases = cases.len();
    let passed_cases = cases
        .iter()
        .filter(|case| case.status == EvalRunStatus::Passed)
        .count();
    let failed_cases = cases
        .iter()
        .filter(|case| case.status == EvalRunStatus::Failed)
        .count();
    let error_cases = cases
        .iter()
        .filter(|case| case.status == EvalRunStatus::Error)
        .count();
    let status = if error_cases > 0 {
        EvalRunStatus::Error
    } else if failed_cases > 0 {
        EvalRunStatus::Failed
    } else {
        EvalRunStatus::Passed
    };

    EvalReport {
        suite_id,
        status,
        summary: EvalSummary {
            total_cases,
            passed_cases,
            failed_cases,
            error_cases,
            pass_rate: if total_cases == 0 {
                0.0
            } else {
                passed_cases as f64 / total_cases as f64
            },
        },
        cases,
    }
}

#[derive(Debug, Clone)]
struct Mismatch {
    path: String,
    expected: Option<Value>,
    actual: Option<Value>,
}

fn first_mismatch(expected: &Value, actual: &Value, path: &str) -> Option<Mismatch> {
    if expected == actual {
        return None;
    }

    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, expected_value) in expected {
                let child_path = format!("{}.{}", path, key);
                match actual.get(key) {
                    Some(actual_value) => {
                        if let Some(mismatch) =
                            first_mismatch(expected_value, actual_value, &child_path)
                        {
                            return Some(mismatch);
                        }
                    }
                    None => {
                        return Some(Mismatch {
                            path: child_path,
                            expected: Some(expected_value.clone()),
                            actual: None,
                        });
                    }
                }
            }
            None
        }
        (Value::Array(expected), Value::Array(actual)) => {
            for (index, expected_value) in expected.iter().enumerate() {
                let child_path = format!("{}[{}]", path, index);
                match actual.get(index) {
                    Some(actual_value) => {
                        if let Some(mismatch) =
                            first_mismatch(expected_value, actual_value, &child_path)
                        {
                            return Some(mismatch);
                        }
                    }
                    None => {
                        return Some(Mismatch {
                            path: child_path,
                            expected: Some(expected_value.clone()),
                            actual: None,
                        });
                    }
                }
            }
            None
        }
        _ => Some(Mismatch {
            path: path.to_string(),
            expected: Some(expected.clone()),
            actual: Some(actual.clone()),
        }),
    }
}

fn select_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path == "$" {
        return Some(root);
    }
    let path = path.strip_prefix("$.").or_else(|| path.strip_prefix('$'))?;
    let mut current = root;
    for segment in parse_path_segments(path)? {
        current = match segment {
            PathSegment::Key(key) => current.get(key.as_str())?,
            PathSegment::Index(index) => current.get(index)?,
        };
    }
    Some(current)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathSegment {
    Key(String),
    Index(usize),
}

fn parse_path_segments(path: &str) -> Option<Vec<PathSegment>> {
    let mut segments = Vec::new();
    let mut key = String::new();
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !key.is_empty() {
                    segments.push(PathSegment::Key(std::mem::take(&mut key)));
                }
            }
            '[' => {
                if !key.is_empty() {
                    segments.push(PathSegment::Key(std::mem::take(&mut key)));
                }
                let mut index = String::new();
                for inner in chars.by_ref() {
                    if inner == ']' {
                        break;
                    }
                    index.push(inner);
                }
                segments.push(PathSegment::Index(index.parse().ok()?));
            }
            _ => key.push(ch),
        }
    }
    if !key.is_empty() {
        segments.push(PathSegment::Key(key));
    }
    Some(segments)
}

fn node_id_from_output_path(path: &str) -> Option<String> {
    let suffix = path.strip_prefix("$.outputs.")?;
    let node_id = suffix.split('.').next()?;
    if node_id.is_empty() {
        None
    } else {
        Some(node_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    #[test]
    fn output_shaped_dataset_localizes_node_mismatch() {
        let record = EvalDatasetRecord {
            id: "case".to_string(),
            input: json!({"messages": []}),
            expected_output: json!({
                "outputs": {
                    "classify": {
                        "output": { "category": "finance" }
                    }
                }
            }),
            metadata: None,
        };
        let output = output_with_node("classify", json!({"category": "education"}));
        let result = compare_record(&record, output, &EvalComparisonConfig::default());

        assert_eq!(result.status, EvalRunStatus::Failed);
        assert_eq!(result.first_failed_node.as_deref(), Some("classify"));
        assert_eq!(
            result.first_failed_path.as_deref(),
            Some("$.outputs.classify.output.category")
        );
    }

    #[test]
    fn exact_mode_treats_expected_output_as_subset() {
        let record = EvalDatasetRecord {
            id: "case".to_string(),
            input: json!({"messages": []}),
            expected_output: json!({
                "terminal_node": "classify",
                "outputs": {
                    "classify": {
                        "output": { "category": "finance" }
                    }
                }
            }),
            metadata: None,
        };
        let output = output_with_node("classify", json!({"category": "finance"}));
        let result = compare_record(&record, output, &EvalComparisonConfig::default());

        assert_eq!(result.status, EvalRunStatus::Passed);
        assert!(result.first_failed_path.is_none());
    }

    fn output_with_node(node_id: &str, payload: Value) -> YamlWorkflowRunOutput {
        let mut outputs = BTreeMap::new();
        outputs.insert(node_id.to_string(), json!({ "output": payload.clone() }));
        YamlWorkflowRunOutput {
            workflow_id: "wf".to_string(),
            entry_node: node_id.to_string(),
            trace: vec![node_id.to_string()],
            outputs,
            terminal_node: node_id.to_string(),
            terminal_output: Some(payload),
            step_timings: Vec::new(),
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 0,
            ttft_ms: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            total_reasoning_tokens: None,
            tokens_per_second: 0.0,
            trace_id: None,
            metadata: None,
        }
    }
}
