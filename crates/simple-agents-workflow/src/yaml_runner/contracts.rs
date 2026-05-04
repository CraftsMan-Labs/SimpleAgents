use std::collections::HashMap;

use async_trait::async_trait;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_json::Value;
use simple_agent_type::message::{MessageContent, Role};
use simple_agent_type::tool::{ToolChoice, ToolType};
use thiserror::Error;

use super::{TraceContext, YamlToolTraceMode, YamlWorkflowTraceTenantContext};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YamlWorkflowTokenKind {
    Output,
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlWorkflowEvent {
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streamable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_kind: Option<YamlWorkflowTokenKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_terminal_node_token: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

pub type WorkflowMessageRole = Role;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowMessage {
    pub role: WorkflowMessageRole,
    pub content: MessageContent,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, alias = "toolCallId")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlTemplateBinding {
    pub index: usize,
    pub expression: String,
    pub source_path: String,
    pub resolved: Value,
    pub resolved_type: String,
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum YamlWorkflowDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YamlWorkflowDiagnostic {
    pub node_id: Option<String>,
    pub code: String,
    pub severity: YamlWorkflowDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum YamlWorkflowRunError {
    #[error("failed to read workflow yaml '{path}': {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse workflow yaml '{path}': {source}")]
    Parse {
        path: String,
        source: serde_yaml::Error,
    },
    #[error("rejected workflow yaml '{path}': {reason}")]
    FileRejected { path: String, reason: String },
    #[error("workflow '{workflow_id}' has no nodes")]
    EmptyNodes { workflow_id: String },
    #[error("entry node '{entry_node}' does not exist")]
    MissingEntry { entry_node: String },
    #[error("unknown node id '{node_id}'")]
    MissingNode { node_id: String },
    #[error("unsupported node type in '{node_id}'")]
    UnsupportedNodeType { node_id: String },
    #[error("unsupported switch condition format: {condition}")]
    UnsupportedCondition { condition: String },
    #[error("switch node '{node_id}' has no valid next target")]
    InvalidSwitchTarget { node_id: String },
    #[error("llm returned non-object payload for node '{node_id}'")]
    LlmPayloadNotObject { node_id: String },
    #[error("custom worker handler '{handler}' is not supported")]
    UnsupportedCustomHandler { handler: String },
    #[error("llm execution failed for node '{node_id}': {message}")]
    Llm { node_id: String, message: String },
    #[error("custom worker execution failed for node '{node_id}': {message}")]
    CustomWorker { node_id: String, message: String },
    #[error("workflow validation failed with {diagnostics_count} error(s)")]
    Validation {
        diagnostics_count: usize,
        diagnostics: Vec<YamlWorkflowDiagnostic>,
    },
    #[error("invalid workflow input: {message}")]
    InvalidInput { message: String },
    #[error("ir runtime execution failed: {message}")]
    IrRuntime { message: String },
    #[error("workflow event stream cancelled: {message}")]
    EventSinkCancelled { message: String },
}

impl Serialize for YamlWorkflowRunError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("YamlWorkflowRunError", 2)?;
        state.serialize_field("code", yaml_workflow_run_error_code(self))?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

fn yaml_workflow_run_error_code(error: &YamlWorkflowRunError) -> &'static str {
    match error {
        YamlWorkflowRunError::Read { .. } => "read_failed",
        YamlWorkflowRunError::Parse { .. } => "parse_failed",
        YamlWorkflowRunError::FileRejected { .. } => "file_rejected",
        YamlWorkflowRunError::EmptyNodes { .. } => "empty_nodes",
        YamlWorkflowRunError::MissingEntry { .. } => "missing_entry",
        YamlWorkflowRunError::MissingNode { .. } => "missing_node",
        YamlWorkflowRunError::UnsupportedNodeType { .. } => "unsupported_node_type",
        YamlWorkflowRunError::UnsupportedCondition { .. } => "unsupported_condition",
        YamlWorkflowRunError::InvalidSwitchTarget { .. } => "invalid_switch_target",
        YamlWorkflowRunError::LlmPayloadNotObject { .. } => "llm_payload_not_object",
        YamlWorkflowRunError::UnsupportedCustomHandler { .. } => "unsupported_custom_handler",
        YamlWorkflowRunError::Llm { .. } => "llm_failed",
        YamlWorkflowRunError::CustomWorker { .. } => "custom_worker_failed",
        YamlWorkflowRunError::Validation { .. } => "validation_failed",
        YamlWorkflowRunError::InvalidInput { .. } => "invalid_input",
        YamlWorkflowRunError::IrRuntime { .. } => "ir_runtime_failed",
        YamlWorkflowRunError::EventSinkCancelled { .. } => "event_sink_cancelled",
    }
}

pub trait YamlWorkflowEventSink: Send + Sync {
    fn emit(&self, event: &YamlWorkflowEvent);

    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Returns true for token delta events that should be gated by `workflow_streaming`.
pub fn is_workflow_stream_delta_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "node_stream_delta"
            | "node_stream_thinking_delta"
            | "node_stream_output_delta"
            | "node_stream_snapshot"
    )
}

/// Forwards events to `inner` unless `workflow_streaming` is false and the event is a stream delta.
pub struct YamlWorkflowStreamFilterSink<'a> {
    inner: &'a dyn YamlWorkflowEventSink,
    workflow_streaming: bool,
}

impl<'a> YamlWorkflowStreamFilterSink<'a> {
    pub fn new(inner: &'a dyn YamlWorkflowEventSink, workflow_streaming: bool) -> Self {
        Self {
            inner,
            workflow_streaming,
        }
    }
}

impl YamlWorkflowEventSink for YamlWorkflowStreamFilterSink<'_> {
    fn emit(&self, event: &YamlWorkflowEvent) {
        if !self.workflow_streaming && is_workflow_stream_delta_event(event.event_type.as_str()) {
            return;
        }
        self.inner.emit(event);
    }

    fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

pub struct NoopYamlWorkflowEventSink;

impl YamlWorkflowEventSink for NoopYamlWorkflowEventSink {
    fn emit(&self, _event: &YamlWorkflowEvent) {}
}

pub(super) fn workflow_event_sink_cancelled_message() -> &'static str {
    "workflow event callback cancelled"
}

pub(super) fn event_sink_is_cancelled(event_sink: Option<&dyn YamlWorkflowEventSink>) -> bool {
    event_sink.map(|sink| sink.is_cancelled()).unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum YamlToIrError {
    #[error("entry node '{entry_node}' does not exist")]
    MissingEntry { entry_node: String },
    #[error("node '{node_id}' has multiple outgoing edges in YAML; IR llm/tool nodes require one")]
    MultipleOutgoingEdge { node_id: String },
    #[error("node '{node_id}' is unsupported for IR conversion: {reason}")]
    UnsupportedNode { node_id: String, reason: String },
}

#[derive(Debug, Clone)]
pub struct YamlLlmExecutionRequest {
    pub node_id: String,
    pub is_terminal_node: bool,
    pub stream_json_as_text: bool,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub messages: Option<Vec<super::Message>>,
    pub append_prompt_as_user: bool,
    pub prompt: String,
    pub prompt_template: String,
    pub prompt_bindings: Vec<YamlTemplateBinding>,
    pub schema: Value,
    pub stream: bool,
    pub heal: bool,
    pub send_schema: bool,
    pub tools: Vec<YamlResolvedTool>,
    pub tool_choice: Option<ToolChoice>,
    pub max_tool_roundtrips: u8,
    pub tool_calls_global_key: Option<String>,
    pub tool_trace_mode: YamlToolTraceMode,
    pub execution_context: Value,
    pub trace_id: Option<String>,
    pub trace_context: Option<TraceContext>,
    pub tenant_context: YamlWorkflowTraceTenantContext,
    pub trace_sampled: bool,
    /// Split thinking/output stream events (mirrors [`YamlWorkflowExecutionFlags::split_stream_deltas`]).
    pub split_stream_deltas: bool,
    /// Include partial streamed text in structured JSON parse error messages when debugging.
    pub debug_stream_parse: bool,
}

#[derive(Debug, Clone)]
pub struct YamlResolvedTool {
    pub definition: super::ToolDefinition,
    pub output_schema: Option<Value>,
}

#[async_trait]
pub trait YamlWorkflowLlmExecutor: Send + Sync {
    async fn complete_structured(
        &self,
        request: YamlLlmExecutionRequest,
        event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<super::YamlLlmExecutionResult, String>;
}

#[async_trait]
pub trait YamlWorkflowCustomWorkerExecutor: Send + Sync {
    async fn execute(
        &self,
        handler: &str,
        handler_file: Option<&str>,
        payload: &Value,
        context: &Value,
    ) -> Result<Value, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum YamlGlobalUpdateOp {
    Set,
    Append,
    Increment,
    Merge,
}

impl YamlGlobalUpdateOp {
    pub fn as_str(self) -> &'static str {
        match self {
            YamlGlobalUpdateOp::Set => "set",
            YamlGlobalUpdateOp::Append => "append",
            YamlGlobalUpdateOp::Increment => "increment",
            YamlGlobalUpdateOp::Merge => "merge",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlGlobalUpdate {
    pub op: YamlGlobalUpdateOp,
    pub from: Option<String>,
    pub by: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlNodeConfig {
    pub prompt: Option<String>,
    #[serde(default, alias = "schema")]
    pub output_schema: Option<Value>,
    pub payload: Option<Value>,
    pub set_globals: Option<HashMap<String, String>>,
    pub update_globals: Option<HashMap<String, YamlGlobalUpdate>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlCustomWorker {
    pub handler: String,
    pub handler_file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YamlHumanInputType {
    Choice,
    Text,
    Form,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlHumanInputOption {
    pub value: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlHumanInput {
    pub input_type: YamlHumanInputType,
    pub prompt: Option<String>,
    pub options: Option<Vec<YamlHumanInputOption>>,
    pub form_schema: Option<Value>,
    pub form_prefill: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlSwitchBranch {
    pub condition: String,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlSwitch {
    #[serde(default)]
    pub branches: Vec<YamlSwitchBranch>,
    pub default: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum YamlToolChoiceConfig {
    Mode(super::ToolChoiceMode),
    Function(YamlToolChoiceFunction),
    OpenAi(super::ToolChoiceTool),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlToolChoiceFunction {
    pub function: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlSimplifiedToolDeclaration {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlOpenAiToolFunction {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlOpenAiToolDeclaration {
    #[serde(rename = "type")]
    pub tool_type: Option<ToolType>,
    pub function: YamlOpenAiToolFunction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum YamlToolDeclaration {
    OpenAi(YamlOpenAiToolDeclaration),
    Simplified(YamlSimplifiedToolDeclaration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YamlToolFormat {
    #[default]
    Openai,
    Simplified,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlLlmCall {
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stream: Option<bool>,
    pub stream_json_as_text: Option<bool>,
    pub heal: Option<bool>,
    pub send_schema: Option<bool>,
    pub messages_path: Option<String>,
    pub append_prompt_as_user: Option<bool>,
    #[serde(default)]
    pub tools_format: YamlToolFormat,
    #[serde(default)]
    pub tools: Vec<YamlToolDeclaration>,
    pub tool_choice: Option<YamlToolChoiceConfig>,
    pub max_tool_roundtrips: Option<u8>,
    pub tool_calls_global_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlNodeType {
    pub llm_call: Option<YamlLlmCall>,
    pub switch: Option<YamlSwitch>,
    pub custom_worker: Option<YamlCustomWorker>,
    pub human_input: Option<YamlHumanInput>,
    pub end: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlNode {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub node_type: YamlNodeType,
    pub config: Option<YamlNodeConfig>,
}

impl YamlNode {
    pub(super) fn kind_name(&self) -> &'static str {
        if self.node_type.llm_call.is_some() {
            "llm_call"
        } else if self.node_type.switch.is_some() {
            "switch"
        } else if self.node_type.custom_worker.is_some() {
            "custom_worker"
        } else if self.node_type.human_input.is_some() {
            "human_input"
        } else if self.node_type.end.is_some() {
            "end"
        } else {
            "unknown"
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlWorkflow {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, Value>>,
    pub entry_node: String,
    #[serde(default)]
    pub nodes: Vec<YamlNode>,
    #[serde(default)]
    pub edges: Vec<YamlEdge>,
}
