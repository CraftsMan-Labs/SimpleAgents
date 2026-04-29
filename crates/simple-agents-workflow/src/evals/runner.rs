use std::path::Path;

use futures::stream::{self, StreamExt};
use serde_json::json;
use serde_json::Value;

use crate::yaml_runner::{
    workflow_execution, YamlWorkflowExecutionFlags, YamlWorkflowExecutionRequest,
    YamlWorkflowRunOutput, YamlWorkflowSource,
};

use super::dataset::{load_dataset, load_eval_suite, resolve_relative_path, validate_suite};
use super::models::{
    EvalCaseResult, EvalComparisonConfig, EvalComparisonMode, EvalCustomEvalConfig,
    EvalDatasetRecord, EvalError, EvalErrorInfo, EvalKind, EvalReport, EvalResult, EvalRunStatus,
    EvalSuiteRunRequest, EvalSummary,
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
    let suite_id = suite.id.clone();

    let max_concurrency = suite.max_concurrency.max(1);
    let mut indexed_cases = stream::iter(records.iter().enumerate())
        .map(|(index, record)| {
            let workflow_path = &workflow_path;
            let options = &options;
            let comparison = &suite.comparison;
            let custom_evals = &suite.custom_evals;
            let suite_id = suite_id.as_str();
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
                    Ok(output) => {
                        let mut case = compare_record(record, output, comparison);
                        evaluate_custom_evals(
                            &mut case,
                            record,
                            custom_evals,
                            request.custom_worker,
                            suite_id,
                        )
                        .await;
                        case
                    }
                    Err(error) => EvalCaseResult {
                        case_id: record.id.clone(),
                        status: EvalRunStatus::Error,
                        first_failed_node: None,
                        first_failed_path: None,
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

    let deterministic = EvalResult {
        id: "deterministic".to_string(),
        kind: EvalKind::Deterministic,
        status,
        passed: status == EvalRunStatus::Passed,
        score: None,
        path: mismatch.as_ref().map(|mismatch| mismatch.path.clone()),
        node_id: first_failed_node.clone(),
        expected: mismatch
            .as_ref()
            .and_then(|mismatch| mismatch.expected.clone()),
        actual: mismatch
            .as_ref()
            .and_then(|mismatch| mismatch.actual.clone()),
        reason: mismatch
            .as_ref()
            .map(|mismatch| format!("first mismatch at {}", mismatch.path)),
        metadata: None,
    };

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
        evaluations: vec![deterministic],
        workflow_output: Some(workflow_output),
        error: None,
    }
}

async fn evaluate_custom_evals(
    case: &mut EvalCaseResult,
    record: &EvalDatasetRecord,
    custom_evals: &[EvalCustomEvalConfig],
    custom_worker: Option<&dyn crate::yaml_runner::YamlWorkflowCustomWorkerExecutor>,
    suite_id: &str,
) {
    if custom_evals.is_empty() {
        return;
    }
    let Some(workflow_output) = case.workflow_output.as_ref() else {
        return;
    };
    let actual_output = serde_json::to_value(workflow_output).unwrap_or(Value::Null);
    let record_value = serde_json::to_value(record).unwrap_or(Value::Null);

    for custom_eval in custom_evals {
        let result = match custom_worker {
            Some(custom_worker) => {
                run_custom_eval(
                    custom_worker,
                    custom_eval,
                    record,
                    &actual_output,
                    &record_value,
                    suite_id,
                )
                .await
            }
            None => EvalResult {
                id: custom_eval.id.clone(),
                kind: EvalKind::Custom,
                status: EvalRunStatus::Error,
                passed: false,
                score: None,
                path: Some(custom_eval.actual_path.clone()),
                node_id: node_id_from_output_path(&custom_eval.actual_path),
                expected: None,
                actual: None,
                reason: Some("custom eval requires a custom worker executor".to_string()),
                metadata: None,
            },
        };
        if result.status != EvalRunStatus::Passed && case.status == EvalRunStatus::Passed {
            case.status = result.status;
            case.first_failed_path = result.path.clone();
            case.first_failed_node = result.node_id.clone();
            case.expected = result.expected.clone();
            case.actual = result.actual.clone();
        }
        if result.status == EvalRunStatus::Error {
            case.error.get_or_insert_with(|| EvalErrorInfo {
                code: "custom_eval_failed".to_string(),
                message: result
                    .reason
                    .clone()
                    .unwrap_or_else(|| "custom eval failed".to_string()),
            });
        }
        case.evaluations.push(result);
    }
}

async fn run_custom_eval(
    custom_worker: &dyn crate::yaml_runner::YamlWorkflowCustomWorkerExecutor,
    custom_eval: &EvalCustomEvalConfig,
    record: &EvalDatasetRecord,
    actual_output: &Value,
    record_value: &Value,
    suite_id: &str,
) -> EvalResult {
    let actual = match select_path(actual_output, &custom_eval.actual_path) {
        Some(value) => value.clone(),
        None => {
            return EvalResult {
                id: custom_eval.id.clone(),
                kind: EvalKind::Custom,
                status: EvalRunStatus::Error,
                passed: false,
                score: None,
                path: Some(custom_eval.actual_path.clone()),
                node_id: node_id_from_output_path(&custom_eval.actual_path),
                expected: None,
                actual: None,
                reason: Some(format!(
                    "actual_path '{}' was not found",
                    custom_eval.actual_path
                )),
                metadata: None,
            };
        }
    };
    let expected = match select_path(record_value, &custom_eval.expected_path) {
        Some(value) => value.clone(),
        None => {
            return EvalResult {
                id: custom_eval.id.clone(),
                kind: EvalKind::Custom,
                status: EvalRunStatus::Error,
                passed: false,
                score: None,
                path: Some(custom_eval.expected_path.clone()),
                node_id: None,
                expected: None,
                actual: Some(actual),
                reason: Some(format!(
                    "expected_path '{}' was not found",
                    custom_eval.expected_path
                )),
                metadata: None,
            };
        }
    };
    let payload = json!({
        "actual": actual,
        "expected": expected,
        "threshold": custom_eval.threshold,
    });
    let context = json!({
        "case_id": record.id,
        "suite_id": suite_id,
        "eval_id": custom_eval.id,
        "metadata": record.metadata,
    });
    match custom_worker
        .execute(
            custom_eval.handler.as_str(),
            custom_eval.handler_file.as_deref(),
            &payload,
            &context,
        )
        .await
    {
        Ok(value) => custom_eval_result_from_value(custom_eval, payload, value),
        Err(message) => EvalResult {
            id: custom_eval.id.clone(),
            kind: EvalKind::Custom,
            status: EvalRunStatus::Error,
            passed: false,
            score: None,
            path: Some(custom_eval.actual_path.clone()),
            node_id: node_id_from_output_path(&custom_eval.actual_path),
            expected: payload.get("expected").cloned(),
            actual: payload.get("actual").cloned(),
            reason: Some(message),
            metadata: None,
        },
    }
}

fn custom_eval_result_from_value(
    custom_eval: &EvalCustomEvalConfig,
    payload: Value,
    value: Value,
) -> EvalResult {
    let score = value.get("score").and_then(Value::as_f64);
    let passed = value
        .get("passed")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            custom_eval
                .threshold
                .zip(score)
                .map(|(threshold, score)| score >= threshold)
                .unwrap_or(false)
        });
    EvalResult {
        id: custom_eval.id.clone(),
        kind: EvalKind::Custom,
        status: if passed {
            EvalRunStatus::Passed
        } else {
            EvalRunStatus::Failed
        },
        passed,
        score,
        path: Some(custom_eval.actual_path.clone()),
        node_id: node_id_from_output_path(&custom_eval.actual_path),
        expected: payload.get("expected").cloned(),
        actual: payload.get("actual").cloned(),
        reason: value
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        metadata: value.get("metadata").cloned(),
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

    use async_trait::async_trait;
    use serde_json::json;

    use crate::yaml_runner::YamlWorkflowCustomWorkerExecutor;

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
            rubric: None,
            custom: None,
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
            rubric: None,
            custom: None,
            metadata: None,
        };
        let output = output_with_node("classify", json!({"category": "finance"}));
        let result = compare_record(&record, output, &EvalComparisonConfig::default());

        assert_eq!(result.status, EvalRunStatus::Passed);
        assert!(result.first_failed_path.is_none());
    }

    #[test]
    fn custom_eval_handler_scores_mocked_rag_chunks() {
        let record = EvalDatasetRecord {
            id: "rag-case".to_string(),
            input: json!({"messages": []}),
            expected_output: json!({"terminal_node": "retrieve_chunks"}),
            rubric: None,
            custom: Some(json!({"expected_sources": ["doc-1", "doc-2"]})),
            metadata: None,
        };
        let output = output_with_node(
            "retrieve_chunks",
            json!([
                {"source_id": "doc-1"},
                {"source_id": "doc-2"},
                {"source_id": "noise"}
            ]),
        );
        let mut case = compare_record(&record, output, &EvalComparisonConfig::default());
        let custom_evals = vec![EvalCustomEvalConfig {
            id: "rag-accuracy".to_string(),
            handler: "evaluate_rag_chunks".to_string(),
            handler_file: Some("eval_handlers.py".to_string()),
            actual_path: "$.outputs.retrieve_chunks.output".to_string(),
            expected_path: "$.custom.expected_sources".to_string(),
            threshold: Some(0.8),
        }];
        let executor = MockRagEvalExecutor;

        futures::executor::block_on(evaluate_custom_evals(
            &mut case,
            &record,
            &custom_evals,
            Some(&executor),
            "suite",
        ));

        assert_eq!(case.status, EvalRunStatus::Passed);
        let rag_eval = case
            .evaluations
            .iter()
            .find(|evaluation| evaluation.id == "rag-accuracy")
            .expect("rag eval result");
        assert_eq!(rag_eval.kind, EvalKind::Custom);
        assert_eq!(rag_eval.score, Some(1.0));
        assert!(rag_eval.passed);
    }

    struct MockRagEvalExecutor;

    #[async_trait]
    impl YamlWorkflowCustomWorkerExecutor for MockRagEvalExecutor {
        async fn execute(
            &self,
            handler: &str,
            _handler_file: Option<&str>,
            payload: &Value,
            _context: &Value,
        ) -> Result<Value, String> {
            assert_eq!(handler, "evaluate_rag_chunks");
            let actual = payload["actual"].as_array().expect("actual chunks");
            let expected = payload["expected"].as_array().expect("expected sources");
            let actual_ids = actual
                .iter()
                .filter_map(|chunk| chunk.get("source_id"))
                .filter_map(Value::as_str)
                .collect::<std::collections::HashSet<_>>();
            let expected_ids = expected
                .iter()
                .filter_map(Value::as_str)
                .collect::<std::collections::HashSet<_>>();
            let matched = actual_ids.intersection(&expected_ids).count();
            let score = matched as f64 / expected_ids.len() as f64;
            Ok(json!({
                "score": score,
                "passed": score >= payload["threshold"].as_f64().unwrap_or(1.0),
                "reason": format!("{matched}/{} expected sources matched", expected_ids.len())
            }))
        }
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
