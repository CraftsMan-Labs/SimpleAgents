use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::yaml_runner::{
    YamlWorkflowCustomWorkerExecutor, YamlWorkflowExecutionFlags, YamlWorkflowExecutorBinding,
    YamlWorkflowRunOptions, YamlWorkflowRunOutput,
};

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("failed to read eval suite '{path}': {source}")]
    ReadSuite {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse eval suite '{path}': {source}")]
    ParseSuite {
        path: String,
        source: serde_yaml::Error,
    },
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
    #[error("invalid eval suite: {message}")]
    InvalidSuite { message: String },
    #[error("invalid eval dataset: {message}")]
    InvalidDataset { message: String },
}

pub struct EvalSuiteRunRequest<'a> {
    pub suite_path: &'a Path,
    pub executor: YamlWorkflowExecutorBinding<'a>,
    pub custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalSuite {
    pub id: String,
    pub workflow_path: PathBuf,
    pub dataset_path: PathBuf,
    #[serde(default)]
    pub execution: Option<YamlWorkflowExecutionFlags>,
    #[serde(default)]
    pub workflow_options: Option<YamlWorkflowRunOptions>,
    #[serde(default)]
    pub comparison: EvalComparisonConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalComparisonMode {
    Exact,
    Paths,
}

impl Default for EvalComparisonMode {
    fn default() -> Self {
        Self::Exact
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EvalComparisonConfig {
    #[serde(default)]
    pub mode: EvalComparisonMode,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalDatasetRecord {
    pub id: String,
    pub input: Value,
    pub expected_output: Value,
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
    pub first_failed_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_failed_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_output: Option<YamlWorkflowRunOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<EvalErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalErrorInfo {
    pub code: String,
    pub message: String,
}
