use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{YamlHumanInputOption, YamlHumanInputType, YamlWorkflow, YamlWorkflowRunError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlStepTiming {
    pub node_id: String,
    pub node_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    pub elapsed_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlLlmNodeMetrics {
    pub elapsed_ms: u128,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    pub tokens_per_second: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YamlWorkflowRunStatus {
    #[default]
    Completed,
    AwaitingHumanInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanRequest {
    pub node_id: String,
    pub input_type: YamlHumanInputType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<YamlHumanInputOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlWorkflowRunOutput {
    pub workflow_id: String,
    pub entry_node: String,
    pub trace: Vec<String>,
    pub outputs: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub globals: BTreeMap<String, Value>,
    pub terminal_node: String,
    pub terminal_output: Option<Value>,
    #[serde(default)]
    pub status: YamlWorkflowRunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_request: Option<HumanRequest>,
    pub step_timings: Vec<YamlStepTiming>,
    pub llm_node_metrics: BTreeMap<String, YamlLlmNodeMetrics>,
    pub llm_node_models: BTreeMap<String, String>,
    pub total_elapsed_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u128>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_reasoning_tokens: Option<u64>,
    pub tokens_per_second: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YamlWorkflowPayloadMode {
    #[default]
    FullPayload,
    RedactedPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YamlToolTraceMode {
    #[default]
    Full,
    Redacted,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct YamlWorkflowTraceContextInput {
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub span_id: Option<String>,
    #[serde(default)]
    pub parent_span_id: Option<String>,
    #[serde(default)]
    pub traceparent: Option<String>,
    #[serde(default)]
    pub tracestate: Option<String>,
    #[serde(default)]
    pub baggage: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct YamlWorkflowTraceTenantContext {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlWorkflowTelemetryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub nerdstats: bool,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f32,
    #[serde(default)]
    pub payload_mode: YamlWorkflowPayloadMode,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_true")]
    pub multi_tenant: bool,
    #[serde(default)]
    pub tool_trace_mode: YamlToolTraceMode,
}

impl Default for YamlWorkflowTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            nerdstats: true,
            sample_rate: 1.0,
            payload_mode: YamlWorkflowPayloadMode::FullPayload,
            retention_days: 30,
            multi_tenant: true,
            tool_trace_mode: YamlToolTraceMode::Full,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct YamlWorkflowTraceOptions {
    #[serde(default)]
    pub context: Option<YamlWorkflowTraceContextInput>,
    #[serde(default)]
    pub tenant: YamlWorkflowTraceTenantContext,
}

/// Global execution toggles for a workflow run (orthogonal to per-node YAML `heal` / `stream`).
///
/// JSON uses snake_case keys: `healing`, `workflow_streaming`, `node_llm_streaming`,
/// `split_stream_deltas`, `debug_stream_parse`. Missing keys deserialize using [`Default`] (see
/// [`Default::default`] on this type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct YamlWorkflowExecutionFlags {
    /// When true, enables the JSON healing path for structured LLM outputs in addition to any
    /// per-node `heal` setting in YAML.
    pub healing: bool,
    /// When false and an event sink is present, token delta events are not forwarded to the sink
    /// (workflow lifecycle and completion events still flow).
    pub workflow_streaming: bool,
    /// When false, LLM nodes never use provider streaming, regardless of YAML `stream`.
    pub node_llm_streaming: bool,
    /// When true, emit separate stream events for thinking vs output (`node_stream_thinking_delta`,
    /// `node_stream_output_delta`) in addition to `node_stream_delta`.
    pub split_stream_deltas: bool,
    /// When true (or when env `SIMPLE_AGENTS_DEBUG_STREAM_PARSE` is `1`/`true`/`yes`), append the
    /// partial streamed LLM text to structured JSON parse/coerce errors for debugging.
    pub debug_stream_parse: bool,
}

impl Default for YamlWorkflowExecutionFlags {
    /// Matches legacy behavior: YAML controls streaming/healing unless callers override flags.
    fn default() -> Self {
        Self {
            healing: false,
            workflow_streaming: false,
            node_llm_streaming: true,
            split_stream_deltas: false,
            debug_stream_parse: false,
        }
    }
}

/// Workflow document location for [`YamlWorkflowExecutionRequest`].
#[derive(Debug, Clone, Copy)]
pub enum YamlWorkflowSource<'a> {
    File(&'a Path),
    Inline(&'a YamlWorkflow),
}

/// LLM backend for [`YamlWorkflowExecutionRequest`].
#[derive(Clone, Copy)]
pub enum YamlWorkflowExecutorBinding<'a> {
    Llm(&'a dyn super::YamlWorkflowLlmExecutor),
    Client(&'a simple_agents_core::SimpleAgentsClient),
}

/// Canonical typed input for workflow execution (handler names remain YAML-only on `custom_worker` nodes).
#[derive(Clone, Copy)]
pub struct YamlWorkflowExecutionRequest<'a> {
    pub source: YamlWorkflowSource<'a>,
    pub workflow_input: &'a Value,
    pub executor: YamlWorkflowExecutorBinding<'a>,
    pub custom_worker: Option<&'a dyn super::YamlWorkflowCustomWorkerExecutor>,
    pub resume: Option<&'a YamlWorkflowRunOutput>,
    pub human_response: Option<&'a Value>,
    pub options: &'a YamlWorkflowRunOptions,
    pub flags: YamlWorkflowExecutionFlags,
}

/// Which public entrypoint is validating the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YamlWorkflowExecutionSurface {
    /// `run` / `run_async`: no workflow event sink; `workflow_streaming` must be false.
    Run,
    /// `stream`: requires an event sink; `workflow_streaming` may be true or false.
    Stream,
}

/// Validate execution flags for the chosen entrypoint (`workflow` is reserved for future graph checks).
pub fn validate_yaml_workflow_execution(
    _workflow: &YamlWorkflow,
    flags: YamlWorkflowExecutionFlags,
    surface: YamlWorkflowExecutionSurface,
) -> Result<(), YamlWorkflowRunError> {
    if matches!(surface, YamlWorkflowExecutionSurface::Run) && flags.workflow_streaming {
        return Err(YamlWorkflowRunError::InvalidInput {
            message: "workflow_streaming=true is not valid for run/run_async (no event sink); use stream(request, sink) or set workflow_streaming=false".to_string(),
        });
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct YamlWorkflowRunOptions {
    #[serde(default)]
    pub telemetry: YamlWorkflowTelemetryConfig,
    #[serde(default)]
    pub trace: YamlWorkflowTraceOptions,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YamlLlmTokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlLlmExecutionResult {
    pub payload: Value,
    pub usage: Option<YamlLlmTokenUsage>,
    pub ttft_ms: Option<u128>,
    pub tool_calls: Vec<YamlToolCallTrace>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlToolCallTrace {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub output: Option<Value>,
    pub status: String,
    pub elapsed_ms: u128,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct YamlTokenTotals {
    pub(super) input_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) total_tokens: u64,
    pub(super) reasoning_tokens: Option<u64>,
}

impl YamlTokenTotals {
    pub(super) fn add_usage(&mut self, usage: &YamlLlmTokenUsage) {
        self.input_tokens += u64::from(usage.prompt_tokens);
        self.output_tokens += u64::from(usage.completion_tokens);
        self.total_tokens += u64::from(usage.total_tokens);

        if let Some(reasoning_tokens) = usage.reasoning_tokens {
            let next = self.reasoning_tokens.unwrap_or(0) + u64::from(reasoning_tokens);
            self.reasoning_tokens = Some(next);
        }
    }

    pub(super) fn tokens_per_second(&self, elapsed_ms: u128) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        round_two_decimals((self.output_tokens as f64) * 1000.0 / (elapsed_ms as f64))
    }
}

fn round_two_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub(super) fn completion_tokens_per_second(completion_tokens: u32, elapsed_ms: u128) -> f64 {
    if elapsed_ms == 0 {
        return 0.0;
    }
    round_two_decimals((completion_tokens as f64) * 1000.0 / (elapsed_ms as f64))
}

pub(super) fn resolve_requested_model(
    run_model_override: Option<&str>,
    node_model: &str,
) -> String {
    run_model_override
        .and_then(|model| {
            let trimmed = model.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_else(|| node_model.to_string())
}

fn default_true() -> bool {
    true
}

fn default_sample_rate() -> f32 {
    1.0
}

fn default_retention_days() -> u32 {
    30
}

pub(super) fn validate_sample_rate(sample_rate: f32) -> Result<(), super::YamlWorkflowRunError> {
    if sample_rate.is_finite() && (0.0..=1.0).contains(&sample_rate) {
        return Ok(());
    }

    Err(super::YamlWorkflowRunError::InvalidInput {
        message: format!(
            "telemetry.sample_rate must be between 0.0 and 1.0 inclusive; received {sample_rate}"
        ),
    })
}
