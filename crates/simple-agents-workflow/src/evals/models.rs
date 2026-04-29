use std::path::Path;

use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::yaml_runner::{
    YamlWorkflowCustomWorkerExecutor, YamlWorkflowExecutionFlags, YamlWorkflowExecutorBinding,
    YamlWorkflowRunOptions, YamlWorkflowRunOutput,
};

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("failed to read eval dataset '{path}': {source}")]
    ReadDataset {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse eval dataset '{path}' line {line}: {source}")]
    ParseDatasetLine {
        path: String,
        line: usize,
        source: serde_json::Error,
    },
    #[error("invalid eval dataset: {message}")]
    InvalidDataset { message: String },
}

impl Serialize for EvalError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("EvalError", 3)?;
        let (code, path) = match self {
            EvalError::ReadDataset { path, .. } => ("read_dataset_failed", Some(path)),
            EvalError::ParseDatasetLine { path, .. } => ("parse_dataset_line_failed", Some(path)),
            EvalError::InvalidDataset { .. } => ("invalid_dataset", None),
        };
        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        if let Some(path) = path {
            state.serialize_field("path", path)?;
        }
        state.end()
    }
}

pub struct EvalSuiteRunRequest<'a> {
    pub suite_id: Option<&'a str>,
    pub workflow_path: &'a Path,
    pub dataset_path: &'a Path,
    pub executor: YamlWorkflowExecutorBinding<'a>,
    pub custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    pub execution: YamlWorkflowExecutionFlags,
    pub workflow_options: YamlWorkflowRunOptions,
    pub max_concurrency: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalDatasetRecord {
    pub id: String,
    pub input: Value,
    pub expected_output: Value,
    #[serde(default)]
    pub rubric: Option<Value>,
    #[serde(default)]
    pub custom: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalRunStatus {
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalReport {
    pub suite_id: String,
    pub status: EvalRunStatus,
    pub summary: EvalSummary,
    pub cases: Vec<EvalCaseResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalSummary {
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub error_cases: usize,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalCaseResult {
    pub case_id: String,
    pub status: EvalRunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evaluations: Vec<EvalResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_output: Option<YamlWorkflowRunOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<EvalErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalResult {
    pub id: String,
    pub status: EvalRunStatus,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalErrorInfo {
    pub code: String,
    pub message: String,
}
