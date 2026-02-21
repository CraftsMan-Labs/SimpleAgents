use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use simple_agent_type::message::{Message, Role};
use simple_agent_type::request::CompletionRequest;
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, SimpleAgentsClient,
};
use simple_agents_healing::JsonishParser;
use thiserror::Error;

use crate::ir::{Node, NodeKind, RouterRoute, WorkflowDefinition, WORKFLOW_IR_V0};
use crate::observability::tracing::{workflow_tracer, SpanKind, TraceContext, WorkflowSpan};
use crate::runtime::{
    LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor, ToolExecutionError,
    ToolExecutionInput, ToolExecutor, WorkflowRuntime, WorkflowRuntimeError,
    WorkflowRuntimeOptions,
};
use crate::visualize::workflow_to_mermaid;

const YAML_START_NODE_ID: &str = "__yaml_start";
const YAML_LLM_TOOL_ID: &str = "__yaml_llm_call";

static TRACE_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlStepTiming {
    pub node_id: String,
    pub node_kind: String,
    pub elapsed_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlLlmNodeMetrics {
    pub elapsed_ms: u128,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u32>,
    pub tokens_per_second: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlWorkflowRunOutput {
    pub workflow_id: String,
    pub entry_node: String,
    pub email_text: String,
    pub trace: Vec<String>,
    pub outputs: BTreeMap<String, Value>,
    pub terminal_node: String,
    pub terminal_output: Option<Value>,
    pub step_timings: Vec<YamlStepTiming>,
    pub llm_node_metrics: BTreeMap<String, YamlLlmNodeMetrics>,
    pub total_elapsed_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u128>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_thinking_tokens: Option<u64>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct YamlWorkflowTraceOptions {
    #[serde(default)]
    pub context: Option<YamlWorkflowTraceContextInput>,
    #[serde(default)]
    pub tenant: YamlWorkflowTraceTenantContext,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct YamlWorkflowRunOptions {
    #[serde(default)]
    pub telemetry: YamlWorkflowTelemetryConfig,
    #[serde(default)]
    pub trace: YamlWorkflowTraceOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YamlLlmTokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub thinking_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlLlmExecutionResult {
    pub payload: Value,
    pub usage: Option<YamlLlmTokenUsage>,
    pub ttft_ms: Option<u128>,
}

#[derive(Debug, Clone, Default)]
struct YamlTokenTotals {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    thinking_tokens: Option<u64>,
}

impl YamlTokenTotals {
    fn add_usage(&mut self, usage: &YamlLlmTokenUsage) {
        self.input_tokens += u64::from(usage.prompt_tokens);
        self.output_tokens += u64::from(usage.completion_tokens);
        self.total_tokens += u64::from(usage.total_tokens);

        if let Some(thinking_tokens) = usage.thinking_tokens {
            let next = self.thinking_tokens.unwrap_or(0) + u64::from(thinking_tokens);
            self.thinking_tokens = Some(next);
        }
    }

    fn tokens_per_second(&self, elapsed_ms: u128) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        round_two_decimals((self.output_tokens as f64) * 1000.0 / (elapsed_ms as f64))
    }
}

fn round_two_decimals(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn completion_tokens_per_second(completion_tokens: u32, elapsed_ms: u128) -> f64 {
    if elapsed_ms == 0 {
        return 0.0;
    }
    round_two_decimals((completion_tokens as f64) * 1000.0 / (elapsed_ms as f64))
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

fn generate_trace_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = u128::from(TRACE_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    format!("{:032x}", now_nanos ^ sequence)
}

fn resolve_trace_id(options: &YamlWorkflowRunOptions, span_context: &TraceContext) -> String {
    options
        .trace
        .context
        .as_ref()
        .and_then(|context| context.trace_id.clone())
        .or_else(|| span_context.trace_id.clone())
        .unwrap_or_else(generate_trace_id)
}

fn workflow_metadata_with_trace(options: &YamlWorkflowRunOptions, trace_id: &str) -> Value {
    json!({
        "telemetry": {
            "trace_id": trace_id,
            "enabled": options.telemetry.enabled,
            "nerdstats": options.telemetry.nerdstats,
            "sample_rate": options.telemetry.sample_rate,
            "payload_mode": match options.telemetry.payload_mode {
                YamlWorkflowPayloadMode::FullPayload => "full_payload",
                YamlWorkflowPayloadMode::RedactedPayload => "redacted_payload",
            },
            "retention_days": options.telemetry.retention_days,
            "multi_tenant": options.telemetry.multi_tenant,
        },
        "trace": {
            "tenant": {
                "workspace_id": options.trace.tenant.workspace_id,
                "user_id": options.trace.tenant.user_id,
                "conversation_id": options.trace.tenant.conversation_id,
                "request_id": options.trace.tenant.request_id,
                "run_id": options.trace.tenant.run_id,
            }
        },
    })
}

fn apply_trace_tenant_attributes(span: &mut dyn WorkflowSpan, options: &YamlWorkflowRunOptions) {
    if let Some(workspace_id) = options.trace.tenant.workspace_id.as_deref() {
        span.set_attribute("tenant.workspace_id", workspace_id);
    }
    if let Some(user_id) = options.trace.tenant.user_id.as_deref() {
        span.set_attribute("tenant.user_id", user_id);
    }
    if let Some(conversation_id) = options.trace.tenant.conversation_id.as_deref() {
        span.set_attribute("tenant.conversation_id", conversation_id);
    }
    if let Some(request_id) = options.trace.tenant.request_id.as_deref() {
        span.set_attribute("tenant.request_id", request_id);
    }
    if let Some(run_id) = options.trace.tenant.run_id.as_deref() {
        span.set_attribute("tenant.run_id", run_id);
    }
}

fn workflow_nerdstats(output: &YamlWorkflowRunOutput) -> Value {
    let llm_nodes_without_usage: Vec<String> = output
        .step_timings
        .iter()
        .filter(|step| step.node_kind == "llm_call" && step.total_tokens.is_none())
        .map(|step| step.node_id.clone())
        .collect();
    let token_metrics_available = llm_nodes_without_usage.is_empty();
    let token_metrics_source = if token_metrics_available {
        "provider_usage"
    } else {
        "provider_stream_usage_unavailable"
    };
    let total_input_tokens = if token_metrics_available {
        json!(output.total_input_tokens)
    } else {
        Value::Null
    };
    let total_output_tokens = if token_metrics_available {
        json!(output.total_output_tokens)
    } else {
        Value::Null
    };
    let total_tokens = if token_metrics_available {
        json!(output.total_tokens)
    } else {
        Value::Null
    };
    let total_thinking_tokens = if token_metrics_available {
        json!(output.total_thinking_tokens)
    } else {
        Value::Null
    };
    let tokens_per_second = if token_metrics_available {
        json!(output.tokens_per_second)
    } else {
        Value::Null
    };

    json!({
        "workflow_id": output.workflow_id,
        "terminal_node": output.terminal_node,
        "total_elapsed_ms": output.total_elapsed_ms,
        "ttft_ms": output.ttft_ms,
        "step_timings": output.step_timings,
        "llm_node_metrics": output.llm_node_metrics,
        "total_input_tokens": total_input_tokens,
        "total_output_tokens": total_output_tokens,
        "total_tokens": total_tokens,
        "total_thinking_tokens": total_thinking_tokens,
        "tokens_per_second": tokens_per_second,
        "trace_id": output.trace_id,
        "token_metrics_available": token_metrics_available,
        "token_metrics_source": token_metrics_source,
        "llm_nodes_without_usage": llm_nodes_without_usage,
    })
}

fn payload_for_span(mode: YamlWorkflowPayloadMode, payload: &Value) -> String {
    match mode {
        YamlWorkflowPayloadMode::FullPayload => payload.to_string(),
        YamlWorkflowPayloadMode::RedactedPayload => json!({
            "redacted": true,
            "value_type": match payload {
                Value::Null => "null",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            }
        })
        .to_string(),
    }
}

fn trace_context_from_options(options: &YamlWorkflowRunOptions) -> Option<TraceContext> {
    options.trace.context.as_ref().map(|input| TraceContext {
        trace_id: input.trace_id.clone(),
        span_id: input.span_id.clone(),
        parent_span_id: input.parent_span_id.clone(),
        traceparent: input.traceparent.clone(),
        tracestate: input.tracestate.clone(),
        baggage: input.baggage.clone(),
    })
}

fn merged_trace_context_for_worker(
    span_context: Option<&TraceContext>,
    resolved_trace_id: Option<&str>,
    options: &YamlWorkflowRunOptions,
) -> TraceContext {
    let input_context = options.trace.context.as_ref();
    let baggage = if let Some(context) = span_context {
        if !context.baggage.is_empty() {
            context.baggage.clone()
        } else {
            input_context
                .map(|value| value.baggage.clone())
                .unwrap_or_default()
        }
    } else {
        input_context
            .map(|value| value.baggage.clone())
            .unwrap_or_default()
    };

    TraceContext {
        trace_id: span_context
            .and_then(|context| context.trace_id.clone())
            .or_else(|| resolved_trace_id.map(|value| value.to_string()))
            .or_else(|| input_context.and_then(|context| context.trace_id.clone())),
        span_id: span_context
            .and_then(|context| context.span_id.clone())
            .or_else(|| input_context.and_then(|context| context.span_id.clone())),
        parent_span_id: span_context
            .and_then(|context| context.parent_span_id.clone())
            .or_else(|| input_context.and_then(|context| context.parent_span_id.clone())),
        traceparent: span_context
            .and_then(|context| context.traceparent.clone())
            .or_else(|| input_context.and_then(|context| context.traceparent.clone())),
        tracestate: span_context
            .and_then(|context| context.tracestate.clone())
            .or_else(|| input_context.and_then(|context| context.tracestate.clone())),
        baggage,
    }
}

fn custom_worker_context_with_trace(
    context: &Value,
    trace_context: &TraceContext,
    tenant_context: &YamlWorkflowTraceTenantContext,
) -> Value {
    let mut context_with_trace = context.clone();
    let Some(root) = context_with_trace.as_object_mut() else {
        return context_with_trace;
    };

    root.insert(
        "trace".to_string(),
        json!({
            "context": {
                "trace_id": trace_context.trace_id,
                "span_id": trace_context.span_id,
                "parent_span_id": trace_context.parent_span_id,
                "traceparent": trace_context.traceparent,
                "tracestate": trace_context.tracestate,
                "baggage": trace_context.baggage,
            },
            "tenant": {
                "workspace_id": tenant_context.workspace_id,
                "user_id": tenant_context.user_id,
                "conversation_id": tenant_context.conversation_id,
                "request_id": tenant_context.request_id,
                "run_id": tenant_context.run_id,
            }
        }),
    );

    context_with_trace
}

fn include_raw_stream_debug_events() -> bool {
    match std::env::var("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "on"
        }
        Err(_) => false,
    }
}

#[derive(Debug)]
struct StreamedPayloadResolution {
    payload: Value,
    heal_confidence: Option<f32>,
}

#[derive(Debug, Default)]
struct StreamJsonAsTextFormatter {
    raw_json: String,
    emitted: bool,
}

impl StreamJsonAsTextFormatter {
    fn push(&mut self, chunk: &str) {
        self.raw_json.push_str(chunk);
    }

    fn emit_if_ready(&mut self, complete: bool) -> Option<String> {
        if self.emitted || !complete {
            return None;
        }
        self.emitted = true;
        Some(render_json_object_as_text(self.raw_json.as_str()))
    }
}

fn render_json_object_as_text(raw_json: &str) -> String {
    let value = match serde_json::from_str::<Value>(raw_json) {
        Ok(value) => value,
        Err(_) => return raw_json.to_string(),
    };
    let Some(object) = value.as_object() else {
        return raw_json.to_string();
    };

    let mut lines = Vec::with_capacity(object.len());
    for (key, value) in object {
        let rendered = match value {
            Value::String(text) => text.clone(),
            _ => value.to_string(),
        };
        lines.push(format!("{key}: {rendered}"));
    }
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YamlWorkflowTokenKind {
    Output,
    Thinking,
}

#[derive(Debug, Default)]
struct StructuredJsonDeltaFilter {
    started: bool,
    completed: bool,
    depth: u32,
    in_string: bool,
    escape: bool,
}

impl StructuredJsonDeltaFilter {
    fn split(&mut self, delta: &str) -> (Option<String>, Option<String>) {
        if delta.is_empty() {
            return (None, None);
        }

        let mut output = String::new();
        let mut thinking = String::new();

        for ch in delta.chars() {
            if self.completed {
                thinking.push(ch);
                continue;
            }

            if !self.started {
                if ch != '{' {
                    thinking.push(ch);
                    continue;
                }
                self.started = true;
                self.depth = 1;
                output.push(ch);
                continue;
            }

            output.push(ch);
            if self.in_string {
                if self.escape {
                    self.escape = false;
                    continue;
                }
                if ch == '\\' {
                    self.escape = true;
                    continue;
                }
                if ch == '"' {
                    self.in_string = false;
                }
                continue;
            }

            match ch {
                '"' => self.in_string = true,
                '{' => self.depth = self.depth.saturating_add(1),
                '}' => {
                    if self.depth > 0 {
                        self.depth -= 1;
                    }
                    if self.depth == 0 {
                        self.completed = true;
                    }
                }
                _ => {}
            }
        }

        let output = if output.is_empty() {
            None
        } else {
            Some(output)
        };
        let thinking = if thinking.is_empty() {
            None
        } else {
            Some(thinking)
        };

        (output, thinking)
    }

    fn completed(&self) -> bool {
        self.completed
    }
}

fn extract_last_fenced_json_block(raw: &str) -> Option<&str> {
    let start = raw.rfind("```json")?;
    let remainder = &raw[start + "```json".len()..];
    let end = remainder.find("```")?;
    let candidate = remainder[..end].trim();
    if candidate.is_empty() {
        return None;
    }
    Some(candidate)
}

fn extract_balanced_object_from(raw: &str, start_index: usize) -> Option<&str> {
    let mut depth = 0u32;
    let mut in_string = false;
    let mut escape = false;

    for (relative_index, ch) in raw[start_index..].char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth = depth.saturating_add(1),
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    let end_index = start_index + relative_index + ch.len_utf8();
                    return Some(raw[start_index..end_index].trim());
                }
            }
            _ => {}
        }
    }

    None
}

fn extract_last_parsable_object(raw: &str) -> Option<&str> {
    let starts: Vec<usize> = raw
        .char_indices()
        .filter_map(|(index, ch)| if ch == '{' { Some(index) } else { None })
        .collect();

    for start in starts.into_iter().rev() {
        let Some(candidate) = extract_balanced_object_from(raw, start) else {
            continue;
        };
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate);
        }
    }

    None
}

fn resolve_structured_json_candidate(raw: &str) -> Option<&str> {
    extract_last_fenced_json_block(raw).or_else(|| extract_last_parsable_object(raw))
}

fn parse_streamed_structured_payload(
    raw: &str,
    heal: bool,
) -> Result<StreamedPayloadResolution, String> {
    if !heal {
        if let Ok(payload) = serde_json::from_str::<Value>(raw) {
            return Ok(StreamedPayloadResolution {
                payload,
                heal_confidence: None,
            });
        }

        let candidate = resolve_structured_json_candidate(raw).ok_or_else(|| {
            "failed to parse streamed structured completion JSON: no JSON object candidate found"
                .to_string()
        })?;
        let payload = serde_json::from_str::<Value>(candidate).map_err(|error| {
            format!(
                "failed to parse streamed structured completion JSON: {error}; candidate={candidate}"
            )
        })?;
        return Ok(StreamedPayloadResolution {
            payload,
            heal_confidence: None,
        });
    }

    let candidate = resolve_structured_json_candidate(raw).unwrap_or(raw);
    let parser = JsonishParser::new();
    let healed = parser
        .parse(candidate)
        .map_err(|error| format!("failed to heal streamed structured completion JSON: {error}"))?;

    Ok(StreamedPayloadResolution {
        payload: healed.value,
        heal_confidence: Some(healed.confidence),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlWorkflowEvent {
    pub event_type: String,
    pub node_id: Option<String>,
    pub step_id: Option<String>,
    pub node_kind: Option<String>,
    pub streamable: Option<bool>,
    pub message: Option<String>,
    pub delta: Option<String>,
    pub token_kind: Option<YamlWorkflowTokenKind>,
    pub is_terminal_node_token: Option<bool>,
    pub elapsed_ms: Option<u128>,
    pub metadata: Option<Value>,
}

pub type WorkflowMessageRole = Role;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowMessage {
    pub role: WorkflowMessageRole,
    pub content: String,
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

pub trait YamlWorkflowEventSink: Send + Sync {
    fn emit(&self, event: &YamlWorkflowEvent);

    fn is_cancelled(&self) -> bool {
        false
    }
}

pub struct NoopYamlWorkflowEventSink;

impl YamlWorkflowEventSink for NoopYamlWorkflowEventSink {
    fn emit(&self, _event: &YamlWorkflowEvent) {}
}

fn workflow_event_sink_cancelled_message() -> &'static str {
    "workflow event callback cancelled"
}

fn event_sink_is_cancelled(event_sink: Option<&dyn YamlWorkflowEventSink>) -> bool {
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

/// Render a YAML workflow graph as Mermaid flowchart.
pub fn yaml_workflow_to_mermaid(workflow: &YamlWorkflow) -> String {
    if let Ok(ir) = yaml_workflow_to_ir(workflow) {
        return workflow_to_mermaid(&ir);
    }

    yaml_workflow_to_mermaid_fallback(workflow)
}

fn yaml_workflow_to_mermaid_fallback(workflow: &YamlWorkflow) -> String {
    let mut lines = Vec::new();
    lines.push("flowchart TD".to_string());

    for node in &workflow.nodes {
        lines.push(format!(
            "  {}[\"{}\\n({})\"]",
            sanitize_mermaid_id(&node.id),
            escape_mermaid_label(&node.id),
            node.kind_name()
        ));
    }

    let mut emitted: HashSet<(String, String, String)> = HashSet::new();

    for edge in &workflow.edges {
        emitted.insert((edge.from.clone(), String::new(), edge.to.clone()));
    }

    for node in &workflow.nodes {
        if let Some(switch) = node.node_type.switch.as_ref() {
            for branch in &switch.branches {
                emitted.insert((
                    node.id.clone(),
                    branch.condition.clone(),
                    branch.target.clone(),
                ));
            }
            emitted.insert((
                node.id.clone(),
                "default".to_string(),
                switch.default.clone(),
            ));
        }
    }

    let mut edges = emitted.into_iter().collect::<Vec<_>>();
    edges.sort();

    for (from, label, to) in edges {
        if label.is_empty() {
            lines.push(format!(
                "  {} --> {}",
                sanitize_mermaid_id(&from),
                sanitize_mermaid_id(&to)
            ));
        } else {
            lines.push(format!(
                "  {} -- \"{}\" --> {}",
                sanitize_mermaid_id(&from),
                escape_mermaid_label(&label),
                sanitize_mermaid_id(&to)
            ));
        }
    }

    lines.join("\n")
}

/// Load a YAML workflow file and render it as Mermaid flowchart.
pub fn yaml_workflow_file_to_mermaid(workflow_path: &Path) -> Result<String, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    Ok(yaml_workflow_to_mermaid(&workflow))
}

pub fn yaml_workflow_to_ir(workflow: &YamlWorkflow) -> Result<WorkflowDefinition, YamlToIrError> {
    let known_ids: HashSet<&str> = workflow.nodes.iter().map(|n| n.id.as_str()).collect();
    if !known_ids.contains(workflow.entry_node.as_str()) {
        return Err(YamlToIrError::MissingEntry {
            entry_node: workflow.entry_node.clone(),
        });
    }

    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &workflow.edges {
        outgoing
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }

    let mut nodes = Vec::with_capacity(workflow.nodes.len() + 1);
    nodes.push(Node {
        id: YAML_START_NODE_ID.to_string(),
        kind: NodeKind::Start {
            next: workflow.entry_node.clone(),
        },
    });

    for node in &workflow.nodes {
        if let Some(llm) = node.node_type.llm_call.as_ref() {
            if node
                .config
                .as_ref()
                .and_then(|c| c.set_globals.as_ref())
                .is_some()
                || node
                    .config
                    .as_ref()
                    .and_then(|c| c.update_globals.as_ref())
                    .is_some()
            {
                return Err(YamlToIrError::UnsupportedNode {
                    node_id: node.id.clone(),
                    reason: "set_globals/update_globals are not represented in canonical IR llm nodes yet"
                        .to_string(),
                });
            }

            let next = single_next_for_node(&outgoing, &node.id)?;
            nodes.push(Node {
                id: node.id.clone(),
                kind: NodeKind::Tool {
                    tool: YAML_LLM_TOOL_ID.to_string(),
                    input: json!({
                        "node_id": node.id,
                        "model": llm.model,
                        "prompt_template": node
                            .config
                            .as_ref()
                            .and_then(|c| c.prompt.clone())
                            .unwrap_or_default(),
                        "stream": llm.stream.unwrap_or(false),
                        "stream_json_as_text": llm.stream_json_as_text.unwrap_or(false),
                        "heal": llm.heal.unwrap_or(false),
                        "messages_path": llm.messages_path,
                        "append_prompt_as_user": llm.append_prompt_as_user.unwrap_or(true),
                        "output_schema": node
                            .config
                            .as_ref()
                            .and_then(|c| c.output_schema.clone())
                            .unwrap_or_else(default_llm_output_schema),
                    }),
                    next,
                },
            });
            continue;
        }

        if let Some(worker) = node.node_type.custom_worker.as_ref() {
            if node
                .config
                .as_ref()
                .and_then(|c| c.set_globals.as_ref())
                .is_some()
                || node
                    .config
                    .as_ref()
                    .and_then(|c| c.update_globals.as_ref())
                    .is_some()
            {
                return Err(YamlToIrError::UnsupportedNode {
                    node_id: node.id.clone(),
                    reason: "set_globals/update_globals are not represented in canonical IR tool nodes yet"
                        .to_string(),
                });
            }

            let next = single_next_for_node(&outgoing, &node.id)?;
            nodes.push(Node {
                id: node.id.clone(),
                kind: NodeKind::Tool {
                    tool: worker.handler.clone(),
                    input: node
                        .config
                        .as_ref()
                        .and_then(|c| c.payload.clone())
                        .unwrap_or_else(|| json!({})),
                    next,
                },
            });
            continue;
        }

        if let Some(switch) = node.node_type.switch.as_ref() {
            nodes.push(Node {
                id: node.id.clone(),
                kind: NodeKind::Router {
                    routes: switch
                        .branches
                        .iter()
                        .map(|b| RouterRoute {
                            when: rewrite_yaml_condition_to_ir(&b.condition),
                            next: b.target.clone(),
                        })
                        .collect(),
                    default: switch.default.clone(),
                },
            });
            continue;
        }

        return Err(YamlToIrError::UnsupportedNode {
            node_id: node.id.clone(),
            reason: "node_type must be llm_call, switch, or custom_worker".to_string(),
        });
    }

    Ok(WorkflowDefinition {
        version: WORKFLOW_IR_V0.to_string(),
        name: workflow.id.clone(),
        nodes,
    })
}

fn single_next_for_node(
    outgoing: &HashMap<&str, Vec<&str>>,
    node_id: &str,
) -> Result<Option<String>, YamlToIrError> {
    match outgoing.get(node_id) {
        None => Ok(None),
        Some(targets) if targets.len() == 1 => Ok(Some(targets[0].to_string())),
        Some(_) => Err(YamlToIrError::MultipleOutgoingEdge {
            node_id: node_id.to_string(),
        }),
    }
}

fn rewrite_yaml_condition_to_ir(expr: &str) -> String {
    let rewritten = expr
        .replace("$.nodes.", "$.node_outputs.")
        .replace(".output.", ".");
    if let Some(prefix) = rewritten.strip_suffix(".output") {
        prefix.to_string()
    } else {
        rewritten
    }
}

fn sanitize_mermaid_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len() + 1);
    if id
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        out.push_str(id);
    } else {
        out.push('n');
        out.push('_');
        out.push_str(id);
    }
    out.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn escape_mermaid_label(label: &str) -> String {
    label.replace('"', "\\\"")
}

#[derive(Debug, Clone)]
pub struct YamlLlmExecutionRequest {
    pub node_id: String,
    pub is_terminal_node: bool,
    pub stream_json_as_text: bool,
    pub model: String,
    pub messages: Option<Vec<Message>>,
    pub append_prompt_as_user: bool,
    pub prompt: String,
    pub prompt_template: String,
    pub prompt_bindings: Vec<YamlTemplateBinding>,
    pub schema: Value,
    pub stream: bool,
    pub heal: bool,
}

#[async_trait]
pub trait YamlWorkflowLlmExecutor: Send + Sync {
    async fn complete_structured(
        &self,
        request: YamlLlmExecutionRequest,
        event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<YamlLlmExecutionResult, String>;
}

#[async_trait]
pub trait YamlWorkflowCustomWorkerExecutor: Send + Sync {
    async fn execute(
        &self,
        handler: &str,
        payload: &Value,
        email_text: &str,
        context: &Value,
    ) -> Result<Value, String>;
}

pub async fn run_workflow_yaml_file(
    workflow_path: &Path,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_workflow_yaml(&workflow, workflow_input, executor).await
}

pub async fn run_email_workflow_yaml_file(
    workflow_path: &Path,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_file(workflow_path, &workflow_input, executor).await
}

pub async fn run_workflow_yaml_file_with_client(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_workflow_yaml_with_client(&workflow, workflow_input, client).await
}

pub async fn run_email_workflow_yaml_file_with_client(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_file_with_client(workflow_path, &workflow_input, client).await
}

pub async fn run_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_workflow_yaml_with_client_and_custom_worker(workflow, workflow_input, client, None).await
}

pub async fn run_email_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_client(workflow, &workflow_input, client).await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_workflow_yaml_with_client_and_custom_worker(
        &workflow,
        workflow_input,
        client,
        custom_worker,
    )
    .await
}

pub async fn run_email_workflow_yaml_file_with_client_and_custom_worker(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_file_with_client_and_custom_worker(
        workflow_path,
        &workflow_input,
        client,
        custom_worker,
    )
    .await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker_and_events(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
        workflow_path,
        workflow_input,
        client,
        custom_worker,
        event_sink,
        &YamlWorkflowRunOptions::default(),
    )
    .await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        &workflow,
        workflow_input,
        client,
        custom_worker,
        event_sink,
        options,
    )
    .await
}

pub async fn run_email_workflow_yaml_file_with_client_and_custom_worker_and_events(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_file_with_client_and_custom_worker_and_events(
        workflow_path,
        &workflow_input,
        client,
        custom_worker,
        event_sink,
    )
    .await
}

pub async fn run_workflow_yaml_with_client_and_custom_worker(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        workflow,
        workflow_input,
        client,
        custom_worker,
        None,
        &YamlWorkflowRunOptions::default(),
    )
    .await
}

pub async fn run_email_workflow_yaml_with_client_and_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_client_and_custom_worker(
        workflow,
        &workflow_input,
        client,
        custom_worker,
    )
    .await
}

pub async fn run_workflow_yaml_with_client_and_custom_worker_and_events(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        workflow,
        workflow_input,
        client,
        custom_worker,
        event_sink,
        &YamlWorkflowRunOptions::default(),
    )
    .await
}

pub async fn run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    struct BorrowedClientExecutor<'a> {
        client: &'a SimpleAgentsClient,
    }

    #[async_trait]
    impl<'a> YamlWorkflowLlmExecutor for BorrowedClientExecutor<'a> {
        async fn complete_structured(
            &self,
            request: YamlLlmExecutionRequest,
            event_sink: Option<&dyn YamlWorkflowEventSink>,
        ) -> Result<YamlLlmExecutionResult, String> {
            let messages = if let Some(mut history) = request.messages.clone() {
                if request.append_prompt_as_user && !request.prompt.trim().is_empty() {
                    history.push(Message::user(&request.prompt));
                }
                history
            } else {
                vec![
                    Message::system("You execute workflow classification steps."),
                    Message::user(&request.prompt),
                ]
            };

            let mut builder = CompletionRequest::builder()
                .model(&request.model)
                .messages(messages);

            if request.stream {
                builder = builder.stream(true);
            }

            let completion_request = builder
                .build()
                .map_err(|error| format!("failed to build completion request: {error}"))?;

            let completion_options = if request.heal && !request.stream {
                CompletionOptions {
                    mode: CompletionMode::HealedJson,
                }
            } else {
                CompletionOptions::default()
            };

            let outcome = self
                .client
                .complete(&completion_request, completion_options)
                .await
                .map_err(|error| error.to_string())?;

            match outcome {
                CompletionOutcome::Stream(mut stream) => {
                    let mut aggregated = String::new();
                    let mut final_stream_usage: Option<simple_agent_type::response::Usage> = None;
                    let stream_started = Instant::now();
                    let mut ttft_ms: Option<u128> = None;
                    let mut delta_filter = StructuredJsonDeltaFilter::default();
                    let include_raw_debug = include_raw_stream_debug_events();
                    let mut json_text_formatter = if request.stream_json_as_text {
                        Some(StreamJsonAsTextFormatter::default())
                    } else {
                        None
                    };
                    while let Some(chunk_result) = stream.next().await {
                        if event_sink_is_cancelled(event_sink) {
                            return Err(workflow_event_sink_cancelled_message().to_string());
                        }
                        let chunk = chunk_result.map_err(|error| error.to_string())?;
                        if let Some(usage) = chunk.usage {
                            final_stream_usage = Some(usage);
                        }
                        if let Some(choice) = chunk.choices.first() {
                            if ttft_ms.is_none()
                                && (choice
                                    .delta
                                    .content
                                    .as_ref()
                                    .is_some_and(|delta| !delta.is_empty())
                                    || choice
                                        .delta
                                        .reasoning_content
                                        .as_ref()
                                        .is_some_and(|delta| !delta.is_empty()))
                            {
                                ttft_ms = Some(stream_started.elapsed().as_millis());
                            }
                            if include_raw_debug {
                                if let Some(reasoning_delta) =
                                    choice.delta.reasoning_content.as_ref()
                                {
                                    if let Some(sink) = event_sink {
                                        sink.emit(&YamlWorkflowEvent {
                                            event_type: "node_stream_thinking_delta".to_string(),
                                            node_id: Some(request.node_id.clone()),
                                            step_id: Some(request.node_id.clone()),
                                            node_kind: Some("llm_call".to_string()),
                                            streamable: Some(true),
                                            message: None,
                                            delta: Some(reasoning_delta.clone()),
                                            token_kind: Some(YamlWorkflowTokenKind::Thinking),
                                            is_terminal_node_token: Some(request.is_terminal_node),
                                            elapsed_ms: None,
                                            metadata: None,
                                        });
                                    }
                                }
                            }
                            if let Some(delta) = choice.delta.content.clone() {
                                aggregated.push_str(delta.as_str());
                                let (output_delta, thinking_delta) =
                                    delta_filter.split(delta.as_str());
                                let rendered_output_delta = if let Some(output_chunk) = output_delta
                                {
                                    if let Some(formatter) = json_text_formatter.as_mut() {
                                        formatter.push(output_chunk.as_str());
                                        formatter.emit_if_ready(delta_filter.completed())
                                    } else {
                                        Some(output_chunk)
                                    }
                                } else {
                                    None
                                };
                                if include_raw_debug {
                                    if let Some(sink) = event_sink {
                                        if let Some(raw_thinking_delta) = thinking_delta.as_ref() {
                                            sink.emit(&YamlWorkflowEvent {
                                                event_type: "node_stream_thinking_delta"
                                                    .to_string(),
                                                node_id: Some(request.node_id.clone()),
                                                step_id: Some(request.node_id.clone()),
                                                node_kind: Some("llm_call".to_string()),
                                                streamable: Some(true),
                                                message: None,
                                                delta: Some(raw_thinking_delta.clone()),
                                                token_kind: Some(YamlWorkflowTokenKind::Thinking),
                                                is_terminal_node_token: Some(
                                                    request.is_terminal_node,
                                                ),
                                                elapsed_ms: None,
                                                metadata: None,
                                            });
                                        }
                                        if let Some(raw_output_delta) =
                                            rendered_output_delta.as_ref()
                                        {
                                            sink.emit(&YamlWorkflowEvent {
                                                event_type: "node_stream_output_delta".to_string(),
                                                node_id: Some(request.node_id.clone()),
                                                step_id: Some(request.node_id.clone()),
                                                node_kind: Some("llm_call".to_string()),
                                                streamable: Some(true),
                                                message: None,
                                                delta: Some(raw_output_delta.clone()),
                                                token_kind: Some(YamlWorkflowTokenKind::Output),
                                                is_terminal_node_token: Some(
                                                    request.is_terminal_node,
                                                ),
                                                elapsed_ms: None,
                                                metadata: None,
                                            });
                                        }
                                    }
                                }
                                if let Some(filtered_delta) = rendered_output_delta {
                                    if let Some(sink) = event_sink {
                                        sink.emit(&YamlWorkflowEvent {
                                            event_type: "node_stream_delta".to_string(),
                                            node_id: Some(request.node_id.clone()),
                                            step_id: Some(request.node_id.clone()),
                                            node_kind: Some("llm_call".to_string()),
                                            streamable: Some(true),
                                            message: None,
                                            delta: Some(filtered_delta),
                                            token_kind: Some(YamlWorkflowTokenKind::Output),
                                            is_terminal_node_token: Some(request.is_terminal_node),
                                            elapsed_ms: None,
                                            metadata: None,
                                        });
                                    }
                                }
                            }
                        }

                        if event_sink_is_cancelled(event_sink) {
                            return Err(workflow_event_sink_cancelled_message().to_string());
                        }
                    }

                    let resolved =
                        parse_streamed_structured_payload(aggregated.as_str(), request.heal)?;
                    if let Some(confidence) = resolved.heal_confidence {
                        if let Some(sink) = event_sink {
                            sink.emit(&YamlWorkflowEvent {
                                event_type: "node_healed".to_string(),
                                node_id: Some(request.node_id.clone()),
                                step_id: Some(request.node_id.clone()),
                                node_kind: Some("llm_call".to_string()),
                                streamable: Some(true),
                                message: Some(format!(
                                    "healed streamed structured response confidence={confidence}"
                                )),
                                delta: None,
                                token_kind: None,
                                is_terminal_node_token: None,
                                elapsed_ms: None,
                                metadata: None,
                            });
                        }
                    }

                    Ok(YamlLlmExecutionResult {
                        payload: resolved.payload,
                        usage: final_stream_usage.map(|usage| YamlLlmTokenUsage {
                            prompt_tokens: usage.prompt_tokens,
                            completion_tokens: usage.completion_tokens,
                            total_tokens: usage.total_tokens,
                            thinking_tokens: None,
                        }),
                        ttft_ms,
                    })
                }
                CompletionOutcome::Response(response) => {
                    let content = response
                        .content()
                        .ok_or_else(|| "completion returned empty content".to_string())?;
                    let payload = serde_json::from_str(content).map_err(|error| {
                        format!("failed to parse structured completion JSON: {error}")
                    })?;

                    Ok(YamlLlmExecutionResult {
                        payload,
                        usage: Some(YamlLlmTokenUsage {
                            prompt_tokens: response.usage.prompt_tokens,
                            completion_tokens: response.usage.completion_tokens,
                            total_tokens: response.usage.total_tokens,
                            thinking_tokens: None,
                        }),
                        ttft_ms: None,
                    })
                }
                CompletionOutcome::HealedJson(healed) => {
                    if let Some(sink) = event_sink {
                        sink.emit(&YamlWorkflowEvent {
                            event_type: "node_healed".to_string(),
                            node_id: Some(request.node_id.clone()),
                            step_id: Some(request.node_id.clone()),
                            node_kind: Some("llm_call".to_string()),
                            streamable: Some(request.stream),
                            message: Some(format!(
                                "healed structured response confidence={}",
                                healed.parsed.confidence
                            )),
                            delta: None,
                            token_kind: None,
                            is_terminal_node_token: None,
                            elapsed_ms: None,
                            metadata: None,
                        });
                    }
                    Ok(YamlLlmExecutionResult {
                        payload: healed.parsed.value,
                        usage: Some(YamlLlmTokenUsage {
                            prompt_tokens: healed.response.usage.prompt_tokens,
                            completion_tokens: healed.response.usage.completion_tokens,
                            total_tokens: healed.response.usage.total_tokens,
                            thinking_tokens: None,
                        }),
                        ttft_ms: None,
                    })
                }
                CompletionOutcome::CoercedSchema(coerced) => Ok(YamlLlmExecutionResult {
                    payload: coerced.coerced.value,
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: coerced.response.usage.prompt_tokens,
                        completion_tokens: coerced.response.usage.completion_tokens,
                        total_tokens: coerced.response.usage.total_tokens,
                        thinking_tokens: None,
                    }),
                    ttft_ms: None,
                }),
            }
        }
    }

    let executor = BorrowedClientExecutor { client };
    run_workflow_yaml_with_custom_worker_and_events_and_options(
        workflow,
        workflow_input,
        &executor,
        custom_worker,
        event_sink,
        options,
    )
    .await
}

pub async fn run_email_workflow_yaml_with_client_and_custom_worker_and_events(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_client_and_custom_worker_and_events(
        workflow,
        &workflow_input,
        client,
        custom_worker,
        event_sink,
    )
    .await
}

pub async fn run_workflow_yaml(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_workflow_yaml_with_custom_worker_and_events(workflow, workflow_input, executor, None, None)
        .await
}

pub async fn run_email_workflow_yaml(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml(workflow, &workflow_input, executor).await
}

pub async fn run_workflow_yaml_with_custom_worker(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_workflow_yaml_with_custom_worker_and_events(
        workflow,
        workflow_input,
        executor,
        custom_worker,
        None,
    )
    .await
}

pub async fn run_email_workflow_yaml_with_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_custom_worker(workflow, &workflow_input, executor, custom_worker).await
}

pub async fn run_workflow_yaml_with_custom_worker_and_events(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_workflow_yaml_with_custom_worker_and_events_and_options(
        workflow,
        workflow_input,
        executor,
        custom_worker,
        event_sink,
        &YamlWorkflowRunOptions::default(),
    )
    .await
}

pub async fn run_workflow_yaml_with_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    if !workflow_input.is_object() {
        return Err(YamlWorkflowRunError::InvalidInput {
            message: "workflow input must be a JSON object".to_string(),
        });
    }

    let email_text = workflow_input
        .get("email_text")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let diagnostics = verify_yaml_workflow(workflow);
    let errors: Vec<YamlWorkflowDiagnostic> = diagnostics
        .iter()
        .filter(|d| d.severity == YamlWorkflowDiagnosticSeverity::Error)
        .cloned()
        .collect();
    if !errors.is_empty() {
        return Err(YamlWorkflowRunError::Validation {
            diagnostics_count: errors.len(),
            diagnostics: errors,
        });
    }

    if let Some(output) =
        try_run_yaml_via_ir_runtime(workflow, workflow_input, executor, custom_worker, options)
            .await?
    {
        return Ok(output);
    }

    let tracer = workflow_tracer();
    let parent_trace_context = trace_context_from_options(options);
    let (workflow_trace_context, mut workflow_span) = tracer.start_span(
        "workflow.run",
        SpanKind::Workflow,
        parent_trace_context.as_ref(),
    );
    let trace_id = if options.telemetry.enabled {
        let value = resolve_trace_id(options, &workflow_trace_context);
        workflow_span.set_attribute("trace_id", value.as_str());
        apply_trace_tenant_attributes(workflow_span.as_mut(), options);
        Some(value)
    } else {
        None
    };

    if workflow.nodes.is_empty() {
        return Err(YamlWorkflowRunError::EmptyNodes {
            workflow_id: workflow.id.clone(),
        });
    }

    let node_map: HashMap<&str, &YamlNode> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    if !node_map.contains_key(workflow.entry_node.as_str()) {
        return Err(YamlWorkflowRunError::MissingEntry {
            entry_node: workflow.entry_node.clone(),
        });
    }

    let edge_map: HashMap<&str, &str> = workflow
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();

    let mut current = workflow.entry_node.clone();
    let mut trace = Vec::new();
    let mut outputs: BTreeMap<String, Value> = BTreeMap::new();
    let mut globals = serde_json::Map::new();
    let mut step_timings = Vec::new();
    let mut llm_node_metrics: BTreeMap<String, YamlLlmNodeMetrics> = BTreeMap::new();
    let mut token_totals = YamlTokenTotals::default();
    let mut workflow_ttft_ms: Option<u128> = None;
    let started = Instant::now();

    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "workflow_started".to_string(),
            node_id: None,
            step_id: None,
            node_kind: None,
            streamable: None,
            message: Some(format!("workflow_id={}", workflow.id)),
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(0),
            metadata: None,
        });
    }

    if event_sink_is_cancelled(event_sink) {
        return Err(YamlWorkflowRunError::EventSinkCancelled {
            message: workflow_event_sink_cancelled_message().to_string(),
        });
    }

    loop {
        if event_sink_is_cancelled(event_sink) {
            return Err(YamlWorkflowRunError::EventSinkCancelled {
                message: workflow_event_sink_cancelled_message().to_string(),
            });
        }

        let node =
            *node_map
                .get(current.as_str())
                .ok_or_else(|| YamlWorkflowRunError::MissingNode {
                    node_id: current.clone(),
                })?;

        trace.push(node.id.clone());
        let step_started = Instant::now();

        let mut node_span = if options.telemetry.enabled {
            let (_, mut span) = tracer.start_span(
                "workflow.node.execute",
                SpanKind::Node,
                Some(&workflow_trace_context),
            );
            span.set_attribute("trace_id", trace_id.as_deref().unwrap_or_default());
            span.set_attribute("node_id", node.id.as_str());
            span.set_attribute("node_kind", node.kind_name());
            Some(span)
        } else {
            None
        };

        let node_streamable = node
            .node_type
            .llm_call
            .as_ref()
            .map(|llm| llm.stream.unwrap_or(false) && !llm.heal.unwrap_or(false));
        let workflow_elapsed_before_node_ms = started.elapsed().as_millis();

        if let Some(sink) = event_sink {
            sink.emit(&YamlWorkflowEvent {
                event_type: "node_started".to_string(),
                node_id: Some(node.id.clone()),
                step_id: Some(node.id.clone()),
                node_kind: Some(node.kind_name().to_string()),
                streamable: node_streamable,
                message: if node_streamable == Some(false) {
                    Some("Node is not streamable; status events only".to_string())
                } else {
                    None
                },
                delta: None,
                token_kind: None,
                is_terminal_node_token: None,
                elapsed_ms: Some(workflow_elapsed_before_node_ms),
                metadata: None,
            });
        }

        if event_sink_is_cancelled(event_sink) {
            return Err(YamlWorkflowRunError::EventSinkCancelled {
                message: workflow_event_sink_cancelled_message().to_string(),
            });
        }

        let mut node_usage: Option<YamlLlmTokenUsage> = None;
        let is_terminal_node = !edge_map.contains_key(node.id.as_str());
        let next = if let Some(llm) = &node.node_type.llm_call {
            let prompt_template = node
                .config
                .as_ref()
                .and_then(|cfg| cfg.prompt.as_deref())
                .unwrap_or_default();
            let context = json!({
                "input": workflow_input,
                "nodes": outputs,
                "globals": Value::Object(globals.clone())
            });
            let messages = if let Some(path) = llm.messages_path.as_deref() {
                Some(
                    parse_messages_from_context(path, &context).map_err(|message| {
                        YamlWorkflowRunError::Llm {
                            node_id: node.id.clone(),
                            message,
                        }
                    })?,
                )
            } else {
                None
            };
            let prompt_bindings = collect_template_bindings(prompt_template, &context);
            let prompt = interpolate_template(prompt_template, &context);
            let schema = llm_output_schema_for_node(node);

            let request = YamlLlmExecutionRequest {
                node_id: node.id.clone(),
                is_terminal_node,
                stream_json_as_text: llm.stream_json_as_text.unwrap_or(false),
                model: llm.model.clone(),
                messages,
                append_prompt_as_user: llm.append_prompt_as_user.unwrap_or(true),
                prompt,
                prompt_template: prompt_template.to_string(),
                prompt_bindings,
                schema,
                stream: llm.stream.unwrap_or(false),
                heal: llm.heal.unwrap_or(false),
            };

            if let Some(span) = node_span.as_mut() {
                span.set_attribute(
                    "node_input",
                    payload_for_span(options.telemetry.payload_mode, &context).as_str(),
                );
            }

            if let Some(sink) = event_sink {
                sink.emit(&YamlWorkflowEvent {
                    event_type: "node_llm_input_resolved".to_string(),
                    node_id: Some(node.id.clone()),
                    step_id: Some(node.id.clone()),
                    node_kind: Some("llm_call".to_string()),
                    streamable: Some(request.stream),
                    message: Some("resolved llm input for telemetry".to_string()),
                    delta: None,
                    token_kind: None,
                    is_terminal_node_token: None,
                    elapsed_ms: Some(started.elapsed().as_millis()),
                    metadata: Some(json!({
                        "model": request.model.clone(),
                        "stream_requested": request.stream,
                        "stream_json_as_text": request.stream_json_as_text,
                        "heal_requested": request.heal,
                        "effective_stream": request.stream,
                        "prompt_template": request.prompt_template.clone(),
                        "prompt": request.prompt.clone(),
                        "schema": request.schema.clone(),
                        "bindings": request.prompt_bindings.clone(),
                    })),
                });
            }

            if event_sink_is_cancelled(event_sink) {
                return Err(YamlWorkflowRunError::EventSinkCancelled {
                    message: workflow_event_sink_cancelled_message().to_string(),
                });
            }

            let llm_result = executor
                .complete_structured(request, event_sink)
                .await
                .map_err(|message| YamlWorkflowRunError::Llm {
                    node_id: node.id.clone(),
                    message,
                })?;

            if let Some(usage) = llm_result.usage.as_ref() {
                token_totals.add_usage(usage);
            }
            if workflow_ttft_ms.is_none() {
                workflow_ttft_ms = llm_result
                    .ttft_ms
                    .map(|node_ttft_ms| workflow_elapsed_before_node_ms + node_ttft_ms);
            }
            node_usage = llm_result.usage;

            let payload = llm_result.payload;

            if !payload.is_object() {
                return Err(YamlWorkflowRunError::LlmPayloadNotObject {
                    node_id: node.id.clone(),
                });
            }

            outputs.insert(node.id.clone(), json!({ "output": payload }));
            if let Some(span) = node_span.as_mut() {
                if let Some(output_payload) = outputs.get(node.id.as_str()) {
                    span.set_attribute(
                        "node_output",
                        payload_for_span(options.telemetry.payload_mode, output_payload).as_str(),
                    );
                }
            }
            apply_set_globals(node, &outputs, workflow_input, &mut globals);
            apply_update_globals(node, &outputs, workflow_input, &mut globals);
            edge_map
                .get(node.id.as_str())
                .map(|value| value.to_string())
        } else if let Some(switch) = &node.node_type.switch {
            let context = json!({
                "input": workflow_input,
                "nodes": outputs,
                "globals": Value::Object(globals.clone())
            });
            let mut chosen = Some(switch.default.clone());
            for branch in &switch.branches {
                if evaluate_switch_condition(branch.condition.as_str(), &context)? {
                    chosen = Some(branch.target.clone());
                    break;
                }
            }
            let chosen = chosen.ok_or_else(|| YamlWorkflowRunError::InvalidSwitchTarget {
                node_id: node.id.clone(),
            })?;
            Some(chosen)
        } else if let Some(custom) = &node.node_type.custom_worker {
            let payload = node
                .config
                .as_ref()
                .and_then(|cfg| cfg.payload.as_ref())
                .cloned()
                .unwrap_or_else(|| json!({}));
            let context = json!({
                "input": workflow_input,
                "nodes": outputs,
                "globals": Value::Object(globals.clone())
            });

            if let Some(span) = node_span.as_mut() {
                span.set_attribute("handler_name", custom.handler.as_str());
                span.set_attribute(
                    "node_input",
                    payload_for_span(options.telemetry.payload_mode, &payload).as_str(),
                );
            }

            let mut handler_span_context: Option<TraceContext> = None;
            let mut handler_span = if options.telemetry.enabled {
                let (span_context, mut span) = tracer.start_span(
                    "handler.invoke",
                    SpanKind::Node,
                    Some(&workflow_trace_context),
                );
                handler_span_context = Some(span_context);
                span.set_attribute("trace_id", trace_id.as_deref().unwrap_or_default());
                span.set_attribute("handler_name", custom.handler.as_str());
                apply_trace_tenant_attributes(span.as_mut(), options);
                Some(span)
            } else {
                None
            };

            let worker_trace_context = merged_trace_context_for_worker(
                handler_span_context.as_ref(),
                trace_id.as_deref(),
                options,
            );
            let worker_context = custom_worker_context_with_trace(
                &context,
                &worker_trace_context,
                &options.trace.tenant,
            );

            let worker_output_result = if let Some(custom_worker_executor) = custom_worker {
                custom_worker_executor
                    .execute(
                        custom.handler.as_str(),
                        &payload,
                        email_text,
                        &worker_context,
                    )
                    .await
                    .map_err(|message| YamlWorkflowRunError::CustomWorker {
                        node_id: node.id.clone(),
                        message,
                    })
            } else {
                mock_custom_worker_output(custom.handler.as_str(), &payload)
            };

            if let Some(span) = handler_span.take() {
                span.end();
            }

            let worker_output = worker_output_result?;

            outputs.insert(node.id.clone(), json!({ "output": worker_output }));
            if let Some(span) = node_span.as_mut() {
                if let Some(output_payload) = outputs.get(node.id.as_str()) {
                    span.set_attribute(
                        "node_output",
                        payload_for_span(options.telemetry.payload_mode, output_payload).as_str(),
                    );
                }
            }
            apply_set_globals(node, &outputs, workflow_input, &mut globals);
            apply_update_globals(node, &outputs, workflow_input, &mut globals);
            edge_map
                .get(node.id.as_str())
                .map(|value| value.to_string())
        } else {
            return Err(YamlWorkflowRunError::UnsupportedNodeType {
                node_id: node.id.clone(),
            });
        };

        let node_kind = node.kind_name().to_string();
        let elapsed_ms = step_started.elapsed().as_millis();
        step_timings.push(YamlStepTiming {
            node_id: node.id.clone(),
            node_kind,
            elapsed_ms,
            prompt_tokens: node_usage.as_ref().map(|usage| usage.prompt_tokens),
            completion_tokens: node_usage.as_ref().map(|usage| usage.completion_tokens),
            total_tokens: node_usage.as_ref().map(|usage| usage.total_tokens),
            thinking_tokens: node_usage.as_ref().and_then(|usage| usage.thinking_tokens),
            tokens_per_second: node_usage
                .as_ref()
                .map(|usage| completion_tokens_per_second(usage.completion_tokens, elapsed_ms)),
        });

        if let Some(usage) = node_usage.as_ref() {
            llm_node_metrics.insert(
                node.id.clone(),
                YamlLlmNodeMetrics {
                    elapsed_ms,
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                    total_tokens: usage.total_tokens,
                    thinking_tokens: usage.thinking_tokens,
                    tokens_per_second: completion_tokens_per_second(
                        usage.completion_tokens,
                        elapsed_ms,
                    ),
                },
            );
        }

        if let Some(mut span) = node_span.take() {
            span.set_attribute("elapsed_ms", elapsed_ms.to_string().as_str());
            span.add_event("node_completed");
            span.end();
        }

        if let Some(sink) = event_sink {
            sink.emit(&YamlWorkflowEvent {
                event_type: "node_completed".to_string(),
                node_id: Some(node.id.clone()),
                step_id: Some(node.id.clone()),
                node_kind: Some(node.kind_name().to_string()),
                streamable: node_streamable,
                message: None,
                delta: None,
                token_kind: None,
                is_terminal_node_token: None,
                elapsed_ms: Some(elapsed_ms),
                metadata: None,
            });
        }

        if event_sink_is_cancelled(event_sink) {
            return Err(YamlWorkflowRunError::EventSinkCancelled {
                message: workflow_event_sink_cancelled_message().to_string(),
            });
        }

        if let Some(next) = next {
            current = next;
            continue;
        }
        break;
    }

    let terminal_node = trace
        .last()
        .cloned()
        .ok_or_else(|| YamlWorkflowRunError::EmptyNodes {
            workflow_id: workflow.id.clone(),
        })?;

    let terminal_output = outputs
        .get(terminal_node.as_str())
        .and_then(|value| value.get("output"))
        .cloned();

    let total_elapsed_ms = started.elapsed().as_millis();
    let output = YamlWorkflowRunOutput {
        workflow_id: workflow.id.clone(),
        entry_node: workflow.entry_node.clone(),
        email_text: email_text.to_string(),
        trace,
        outputs,
        terminal_node,
        terminal_output,
        step_timings,
        llm_node_metrics,
        total_elapsed_ms,
        ttft_ms: workflow_ttft_ms,
        total_input_tokens: token_totals.input_tokens,
        total_output_tokens: token_totals.output_tokens,
        total_tokens: token_totals.total_tokens,
        total_thinking_tokens: token_totals.thinking_tokens,
        tokens_per_second: token_totals.tokens_per_second(total_elapsed_ms),
        trace_id: trace_id.clone(),
        metadata: trace_id
            .as_ref()
            .map(|value| workflow_metadata_with_trace(options, value)),
    };

    if let Some(sink) = event_sink {
        let event_metadata = if options.telemetry.nerdstats {
            Some(json!({
                "nerdstats": workflow_nerdstats(&output),
            }))
        } else {
            None
        };
        sink.emit(&YamlWorkflowEvent {
            event_type: "workflow_completed".to_string(),
            node_id: None,
            step_id: None,
            node_kind: None,
            streamable: None,
            message: Some(format!("terminal_node={}", output.terminal_node)),
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(output.total_elapsed_ms),
            metadata: event_metadata,
        });
    }

    if event_sink_is_cancelled(event_sink) {
        return Err(YamlWorkflowRunError::EventSinkCancelled {
            message: workflow_event_sink_cancelled_message().to_string(),
        });
    }

    workflow_span.set_attribute("workflow_id", workflow.id.as_str());
    if let Some(trace_id_value) = trace_id.as_ref() {
        workflow_span.set_attribute("trace_id", trace_id_value.as_str());
    }
    workflow_span.end();

    Ok(output)
}

async fn try_run_yaml_via_ir_runtime(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    options: &YamlWorkflowRunOptions,
) -> Result<Option<YamlWorkflowRunOutput>, YamlWorkflowRunError> {
    let ir = match yaml_workflow_to_ir(workflow) {
        Ok(def) => def,
        Err(YamlToIrError::UnsupportedNode { .. })
        | Err(YamlToIrError::MultipleOutgoingEdge { .. }) => return Ok(None),
        Err(err) => {
            return Err(YamlWorkflowRunError::InvalidInput {
                message: err.to_string(),
            });
        }
    };

    let tracer = workflow_tracer();
    let parent_trace_context = trace_context_from_options(options);
    let (workflow_trace_context, mut workflow_span) = tracer.start_span(
        "workflow.run",
        SpanKind::Workflow,
        parent_trace_context.as_ref(),
    );
    let trace_id = if options.telemetry.enabled {
        let value = resolve_trace_id(options, &workflow_trace_context);
        workflow_span.set_attribute("trace_id", value.as_str());
        apply_trace_tenant_attributes(workflow_span.as_mut(), options);
        Some(value)
    } else {
        None
    };

    struct NoopLlm;
    #[async_trait]
    impl LlmExecutor for NoopLlm {
        async fn execute(
            &self,
            _input: LlmExecutionInput,
        ) -> Result<LlmExecutionOutput, LlmExecutionError> {
            Err(LlmExecutionError::UnexpectedOutcome(
                "yaml_ir_uses_tool_path",
            ))
        }
    }

    struct YamlIrToolExecutor<'a> {
        llm_executor: &'a dyn YamlWorkflowLlmExecutor,
        custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
        token_totals: std::sync::Mutex<YamlTokenTotals>,
        node_usage: std::sync::Mutex<BTreeMap<String, YamlLlmTokenUsage>>,
        trace_id: Option<String>,
        trace_context: Option<TraceContext>,
        trace_input_context: Option<YamlWorkflowTraceContextInput>,
        tenant_context: YamlWorkflowTraceTenantContext,
        payload_mode: YamlWorkflowPayloadMode,
    }

    #[async_trait]
    impl ToolExecutor for YamlIrToolExecutor<'_> {
        async fn execute_tool(
            &self,
            input: ToolExecutionInput,
        ) -> Result<Value, ToolExecutionError> {
            let context = build_yaml_context_from_ir_scope(&input.scoped_input);

            if input.tool == YAML_LLM_TOOL_ID {
                let node_id = input
                    .input
                    .get("node_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ToolExecutionError::Failed("yaml llm call missing node_id".to_string())
                    })?
                    .to_string();
                let node_id_for_metrics = node_id.clone();
                let model = input
                    .input
                    .get("model")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ToolExecutionError::Failed("yaml llm call missing model".to_string())
                    })?
                    .to_string();
                let prompt_template = input
                    .input
                    .get("prompt_template")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let stream = input
                    .input
                    .get("stream")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let heal = input
                    .input
                    .get("heal")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let append_prompt_as_user = input
                    .input
                    .get("append_prompt_as_user")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let messages_path = input
                    .input
                    .get("messages_path")
                    .and_then(Value::as_str)
                    .map(str::to_string);

                let messages = if let Some(path) = messages_path.as_deref() {
                    Some(
                        parse_messages_from_context(path, &context)
                            .map_err(ToolExecutionError::Failed)?,
                    )
                } else {
                    None
                };

                let prompt_bindings = collect_template_bindings(&prompt_template, &context);
                let prompt = interpolate_template(&prompt_template, &context);
                let schema = input
                    .input
                    .get("output_schema")
                    .cloned()
                    .unwrap_or_else(default_llm_output_schema);

                let request = YamlLlmExecutionRequest {
                    node_id,
                    is_terminal_node: false,
                    stream_json_as_text: input
                        .input
                        .get("stream_json_as_text")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    model,
                    messages,
                    append_prompt_as_user,
                    prompt,
                    prompt_template,
                    prompt_bindings,
                    schema,
                    stream,
                    heal,
                };

                let llm_result = self
                    .llm_executor
                    .complete_structured(request, None)
                    .await
                    .map_err(ToolExecutionError::Failed);

                if let Ok(ref result) = llm_result {
                    if let Some(usage) = result.usage.as_ref() {
                        if let Ok(mut totals) = self.token_totals.lock() {
                            totals.add_usage(usage);
                        }
                        if let Ok(mut usage_map) = self.node_usage.lock() {
                            usage_map.insert(node_id_for_metrics, usage.clone());
                        }
                    }
                }

                return llm_result.map(|result| result.payload);
            }

            let worker = self
                .custom_worker
                .ok_or_else(|| ToolExecutionError::NotFound {
                    tool: input.tool.clone(),
                })?;

            let payload = input.input.clone();
            let email_text = context
                .get("input")
                .and_then(|v| v.get("email_text"))
                .and_then(Value::as_str)
                .unwrap_or_default();

            let tracer = workflow_tracer();
            let mut handler_span_context: Option<TraceContext> = None;
            let mut handler_span = if self.trace_id.is_some() {
                let (span_context, mut span) = tracer.start_span(
                    "handler.invoke",
                    SpanKind::Node,
                    self.trace_context.as_ref(),
                );
                handler_span_context = Some(span_context);
                if let Some(trace_id) = self.trace_id.as_ref() {
                    span.set_attribute("trace_id", trace_id.as_str());
                }
                span.set_attribute("handler_name", input.tool.as_str());
                if let Some(workspace_id) = self.tenant_context.workspace_id.as_deref() {
                    span.set_attribute("tenant.workspace_id", workspace_id);
                }
                if let Some(user_id) = self.tenant_context.user_id.as_deref() {
                    span.set_attribute("tenant.user_id", user_id);
                }
                if let Some(conversation_id) = self.tenant_context.conversation_id.as_deref() {
                    span.set_attribute("tenant.conversation_id", conversation_id);
                }
                if let Some(request_id) = self.tenant_context.request_id.as_deref() {
                    span.set_attribute("tenant.request_id", request_id);
                }
                if let Some(run_id) = self.tenant_context.run_id.as_deref() {
                    span.set_attribute("tenant.run_id", run_id);
                }
                span.set_attribute(
                    "node_input",
                    payload_for_span(self.payload_mode, &payload).as_str(),
                );
                Some(span)
            } else {
                None
            };

            let trace_options = YamlWorkflowRunOptions {
                telemetry: YamlWorkflowTelemetryConfig::default(),
                trace: YamlWorkflowTraceOptions {
                    context: self.trace_input_context.clone(),
                    tenant: self.tenant_context.clone(),
                },
            };
            let worker_trace_context = merged_trace_context_for_worker(
                handler_span_context.as_ref(),
                self.trace_id.as_deref(),
                &trace_options,
            );
            let worker_context = custom_worker_context_with_trace(
                &context,
                &worker_trace_context,
                &self.tenant_context,
            );

            let output_result = worker
                .execute(&input.tool, &payload, email_text, &worker_context)
                .await
                .map_err(ToolExecutionError::Failed);

            if let Some(span) = handler_span.as_mut() {
                if output_result.is_ok() {
                    span.add_event("handler.success");
                } else {
                    span.add_event("handler.error");
                }
            }

            if let Some(span) = handler_span.take() {
                span.end();
            }

            output_result
        }
    }

    let tool_executor = YamlIrToolExecutor {
        llm_executor: executor,
        custom_worker,
        token_totals: std::sync::Mutex::new(YamlTokenTotals::default()),
        node_usage: std::sync::Mutex::new(BTreeMap::new()),
        trace_id: trace_id.clone(),
        trace_context: trace_id.as_ref().map(|_| workflow_trace_context.clone()),
        trace_input_context: options.trace.context.clone(),
        tenant_context: options.trace.tenant.clone(),
        payload_mode: options.telemetry.payload_mode,
    };

    let runtime = WorkflowRuntime::new(
        ir,
        &NoopLlm,
        Some(&tool_executor),
        WorkflowRuntimeOptions::default(),
    );

    let started = Instant::now();
    let result = match runtime.execute(workflow_input.clone(), None).await {
        Ok(result) => result,
        Err(WorkflowRuntimeError::Validation(_)) => return Ok(None),
        Err(error) => {
            return Err(YamlWorkflowRunError::IrRuntime {
                message: error.to_string(),
            });
        }
    };
    let total_elapsed_ms = started.elapsed().as_millis();

    let mut outputs: BTreeMap<String, Value> = BTreeMap::new();
    for (node_id, output) in result.node_outputs {
        if node_id == YAML_START_NODE_ID {
            continue;
        }
        outputs.insert(node_id, json!({"output": output}));
    }

    let mut trace = Vec::new();
    let mut step_timings = Vec::new();
    let node_usage_map = tool_executor
        .node_usage
        .lock()
        .map(|usage| usage.clone())
        .unwrap_or_default();
    let mut llm_node_metrics: BTreeMap<String, YamlLlmNodeMetrics> = BTreeMap::new();
    for execution in result.node_executions {
        if execution.node_id == YAML_START_NODE_ID {
            continue;
        }
        trace.push(execution.node_id.clone());
        let usage = node_usage_map.get(&execution.node_id);
        if let Some(usage) = usage {
            llm_node_metrics.insert(
                execution.node_id.clone(),
                YamlLlmNodeMetrics {
                    elapsed_ms: 0,
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                    total_tokens: usage.total_tokens,
                    thinking_tokens: usage.thinking_tokens,
                    tokens_per_second: completion_tokens_per_second(usage.completion_tokens, 0),
                },
            );
        }
        step_timings.push(YamlStepTiming {
            node_id: execution.node_id,
            node_kind: "ir_runtime".to_string(),
            elapsed_ms: 0,
            prompt_tokens: usage.map(|value| value.prompt_tokens),
            completion_tokens: usage.map(|value| value.completion_tokens),
            total_tokens: usage.map(|value| value.total_tokens),
            thinking_tokens: usage.and_then(|value| value.thinking_tokens),
            tokens_per_second: usage
                .map(|value| completion_tokens_per_second(value.completion_tokens, 0)),
        });
    }

    let terminal_node = result.terminal_node_id;
    let terminal_output = outputs
        .get(&terminal_node)
        .and_then(|v| v.get("output"))
        .cloned();

    let email_text = workflow_input
        .get("email_text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let token_totals = tool_executor
        .token_totals
        .lock()
        .map(|totals| totals.clone())
        .unwrap_or_default();

    workflow_span.set_attribute("workflow_id", workflow.id.as_str());
    workflow_span.end();

    Ok(Some(YamlWorkflowRunOutput {
        workflow_id: workflow.id.clone(),
        entry_node: workflow.entry_node.clone(),
        email_text,
        trace,
        outputs,
        terminal_node,
        terminal_output,
        step_timings,
        llm_node_metrics,
        total_elapsed_ms,
        ttft_ms: None,
        total_input_tokens: token_totals.input_tokens,
        total_output_tokens: token_totals.output_tokens,
        total_tokens: token_totals.total_tokens,
        total_thinking_tokens: token_totals.thinking_tokens,
        tokens_per_second: token_totals.tokens_per_second(total_elapsed_ms),
        trace_id: trace_id.clone(),
        metadata: trace_id
            .as_ref()
            .map(|value| workflow_metadata_with_trace(options, value)),
    }))
}

fn build_yaml_context_from_ir_scope(scoped_input: &Value) -> Value {
    let input = scoped_input.get("input").cloned().unwrap_or(Value::Null);

    let mut nodes = serde_json::Map::new();
    if let Some(node_outputs) = scoped_input.get("node_outputs").and_then(Value::as_object) {
        for (node_id, output) in node_outputs {
            nodes.insert(node_id.clone(), json!({"output": output.clone()}));
        }
    }

    json!({
        "input": input,
        "nodes": Value::Object(nodes),
        "globals": Value::Object(serde_json::Map::new())
    })
}

pub async fn run_email_workflow_yaml_with_custom_worker_and_events(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_custom_worker_and_events(
        workflow,
        &workflow_input,
        executor,
        custom_worker,
        event_sink,
    )
    .await
}

fn evaluate_switch_condition(
    condition: &str,
    context: &Value,
) -> Result<bool, YamlWorkflowRunError> {
    let (left, right) =
        condition
            .split_once("==")
            .ok_or_else(|| YamlWorkflowRunError::UnsupportedCondition {
                condition: condition.to_string(),
            })?;

    let left_path = left.trim().trim_start_matches("$.");
    let right_literal = right.trim().trim_matches('"').trim_matches('\'');
    let left_value = resolve_path(context, left_path);
    Ok(left_value
        .and_then(Value::as_str)
        .map(|value| value == right_literal)
        .unwrap_or(false))
}

fn parse_messages_from_context(path: &str, context: &Value) -> Result<Vec<Message>, String> {
    let normalized_path = path.trim().trim_start_matches("$.");
    let value = resolve_path(context, normalized_path)
        .ok_or_else(|| format!("messages_path not found: {path}"))?;
    let list: Vec<WorkflowMessage> = serde_json::from_value(value.clone()).map_err(|err| {
        format!("messages_path must resolve to a list of messages: {path}; {err}")
    })?;
    if list.is_empty() {
        return Err(format!(
            "messages_path must not resolve to an empty list: {path}"
        ));
    }

    let mut messages = Vec::with_capacity(list.len());
    for (index, item) in list.into_iter().enumerate() {
        let mut message = match item.role {
            Role::System => Message::system(item.content),
            Role::User => Message::user(item.content),
            Role::Assistant => Message::assistant(item.content),
            Role::Tool => {
                let tool_call_id = item
                    .tool_call_id
                    .ok_or_else(|| format!("tool message at index {index} missing tool_call_id"))?;
                Message::tool(item.content, tool_call_id)
            }
        };

        if let Some(name) = item.name {
            message = message.with_name(name);
        }

        messages.push(message);
    }

    Ok(messages)
}

pub fn verify_yaml_workflow(workflow: &YamlWorkflow) -> Vec<YamlWorkflowDiagnostic> {
    let mut diagnostics = Vec::new();
    let known_ids: HashMap<&str, &YamlNode> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    if !known_ids.contains_key(workflow.entry_node.as_str()) {
        diagnostics.push(YamlWorkflowDiagnostic {
            node_id: None,
            code: "missing_entry".to_string(),
            severity: YamlWorkflowDiagnosticSeverity::Error,
            message: format!("entry node '{}' does not exist", workflow.entry_node),
        });
    }

    for edge in &workflow.edges {
        if !known_ids.contains_key(edge.from.as_str()) {
            diagnostics.push(YamlWorkflowDiagnostic {
                node_id: Some(edge.from.clone()),
                code: "unknown_edge_from".to_string(),
                severity: YamlWorkflowDiagnosticSeverity::Error,
                message: format!("edge.from '{}' does not exist", edge.from),
            });
        }
        if !known_ids.contains_key(edge.to.as_str()) {
            diagnostics.push(YamlWorkflowDiagnostic {
                node_id: Some(edge.to.clone()),
                code: "unknown_edge_to".to_string(),
                severity: YamlWorkflowDiagnosticSeverity::Error,
                message: format!("edge.to '{}' does not exist", edge.to),
            });
        }
    }

    for node in &workflow.nodes {
        if let Some(llm) = &node.node_type.llm_call {
            if llm.model.trim().is_empty() {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "empty_model".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Error,
                    message: "llm_call.model must not be empty".to_string(),
                });
            }
            if llm.stream.unwrap_or(false) && llm.heal.unwrap_or(false) {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "stream_heal_conflict".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Warning,
                    message:
                        "llm_call.stream=true with heal=true is not streamable; runtime will disable streaming"
                            .to_string(),
                });
            }
        }

        if let Some(switch) = &node.node_type.switch {
            for branch in &switch.branches {
                if !known_ids.contains_key(branch.target.as_str()) {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "unknown_switch_target".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: format!("switch branch target '{}' does not exist", branch.target),
                    });
                }
            }
            if !known_ids.contains_key(switch.default.as_str()) {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "unknown_switch_default".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Error,
                    message: format!("switch default target '{}' does not exist", switch.default),
                });
            }
        }

        if let Some(config) = node.config.as_ref() {
            if let Some(update_globals) = config.update_globals.as_ref() {
                for (key, update) in update_globals {
                    let is_valid_op =
                        matches!(update.op.as_str(), "set" | "append" | "increment" | "merge");
                    if !is_valid_op {
                        diagnostics.push(YamlWorkflowDiagnostic {
                            node_id: Some(node.id.clone()),
                            code: "unknown_update_op".to_string(),
                            severity: YamlWorkflowDiagnosticSeverity::Error,
                            message: format!(
                                "update_globals key '{}' has unknown op '{}'; expected set|append|increment|merge",
                                key, update.op
                            ),
                        });
                    }

                    if update.op != "increment" && update.from.is_none() {
                        diagnostics.push(YamlWorkflowDiagnostic {
                            node_id: Some(node.id.clone()),
                            code: "missing_update_from".to_string(),
                            severity: YamlWorkflowDiagnosticSeverity::Error,
                            message: format!(
                                "update_globals key '{}' with op '{}' requires 'from'",
                                key, update.op
                            ),
                        });
                    }
                }
            }
        }
    }

    diagnostics
}

fn resolve_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .try_fold(value, |current, segment| current.get(segment))
}

fn interpolate_template(template: &str, context: &Value) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    loop {
        let Some(start) = rest.find("{{") else {
            out.push_str(rest);
            break;
        };

        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            out.push_str(&rest[start..]);
            break;
        };

        let expr = after_start[..end].trim();
        let source_path = expr.trim_start_matches("$.");
        let replacement = resolve_path(context, source_path)
            .map(value_to_template_string)
            .unwrap_or_default();
        out.push_str(replacement.as_str());

        rest = &after_start[end + 2..];
    }

    out
}

fn collect_template_bindings(template: &str, context: &Value) -> Vec<YamlTemplateBinding> {
    let mut bindings = Vec::new();
    let mut rest = template;

    loop {
        let Some(start) = rest.find("{{") else {
            break;
        };

        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };

        let expr = after_start[..end].trim();
        let source_path = expr.trim_start_matches("$.").to_string();
        let resolved = resolve_path(context, source_path.as_str()).cloned();
        let missing = resolved.is_none();
        let resolved_value = resolved.unwrap_or(Value::Null);
        bindings.push(YamlTemplateBinding {
            index: bindings.len(),
            expression: expr.to_string(),
            source_path,
            resolved_type: json_type_name(&resolved_value).to_string(),
            missing,
            resolved: resolved_value,
        });

        rest = &after_start[end + 2..];
    }

    bindings
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn value_to_template_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn apply_set_globals(
    node: &YamlNode,
    outputs: &BTreeMap<String, Value>,
    workflow_input: &Value,
    globals: &mut serde_json::Map<String, Value>,
) {
    let Some(config) = node.config.as_ref() else {
        return;
    };
    let Some(set_globals) = config.set_globals.as_ref() else {
        return;
    };

    let context = json!({
        "input": workflow_input,
        "nodes": outputs,
        "globals": Value::Object(globals.clone())
    });

    for (key, expr) in set_globals {
        let value = resolve_path(&context, expr.as_str())
            .cloned()
            .unwrap_or(Value::Null);
        globals.insert(key.clone(), value);
    }
}

fn apply_update_globals(
    node: &YamlNode,
    outputs: &BTreeMap<String, Value>,
    workflow_input: &Value,
    globals: &mut serde_json::Map<String, Value>,
) {
    let Some(config) = node.config.as_ref() else {
        return;
    };
    let Some(update_globals) = config.update_globals.as_ref() else {
        return;
    };

    let context = json!({
        "input": workflow_input,
        "nodes": outputs,
        "globals": Value::Object(globals.clone())
    });

    for (key, update) in update_globals {
        match update.op.as_str() {
            "set" => {
                if let Some(path) = update.from.as_ref() {
                    let value = resolve_path(&context, path.as_str())
                        .cloned()
                        .unwrap_or(Value::Null);
                    globals.insert(key.clone(), value);
                }
            }
            "append" => {
                if let Some(path) = update.from.as_ref() {
                    let value = resolve_path(&context, path.as_str())
                        .cloned()
                        .unwrap_or(Value::Null);
                    let entry = globals
                        .entry(key.clone())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    match entry {
                        Value::Array(items) => items.push(value),
                        other => {
                            let existing = other.clone();
                            *other = Value::Array(vec![existing, value]);
                        }
                    }
                }
            }
            "increment" => {
                let by = update.by.unwrap_or(1.0);
                let current = globals
                    .get(key.as_str())
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                if let Some(next) = serde_json::Number::from_f64(current + by) {
                    globals.insert(key.clone(), Value::Number(next));
                }
            }
            "merge" => {
                if let Some(path) = update.from.as_ref() {
                    let source = resolve_path(&context, path.as_str())
                        .cloned()
                        .unwrap_or(Value::Null);
                    if let Value::Object(source_map) = source {
                        let target = globals
                            .entry(key.clone())
                            .or_insert_with(|| Value::Object(serde_json::Map::new()));
                        if let Value::Object(target_map) = target {
                            target_map.extend(source_map);
                        } else {
                            *target = Value::Object(source_map);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn llm_output_schema_for_node(node: &YamlNode) -> Value {
    if let Some(schema) = node
        .config
        .as_ref()
        .and_then(|cfg| cfg.output_schema.clone())
    {
        return schema;
    }

    default_llm_output_schema()
}

fn default_llm_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true
    })
}

fn mock_rag(topic: &str) -> Value {
    let (kb_source, playbook) = match topic {
        "probation" => (
            "hr_policy/probation.md",
            "Collect manager review, performance evidence, and probation timeline.",
        ),
        "leave_request" => (
            "hr_policy/leave.md",
            "Validate leave balance, manager approval, and blackout dates.",
        ),
        "supply_chain_order_assessment" => (
            "supply_chain/order_assessment.md",
            "Review order specs, inventory risk, and vendor lead-time guidance.",
        ),
        "supply_chain_order_replacement" => (
            "supply_chain/order_replacement.md",
            "Collect order id, damage proof, and replacement SLA policy.",
        ),
        "termination_first_time_offense" => (
            "hr_policy/termination_first_offense.md",
            "Validate first-incident criteria and route to HRBP review.",
        ),
        "termination_repeated_offense" => (
            "hr_policy/termination_repeated_offense.md",
            "Collect prior warnings and escalation approvals before final action.",
        ),
        _ => (
            "shared/request_clarification.md",
            "Request clarifying details before routing.",
        ),
    };

    json!({
        "kb_source": kb_source,
        "playbook": playbook,
    })
}

fn mock_custom_worker_output(
    handler: &str,
    payload: &Value,
) -> Result<Value, YamlWorkflowRunError> {
    if let Some(topic) = payload.get("topic").and_then(Value::as_str) {
        let mut value = mock_rag(topic);
        if let Value::Object(object) = &mut value {
            object.insert("handler".to_string(), Value::String(handler.to_string()));
        }
        return Ok(value);
    }

    Err(YamlWorkflowRunError::UnsupportedCustomHandler {
        handler: handler.to_string(),
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlWorkflow {
    pub id: String,
    pub entry_node: String,
    #[serde(default)]
    pub nodes: Vec<YamlNode>,
    #[serde(default)]
    pub edges: Vec<YamlEdge>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNode {
    pub id: String,
    pub node_type: YamlNodeType,
    pub config: Option<YamlNodeConfig>,
}

impl YamlNode {
    fn kind_name(&self) -> &'static str {
        if self.node_type.llm_call.is_some() {
            "llm_call"
        } else if self.node_type.switch.is_some() {
            "switch"
        } else if self.node_type.custom_worker.is_some() {
            "custom_worker"
        } else {
            "unknown"
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNodeType {
    pub llm_call: Option<YamlLlmCall>,
    pub switch: Option<YamlSwitch>,
    pub custom_worker: Option<YamlCustomWorker>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlLlmCall {
    pub model: String,
    pub stream: Option<bool>,
    pub stream_json_as_text: Option<bool>,
    pub heal: Option<bool>,
    pub messages_path: Option<String>,
    pub append_prompt_as_user: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlSwitch {
    #[serde(default)]
    pub branches: Vec<YamlSwitchBranch>,
    pub default: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlSwitchBranch {
    pub condition: String,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlCustomWorker {
    pub handler: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNodeConfig {
    pub prompt: Option<String>,
    #[serde(default, alias = "schema")]
    pub output_schema: Option<Value>,
    pub payload: Option<Value>,
    pub set_globals: Option<HashMap<String, String>>,
    pub update_globals: Option<HashMap<String, YamlGlobalUpdate>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlGlobalUpdate {
    pub op: String,
    pub from: Option<String>,
    pub by: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlEdge {
    pub from: String,
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    fn stream_debug_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct MockExecutor;

    struct RecordingSink {
        events: Mutex<Vec<YamlWorkflowEvent>>,
    }

    struct CancelAfterFirstEventSink {
        cancelled: AtomicBool,
    }

    impl YamlWorkflowEventSink for CancelAfterFirstEventSink {
        fn emit(&self, _event: &YamlWorkflowEvent) {
            self.cancelled.store(true, Ordering::SeqCst);
        }

        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::SeqCst)
        }
    }

    struct CountingExecutor {
        call_count: AtomicUsize,
    }

    struct CapturingWorker {
        context: Mutex<Option<Value>>,
    }

    #[async_trait]
    impl YamlWorkflowLlmExecutor for CountingExecutor {
        async fn complete_structured(
            &self,
            _request: YamlLlmExecutionRequest,
            _event_sink: Option<&dyn YamlWorkflowEventSink>,
        ) -> Result<YamlLlmExecutionResult, String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(YamlLlmExecutionResult {
                payload: json!({"state":"ok"}),
                usage: None,
                ttft_ms: None,
            })
        }
    }

    impl YamlWorkflowEventSink for RecordingSink {
        fn emit(&self, event: &YamlWorkflowEvent) {
            self.events
                .lock()
                .expect("recording sink lock should not be poisoned")
                .push(event.clone());
        }
    }

    #[async_trait]
    impl YamlWorkflowCustomWorkerExecutor for CapturingWorker {
        async fn execute(
            &self,
            _handler: &str,
            _payload: &Value,
            _email_text: &str,
            context: &Value,
        ) -> Result<Value, String> {
            let mut guard = self
                .context
                .lock()
                .map_err(|_| "capturing worker lock should not be poisoned".to_string())?;
            *guard = Some(context.clone());
            Ok(json!({"ok": true}))
        }
    }

    #[async_trait]
    impl YamlWorkflowLlmExecutor for MockExecutor {
        async fn complete_structured(
            &self,
            request: YamlLlmExecutionRequest,
            _event_sink: Option<&dyn YamlWorkflowEventSink>,
        ) -> Result<YamlLlmExecutionResult, String> {
            let prompt = request.prompt;
            if prompt.contains("exactly one category") {
                return Ok(YamlLlmExecutionResult {
                    payload: json!({"category":"termination","reason":"mock"}),
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: 10,
                        completion_tokens: 5,
                        total_tokens: 15,
                        thinking_tokens: None,
                    }),
                    ttft_ms: None,
                });
            }
            if prompt.contains("Determine termination subtype") {
                return Ok(YamlLlmExecutionResult {
                    payload: json!({"subtype":"repeated_offense","reason":"mock"}),
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: 12,
                        completion_tokens: 6,
                        total_tokens: 18,
                        thinking_tokens: None,
                    }),
                    ttft_ms: None,
                });
            }
            if prompt.contains("Determine supply chain subtype") {
                return Ok(YamlLlmExecutionResult {
                    payload: json!({"subtype":"order_replacement","reason":"mock"}),
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: 11,
                        completion_tokens: 4,
                        total_tokens: 15,
                        thinking_tokens: None,
                    }),
                    ttft_ms: None,
                });
            }
            Err("unexpected prompt".to_string())
        }
    }

    #[tokio::test]
    async fn runs_yaml_workflow_and_returns_step_timings() {
        let yaml = r#"
id: email-intake-classification
entry_node: classify_top_level
nodes:
  - id: classify_top_level
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
  - id: route_top_level
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify_top_level.output.category == "termination"'
            target: classify_termination_subtype
        default: rag_clarification
  - id: classify_termination_subtype
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Determine termination subtype:
        {{ input.email_text }}
  - id: route_termination_subtype
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify_termination_subtype.output.subtype == "repeated_offense"'
            target: rag_termination_repeated_offense
        default: rag_clarification
  - id: rag_termination_repeated_offense
    node_type:
      custom_worker:
        handler: GetRagData
    config:
      payload:
        topic: termination_repeated_offense
  - id: rag_clarification
    node_type:
      custom_worker:
        handler: GetRagData
    config:
      payload:
        topic: clarification
edges:
  - from: classify_top_level
    to: route_top_level
  - from: classify_termination_subtype
    to: route_termination_subtype
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let output = run_email_workflow_yaml(&workflow, "test", &MockExecutor)
            .await
            .expect("yaml workflow should execute");

        assert_eq!(output.workflow_id, "email-intake-classification");
        assert_eq!(output.terminal_node, "rag_termination_repeated_offense");
        assert!(!output.step_timings.is_empty());
        assert_eq!(output.step_timings.len(), output.trace.len());
        assert!(output
            .outputs
            .contains_key("rag_termination_repeated_offense"));
        assert_eq!(output.total_input_tokens, 22);
        assert_eq!(output.total_output_tokens, 11);
        assert_eq!(output.total_tokens, 33);
    }

    #[tokio::test]
    async fn emits_resolved_llm_input_event_with_bindings() {
        let yaml = r#"
id: email-intake-classification
entry_node: classify_top_level
nodes:
  - id: classify_top_level
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let sink = RecordingSink {
            events: Mutex::new(Vec::new()),
        };

        let output = run_email_workflow_yaml_with_custom_worker_and_events(
            &workflow,
            "Need help with termination",
            &MockExecutor,
            None,
            Some(&sink),
        )
        .await
        .expect("yaml workflow should execute");

        assert_eq!(output.terminal_node, "classify_top_level");

        let events = sink
            .events
            .lock()
            .expect("recording sink lock should not be poisoned");
        let llm_event = events
            .iter()
            .find(|event| event.event_type == "node_llm_input_resolved")
            .expect("expected llm input telemetry event");

        let metadata = llm_event
            .metadata
            .as_ref()
            .expect("llm input event must include metadata");
        assert_eq!(metadata["model"], Value::String("gpt-4.1".to_string()));
        assert_eq!(metadata["stream_requested"], Value::Bool(false));
        assert_eq!(metadata["heal_requested"], Value::Bool(false));
        assert!(metadata["prompt"]
            .as_str()
            .expect("prompt should be a string")
            .contains("Need help with termination"));

        let bindings = metadata["bindings"]
            .as_array()
            .expect("bindings should be an array");
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings[0]["source_path"],
            Value::String("input.email_text".to_string())
        );
        assert_eq!(
            bindings[0]["resolved"],
            Value::String("Need help with termination".to_string())
        );
        assert_eq!(bindings[0]["missing"], Value::Bool(false));
        assert_eq!(
            bindings[0]["resolved_type"],
            Value::String("string".to_string())
        );
    }

    #[tokio::test]
    async fn workflow_completed_event_includes_nerdstats_by_default() {
        let yaml = r#"
id: nerdstats-default
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let sink = RecordingSink {
            events: Mutex::new(Vec::new()),
        };

        let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &MockExecutor,
            None,
            Some(&sink),
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect("workflow should execute");

        let events = sink
            .events
            .lock()
            .expect("recording sink lock should not be poisoned");
        let completed = events
            .iter()
            .find(|event| event.event_type == "workflow_completed")
            .expect("expected workflow_completed event");
        let metadata = completed
            .metadata
            .as_ref()
            .expect("workflow_completed should include metadata by default");
        let nerdstats = metadata
            .get("nerdstats")
            .expect("nerdstats should be present by default");

        assert_eq!(nerdstats["workflow_id"], Value::String(output.workflow_id));
        assert_eq!(
            nerdstats["terminal_node"],
            Value::String(output.terminal_node)
        );
        assert_eq!(
            nerdstats["total_tokens"],
            Value::Number(output.total_tokens.into())
        );
        assert_eq!(nerdstats["token_metrics_available"], Value::Bool(true));
        assert_eq!(
            nerdstats["token_metrics_source"],
            Value::String("provider_usage".to_string())
        );
        assert_eq!(nerdstats["ttft_ms"], Value::Null);
    }

    #[tokio::test]
    async fn workflow_completed_event_omits_nerdstats_when_disabled() {
        let yaml = r#"
id: nerdstats-disabled
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let sink = RecordingSink {
            events: Mutex::new(Vec::new()),
        };
        let options = YamlWorkflowRunOptions {
            telemetry: YamlWorkflowTelemetryConfig {
                nerdstats: false,
                ..YamlWorkflowTelemetryConfig::default()
            },
            ..YamlWorkflowRunOptions::default()
        };

        run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &MockExecutor,
            None,
            Some(&sink),
            &options,
        )
        .await
        .expect("workflow should execute");

        let events = sink
            .events
            .lock()
            .expect("recording sink lock should not be poisoned");
        let completed = events
            .iter()
            .find(|event| event.event_type == "workflow_completed")
            .expect("expected workflow_completed event");
        assert!(completed.metadata.is_none());
    }

    struct StreamAwareMockExecutor;

    #[async_trait]
    impl YamlWorkflowLlmExecutor for StreamAwareMockExecutor {
        async fn complete_structured(
            &self,
            request: YamlLlmExecutionRequest,
            _event_sink: Option<&dyn YamlWorkflowEventSink>,
        ) -> Result<YamlLlmExecutionResult, String> {
            Ok(YamlLlmExecutionResult {
                payload: json!({"state":"ok"}),
                usage: Some(YamlLlmTokenUsage {
                    prompt_tokens: 20,
                    completion_tokens: 10,
                    total_tokens: 30,
                    thinking_tokens: None,
                }),
                ttft_ms: if request.stream { Some(12) } else { None },
            })
        }
    }

    #[tokio::test]
    async fn workflow_completed_event_includes_nerdstats_for_streaming_nodes() {
        let yaml = r#"
id: nerdstats-streaming
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
        stream: true
    config:
      prompt: |
        Return JSON only:
        {"state":"ok"}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let sink = RecordingSink {
            events: Mutex::new(Vec::new()),
        };

        run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &StreamAwareMockExecutor,
            None,
            Some(&sink),
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect("workflow should execute");

        let events = sink
            .events
            .lock()
            .expect("recording sink lock should not be poisoned");
        let completed = events
            .iter()
            .find(|event| event.event_type == "workflow_completed")
            .expect("expected workflow_completed event");
        let metadata = completed
            .metadata
            .as_ref()
            .expect("workflow_completed should include metadata by default");
        let nerdstats = metadata
            .get("nerdstats")
            .expect("nerdstats should be present by default");

        assert_eq!(nerdstats["token_metrics_available"], Value::Bool(true));
        assert_eq!(nerdstats["total_tokens"], Value::Number(30u64.into()));
        assert_eq!(nerdstats["ttft_ms"], Value::Number(12u64.into()));
    }

    #[test]
    fn workflow_nerdstats_marks_stream_token_metrics_unavailable() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "workflow".to_string(),
            entry_node: "start".to_string(),
            email_text: "hello".to_string(),
            trace: vec!["llm_node".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "llm_node".to_string(),
            terminal_output: None,
            step_timings: vec![YamlStepTiming {
                node_id: "llm_node".to_string(),
                node_kind: "llm_call".to_string(),
                elapsed_ms: 100,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                thinking_tokens: None,
                tokens_per_second: None,
            }],
            llm_node_metrics: BTreeMap::new(),
            total_elapsed_ms: 100,
            ttft_ms: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            total_thinking_tokens: None,
            tokens_per_second: 0.0,
            trace_id: Some("trace-1".to_string()),
            metadata: None,
        };

        let nerdstats = workflow_nerdstats(&output);
        assert_eq!(nerdstats["token_metrics_available"], Value::Bool(false));
        assert_eq!(
            nerdstats["token_metrics_source"],
            Value::String("provider_stream_usage_unavailable".to_string())
        );
        assert_eq!(nerdstats["total_tokens"], Value::Null);
        assert_eq!(nerdstats["ttft_ms"], Value::Null);
    }

    #[test]
    fn workflow_nerdstats_includes_ttft_when_available() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "workflow".to_string(),
            entry_node: "start".to_string(),
            email_text: "hello".to_string(),
            trace: vec!["llm_node".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "llm_node".to_string(),
            terminal_output: None,
            step_timings: vec![YamlStepTiming {
                node_id: "llm_node".to_string(),
                node_kind: "llm_call".to_string(),
                elapsed_ms: 100,
                prompt_tokens: Some(10),
                completion_tokens: Some(15),
                total_tokens: Some(25),
                thinking_tokens: None,
                tokens_per_second: Some(150.0),
            }],
            llm_node_metrics: BTreeMap::new(),
            total_elapsed_ms: 100,
            ttft_ms: Some(42),
            total_input_tokens: 10,
            total_output_tokens: 15,
            total_tokens: 25,
            total_thinking_tokens: None,
            tokens_per_second: 150.0,
            trace_id: Some("trace-2".to_string()),
            metadata: None,
        };

        let nerdstats = workflow_nerdstats(&output);
        assert_eq!(nerdstats["ttft_ms"], Value::Number(42u64.into()));
    }

    struct MessageHistoryExecutor;

    #[async_trait]
    impl YamlWorkflowLlmExecutor for MessageHistoryExecutor {
        async fn complete_structured(
            &self,
            request: YamlLlmExecutionRequest,
            _event_sink: Option<&dyn YamlWorkflowEventSink>,
        ) -> Result<YamlLlmExecutionResult, String> {
            let messages = request
                .messages
                .ok_or_else(|| "expected messages in request".to_string())?;
            if messages.len() != 2 {
                return Err(format!("expected 2 messages, got {}", messages.len()));
            }
            Ok(YamlLlmExecutionResult {
                payload: json!({"category":"termination","reason":"history"}),
                usage: Some(YamlLlmTokenUsage {
                    prompt_tokens: 7,
                    completion_tokens: 3,
                    total_tokens: 10,
                    thinking_tokens: None,
                }),
                ttft_ms: None,
            })
        }
    }

    #[tokio::test]
    async fn supports_messages_path_in_workflow_input() {
        let yaml = r#"
id: email-intake-classification
entry_node: classify_top_level
nodes:
  - id: classify_top_level
    node_type:
      llm_call:
        model: gpt-4.1
        messages_path: input.messages
        append_prompt_as_user: false
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let input = json!({
            "email_text": "ignored",
            "messages": [
                {"role": "system", "content": "You are a classifier"},
                {"role": "user", "content": "Please classify this"}
            ]
        });

        let output = run_workflow_yaml(&workflow, &input, &MessageHistoryExecutor)
            .await
            .expect("workflow should use chat history from input");

        assert_eq!(output.terminal_node, "classify_top_level");
        assert_eq!(
            output.outputs["classify_top_level"]["output"]["reason"],
            Value::String("history".to_string())
        );
    }

    #[tokio::test]
    async fn wrapper_entrypoints_produce_equivalent_outputs() {
        let yaml = r#"
id: wrapper-equivalence
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let input = json!({"email_text":"hello"});

        let a = run_workflow_yaml(&workflow, &input, &MockExecutor)
            .await
            .expect("base entrypoint should execute");
        let b = run_workflow_yaml_with_custom_worker(&workflow, &input, &MockExecutor, None)
            .await
            .expect("custom worker wrapper should execute");
        let c = run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &input,
            &MockExecutor,
            None,
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect("events/options wrapper should execute");

        assert_eq!(a.workflow_id, b.workflow_id);
        assert_eq!(a.workflow_id, c.workflow_id);
        assert_eq!(a.terminal_node, b.terminal_node);
        assert_eq!(a.terminal_node, c.terminal_node);
        assert_eq!(a.outputs, b.outputs);
        assert_eq!(a.outputs, c.outputs);
        assert_eq!(a.total_tokens, b.total_tokens);
        assert_eq!(a.total_tokens, c.total_tokens);
    }

    #[tokio::test]
    async fn custom_worker_receives_trace_context_block() {
        let yaml = r#"
id: custom-worker-trace-context
entry_node: lookup
nodes:
  - id: lookup
    node_type:
      custom_worker:
        handler: GetRagData
    config:
      payload:
        topic: demo
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let worker = CapturingWorker {
            context: Mutex::new(None),
        };
        let options = YamlWorkflowRunOptions {
            trace: YamlWorkflowTraceOptions {
                context: Some(YamlWorkflowTraceContextInput {
                    trace_id: Some("trace-fixed-ctx".to_string()),
                    traceparent: Some("00-trace-fixed-ctx-span-fixed-01".to_string()),
                    ..YamlWorkflowTraceContextInput::default()
                }),
                tenant: YamlWorkflowTraceTenantContext {
                    conversation_id: Some("7fd67af3-c67d-46cb-95af-08f8ed7b06a5".to_string()),
                    request_id: Some("turn-7".to_string()),
                    ..YamlWorkflowTraceTenantContext::default()
                },
            },
            ..YamlWorkflowRunOptions::default()
        };

        run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &MockExecutor,
            Some(&worker),
            None,
            &options,
        )
        .await
        .expect("workflow should execute");

        let captured_context = worker
            .context
            .lock()
            .expect("capturing worker lock should not be poisoned")
            .clone()
            .expect("custom worker should receive context");

        assert_eq!(
            captured_context
                .get("trace")
                .and_then(|trace| trace.get("context"))
                .and_then(|context| context.get("trace_id"))
                .and_then(Value::as_str),
            Some("trace-fixed-ctx")
        );
        assert_eq!(
            captured_context
                .get("trace")
                .and_then(|trace| trace.get("context"))
                .and_then(|context| context.get("traceparent"))
                .and_then(Value::as_str),
            Some("00-trace-fixed-ctx-span-fixed-01")
        );
        assert_eq!(
            captured_context
                .get("trace")
                .and_then(|trace| trace.get("tenant"))
                .and_then(|tenant| tenant.get("conversation_id"))
                .and_then(Value::as_str),
            Some("7fd67af3-c67d-46cb-95af-08f8ed7b06a5")
        );
    }

    #[tokio::test]
    async fn event_sink_cancellation_stops_workflow_before_llm_execution() {
        let yaml = r#"
id: cancellation-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let executor = CountingExecutor {
            call_count: AtomicUsize::new(0),
        };
        let sink = CancelAfterFirstEventSink {
            cancelled: AtomicBool::new(false),
        };

        let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &executor,
            None,
            Some(&sink),
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect_err("workflow should stop when sink signals cancellation");

        assert!(matches!(
            err,
            YamlWorkflowRunError::EventSinkCancelled { .. }
        ));
        assert_eq!(executor.call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn rejects_invalid_messages_path_shape() {
        let yaml = r#"
id: email-intake-classification
entry_node: classify_top_level
nodes:
  - id: classify_top_level
    node_type:
      llm_call:
        model: gpt-4.1
        messages_path: input.messages
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let input = json!({
            "email_text": "ignored",
            "messages": "not-a-list"
        });

        let err = run_workflow_yaml(&workflow, &input, &MessageHistoryExecutor)
            .await
            .expect_err("workflow should fail for invalid messages shape");

        assert!(matches!(err, YamlWorkflowRunError::Llm { .. }));
    }

    #[test]
    fn renders_yaml_workflow_to_mermaid_with_switch_labels() {
        let yaml = r#"
id: chat-workflow
entry_node: decide
nodes:
  - id: decide
    node_type:
      switch:
        branches:
          - condition: '$.input.mode == "draft"'
            target: draft
        default: ask
  - id: draft
    node_type:
      llm_call:
        model: gpt-4.1
  - id: ask
    node_type:
      llm_call:
        model: gpt-4.1
edges:
  - from: draft
    to: ask
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let mermaid = yaml_workflow_to_mermaid(&workflow);

        assert!(mermaid.contains("flowchart TD"));
        assert!(mermaid.contains("decide -- \"route1\" --> draft"));
        assert!(mermaid.contains("decide -- \"default\" --> ask"));
        assert!(mermaid.contains("draft --> ask"));
    }

    #[test]
    fn converts_yaml_workflow_to_ir_definition() {
        let yaml = r#"
id: chat-workflow
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        classify
  - id: route
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify.output.kind == "x"'
            target: done
        default: done
  - id: done
    node_type:
      custom_worker:
        handler: GetRagData
    config:
      payload:
        topic: test
edges:
  - from: classify
    to: route
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let ir = yaml_workflow_to_ir(&workflow).expect("yaml should convert to ir");

        assert_eq!(ir.name, "chat-workflow");
        assert!(ir.nodes.iter().any(|n| n.id == "__yaml_start"));
        assert!(ir.nodes.iter().any(|n| n.id == "classify"));
        assert!(ir.nodes.iter().any(|n| n.id == "route"));
        assert!(ir.nodes.iter().any(|n| n.id == "done"));
    }

    #[test]
    fn supports_yaml_to_ir_when_messages_path_is_used() {
        let yaml = r#"
id: chat-workflow
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
        messages_path: input.messages
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let ir =
            yaml_workflow_to_ir(&workflow).expect("messages_path should convert to tool-based IR");
        assert!(ir.nodes.iter().any(|node| matches!(
            node.kind,
            crate::ir::NodeKind::Tool { ref tool, .. } if tool == "__yaml_llm_call"
        )));
    }

    #[tokio::test]
    async fn workflow_output_contains_trace_id_in_both_locations() {
        let yaml = r#"
id: trace-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let output = run_workflow_yaml(&workflow, &json!({"email_text":"hello"}), &MockExecutor)
            .await
            .expect("workflow should execute");

        let trace_id = output
            .trace_id
            .as_deref()
            .expect("trace_id should be present");
        assert!(!trace_id.is_empty());
        assert_eq!(
            output.metadata.as_ref().and_then(|value| {
                value
                    .get("telemetry")
                    .and_then(|telemetry| telemetry.get("trace_id"))
                    .and_then(Value::as_str)
            }),
            Some(trace_id)
        );
    }

    #[tokio::test]
    async fn workflow_run_options_use_explicit_trace_id_and_payload_mode() {
        let yaml = r#"
id: trace-options-test
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Classify this email into exactly one category:
        {{ input.email_text }}
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let options = YamlWorkflowRunOptions {
            telemetry: YamlWorkflowTelemetryConfig {
                payload_mode: YamlWorkflowPayloadMode::RedactedPayload,
                ..YamlWorkflowTelemetryConfig::default()
            },
            trace: YamlWorkflowTraceOptions {
                context: Some(YamlWorkflowTraceContextInput {
                    trace_id: Some("trace-fixed-123".to_string()),
                    traceparent: Some("00-trace-fixed-123-span-1-01".to_string()),
                    ..YamlWorkflowTraceContextInput::default()
                }),
                tenant: YamlWorkflowTraceTenantContext {
                    conversation_id: Some("6e6d3125-b9f1-4af2-af1f-7cca024a2c42".to_string()),
                    ..YamlWorkflowTraceTenantContext::default()
                },
            },
        };

        let output = run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &MockExecutor,
            None,
            None,
            &options,
        )
        .await
        .expect("workflow should execute");

        assert_eq!(output.trace_id.as_deref(), Some("trace-fixed-123"));
        assert_eq!(
            output
                .metadata
                .as_ref()
                .and_then(|value| value.get("telemetry"))
                .and_then(|telemetry| telemetry.get("payload_mode"))
                .and_then(Value::as_str),
            Some("redacted_payload")
        );
        assert_eq!(
            output
                .metadata
                .as_ref()
                .and_then(|value| value.get("trace"))
                .and_then(|trace| trace.get("tenant"))
                .and_then(|tenant| tenant.get("conversation_id"))
                .and_then(Value::as_str),
            Some("6e6d3125-b9f1-4af2-af1f-7cca024a2c42")
        );
    }

    #[test]
    fn streamed_payload_parser_extracts_last_json_object() {
        let raw = r#"{"state":"missing_scenario","reason":"ok"}

extra reasoning text

{"state":"ready","reason":"final"}"#;

        let resolved = parse_streamed_structured_payload(raw, false)
            .expect("parser should extract final JSON object");
        assert_eq!(resolved.payload["state"], "ready");
        assert!(resolved.heal_confidence.is_none());
    }

    #[test]
    fn streamed_payload_parser_handles_unbalanced_reasoning_before_json() {
        let raw = "reasoning text with unmatched { braces and thoughts\n{\"state\":\"ready\",\"reason\":\"final\"}";

        let resolved = parse_streamed_structured_payload(raw, false)
            .expect("parser should recover final structured JSON object");
        assert_eq!(resolved.payload["state"], "ready");
    }

    #[test]
    fn streamed_payload_parser_handles_markdown_with_heal() {
        let raw = r#"Some preface
```json
{
  "state": "missing_scenario",
  "reason": "Need more details"
}
```
Some trailing explanation"#;

        let resolved = parse_streamed_structured_payload(raw, true)
            .expect("heal path should parse JSON block");
        assert_eq!(resolved.payload["state"], "missing_scenario");
        assert!(resolved.heal_confidence.is_some());
    }

    #[test]
    fn streamed_payload_parser_errors_when_no_json_candidate_exists() {
        let raw = "No JSON in this streamed output";
        let error = parse_streamed_structured_payload(raw, false)
            .expect_err("strict stream parse should fail without JSON candidate");
        assert!(error.contains("no JSON object candidate found"));
    }

    #[test]
    fn include_raw_stream_debug_events_defaults_to_false() {
        let _guard = stream_debug_env_lock()
            .lock()
            .expect("stream debug env lock should not be poisoned");
        std::env::remove_var("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW");
        assert!(!include_raw_stream_debug_events());
    }

    #[test]
    fn include_raw_stream_debug_events_accepts_truthy_values() {
        let _guard = stream_debug_env_lock()
            .lock()
            .expect("stream debug env lock should not be poisoned");
        std::env::set_var("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW", "true");
        assert!(include_raw_stream_debug_events());
        std::env::remove_var("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW");
    }

    #[test]
    fn structured_json_delta_filter_strips_reasoning_prefix_and_suffix() {
        let mut filter = StructuredJsonDeltaFilter::default();
        let chunks = vec![
            "I will think first... ",
            "{\"state\":\"missing_scenario\",",
            "\"reason\":\"Need more details\"}",
            " additional commentary",
        ];

        let filtered = chunks
            .into_iter()
            .filter_map(|chunk| filter.split(chunk).0)
            .collect::<String>();

        assert_eq!(
            filtered,
            "{\"state\":\"missing_scenario\",\"reason\":\"Need more details\"}"
        );
    }

    #[test]
    fn structured_json_delta_filter_handles_braces_inside_strings() {
        let mut filter = StructuredJsonDeltaFilter::default();
        let chunks = vec![
            "preface ",
            "{\"reason\":\"brace } in text\",\"state\":\"ok\"}",
            " trailing",
        ];

        let filtered = chunks
            .into_iter()
            .filter_map(|chunk| filter.split(chunk).0)
            .collect::<String>();

        assert_eq!(
            filtered,
            "{\"reason\":\"brace } in text\",\"state\":\"ok\"}"
        );
    }

    #[test]
    fn render_json_object_as_text_converts_top_level_fields() {
        let rendered =
            render_json_object_as_text(r#"{"question":"q","confidence":0.8,"nested":{"a":1}}"#);
        let lines: std::collections::HashSet<&str> = rendered.lines().collect();

        assert_eq!(lines.len(), 3);
        assert!(lines.contains("question: q"));
        assert!(lines.contains("confidence: 0.8"));
        assert!(lines.contains("nested: {\"a\":1}"));
    }

    #[test]
    fn stream_json_as_text_formatter_emits_once_when_complete() {
        let mut formatter = StreamJsonAsTextFormatter::default();
        formatter.push("{\"question\":\"hello\"}");

        let first = formatter.emit_if_ready(true);
        let second = formatter.emit_if_ready(true);

        assert_eq!(first, Some("question: hello".to_string()));
        assert_eq!(second, None);
    }

    #[test]
    fn rewrite_yaml_condition_preserves_output_prefix_in_field_names() {
        let expr = "$.nodes.classify.output.output_total == 1";
        let rewritten = rewrite_yaml_condition_to_ir(expr);
        assert_eq!(rewritten, "$.node_outputs.classify.output_total == 1");
    }

    #[tokio::test]
    async fn validates_workflow_input_before_ir_runtime_path() {
        let yaml = r#"
id: chat-workflow
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        classify
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let err = run_workflow_yaml(&workflow, &json!("not-an-object"), &MockExecutor)
            .await
            .expect_err("non-object input should fail before execution");

        assert!(matches!(err, YamlWorkflowRunError::InvalidInput { .. }));
    }

    #[test]
    fn interpolate_template_supports_dollar_prefixed_paths() {
        let context = json!({
            "input": {
                "email_text": "hello"
            }
        });

        let rendered = interpolate_template("value={{ $.input.email_text }}", &context);
        assert_eq!(rendered, "value=hello");
    }
}
