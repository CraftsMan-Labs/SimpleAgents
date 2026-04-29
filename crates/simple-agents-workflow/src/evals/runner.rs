use futures::stream::{self, StreamExt};
use serde_json::Value;

use crate::yaml_runner::{workflow_execution, YamlWorkflowExecutionRequest, YamlWorkflowSource};

use super::dataset::load_dataset;
use super::models::{
    EvalCaseResult, EvalDatasetRecord, EvalError, EvalErrorInfo, EvalReport, EvalResult,
    EvalRunStatus, EvalSuiteRunRequest, EvalSummary,
};

/// Minimal native eval runner.
///
/// Language bindings expose the primary callback-based API. This native runner stays intentionally
/// small: it runs each golden record and performs a subset comparison against `expected_output`.
pub async fn run_eval_suite(request: EvalSuiteRunRequest<'_>) -> Result<EvalReport, EvalError> {
    let records = load_dataset(request.dataset_path)?;
    let suite_id = request.suite_id.unwrap_or_else(|| {
        request
            .dataset_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("eval-suite")
    });
    let max_concurrency = request.max_concurrency.max(1);

    let mut indexed_cases = stream::iter(records.iter().enumerate())
        .map(|(index, record)| {
            let workflow_path = request.workflow_path;
            let options = &request.workflow_options;
            async move {
                let execution_request = YamlWorkflowExecutionRequest {
                    source: YamlWorkflowSource::File(workflow_path),
                    workflow_input: &record.input,
                    executor: request.executor,
                    custom_worker: request.custom_worker,
                    options,
                    flags: request.execution,
                };

                let case = match workflow_execution::run(execution_request).await {
                    Ok(output) => compare_record(record, output),
                    Err(error) => EvalCaseResult {
                        case_id: record.id.clone(),
                        status: EvalRunStatus::Error,
                        expected: None,
                        actual: None,
                        evaluations: Vec::new(),
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

    Ok(build_report(suite_id.to_string(), cases))
}

fn compare_record(
    record: &EvalDatasetRecord,
    workflow_output: crate::yaml_runner::YamlWorkflowRunOutput,
) -> EvalCaseResult {
    let actual_output = serde_json::to_value(&workflow_output).unwrap_or(Value::Null);
    let mismatch = first_mismatch(&record.expected_output, &actual_output, "$");
    let status = if mismatch.is_some() {
        EvalRunStatus::Failed
    } else {
        EvalRunStatus::Passed
    };
    let reason = mismatch
        .as_ref()
        .map(|mismatch| format!("first mismatch at {}", mismatch.path));

    let deterministic = EvalResult {
        id: "subset".to_string(),
        status,
        passed: status == EvalRunStatus::Passed,
        score: None,
        expected: mismatch
            .as_ref()
            .and_then(|mismatch| mismatch.expected.clone()),
        actual: mismatch.as_ref().and_then(|mismatch| mismatch.actual.clone()),
        reason,
        metadata: None,
    };

    EvalCaseResult {
        case_id: record.id.clone(),
        status,
        expected: deterministic.expected.clone(),
        actual: deterministic.actual.clone(),
        evaluations: vec![deterministic],
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    #[test]
    fn subset_comparison_accepts_stable_golden_fields() {
        let record = EvalDatasetRecord {
            id: "case".to_string(),
            input: json!({"messages": [{"role": "user", "content": "hi"}]}),
            expected_output: json!({
                "terminal_node": "classify",
                "outputs": {
                    "classify": {
                        "output": { "category": "finance" }
                    }
                }
            }),
            rubric: None,
            custom: None,
            metadata: None,
        };
        let output = output_with_node("classify", json!({"category": "finance", "reason": "ok"}));
        let result = compare_record(&record, output);

        assert_eq!(result.status, EvalRunStatus::Passed);
    }

    #[test]
    fn subset_comparison_reports_first_mismatch() {
        let record = EvalDatasetRecord {
            id: "case".to_string(),
            input: json!({"messages": [{"role": "user", "content": "hi"}]}),
            expected_output: json!({
                "outputs": {
                    "classify": {
                        "output": { "category": "finance" }
                    }
                }
            }),
            rubric: None,
            custom: None,
            metadata: None,
        };
        let output = output_with_node("classify", json!({"category": "education"}));
        let result = compare_record(&record, output);

        assert_eq!(result.status, EvalRunStatus::Failed);
        assert_eq!(
            result.evaluations[0].reason.as_deref(),
            Some("first mismatch at $.outputs.classify.output.category")
        );
    }

    fn output_with_node(
        node_id: &str,
        payload: Value,
    ) -> crate::yaml_runner::YamlWorkflowRunOutput {
        let mut outputs = BTreeMap::new();
        outputs.insert(node_id.to_string(), json!({ "output": payload.clone() }));
        crate::yaml_runner::YamlWorkflowRunOutput {
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
