use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use simple_agent_type::message::{Message, Role};
use simple_agent_type::request::CompletionRequest;
use simple_agent_type::response::FinishReason;
use simple_agent_type::tool::{
    ToolCall, ToolChoice, ToolChoiceMode, ToolChoiceTool, ToolDefinition, ToolType,
};
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, SimpleAgentsClient,
};
use simple_agents_healing::JsonishParser;
use thiserror::Error;

use crate::ir::{Node, NodeKind, RouterRoute, WorkflowDefinition, WORKFLOW_IR_V0};
use crate::observability::tracing::{
    flush_workflow_tracer, workflow_tracer, SpanKind, TraceContext, WorkflowSpan,
};
use crate::runtime::{
    LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor, ToolExecutionError,
    ToolExecutionInput, ToolExecutor, WorkflowRuntime, WorkflowRuntimeError,
    WorkflowRuntimeOptions,
};
use crate::validation::validate_and_normalize;
use crate::visualize::workflow_to_mermaid;

mod runner;
pub use runner::WorkflowRunner;
mod api;
mod client_executor;
mod context;
mod events;
mod execute;
mod globals;
mod llm_tools;
mod node_execution;
mod spans;
mod typed_contracts;
mod validation;
pub use api::{
    run_email_workflow_yaml, run_email_workflow_yaml_file,
    run_email_workflow_yaml_file_with_client,
    run_email_workflow_yaml_file_with_client_and_custom_worker,
    run_email_workflow_yaml_file_with_client_and_custom_worker_and_events,
    run_email_workflow_yaml_with_client, run_email_workflow_yaml_with_client_and_custom_worker,
    run_email_workflow_yaml_with_client_and_custom_worker_and_events,
    run_email_workflow_yaml_with_custom_worker,
    run_email_workflow_yaml_with_custom_worker_and_events, run_workflow_yaml,
    run_workflow_yaml_file, run_workflow_yaml_file_with_client,
    run_workflow_yaml_file_with_client_and_custom_worker,
    run_workflow_yaml_file_with_client_and_custom_worker_and_events,
    run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options,
    run_workflow_yaml_with_client, run_workflow_yaml_with_client_and_custom_worker,
    run_workflow_yaml_with_client_and_custom_worker_and_events,
    run_workflow_yaml_with_custom_worker, run_workflow_yaml_with_custom_worker_and_events,
};
use client_executor::BorrowedClientExecutor;
use context::{
    build_yaml_context_from_ir_scope, collect_template_bindings, evaluate_switch_condition,
    interpolate_template, json_type_name, parse_messages_from_context, resolve_path,
};
use globals::{apply_set_globals, apply_update_globals};
use llm_tools::{
    default_llm_output_schema, llm_output_schema_for_node, normalize_llm_tools,
    normalize_tool_choice,
};
pub use typed_contracts::{
    YamlWorkflowEventType, YamlWorkflowNodeKind, YamlWorkflowNodeOutputRecord,
    YamlWorkflowRunTypedOutput, YamlWorkflowTypedEvent,
};
pub use validation::verify_yaml_workflow;

const YAML_START_NODE_ID: &str = "__yaml_start";
const YAML_LLM_TOOL_ID: &str = "__yaml_llm_call";
const MAX_WORKFLOW_YAML_BYTES: u64 = 1024 * 1024;
const MAX_WORKFLOW_YAML_DEPTH: usize = 64;

static TRACE_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlLlmNodeMetrics {
    pub elapsed_ms: u128,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
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
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YamlLlmTokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlLlmExecutionResult {
    pub payload: Value,
    pub usage: Option<YamlLlmTokenUsage>,
    pub ttft_ms: Option<u128>,
    pub tool_calls: Vec<YamlToolCallTrace>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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
struct YamlTokenTotals {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    reasoning_tokens: Option<u64>,
}

impl YamlTokenTotals {
    fn add_usage(&mut self, usage: &YamlLlmTokenUsage) {
        self.input_tokens += u64::from(usage.prompt_tokens);
        self.output_tokens += u64::from(usage.completion_tokens);
        self.total_tokens += u64::from(usage.total_tokens);

        if let Some(reasoning_tokens) = usage.reasoning_tokens {
            let next = self.reasoning_tokens.unwrap_or(0) + u64::from(reasoning_tokens);
            self.reasoning_tokens = Some(next);
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

fn resolve_requested_model(run_model_override: Option<&str>, node_model: &str) -> String {
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

fn validate_sample_rate(sample_rate: f32) -> Result<(), YamlWorkflowRunError> {
    if sample_rate.is_finite() && (0.0..=1.0).contains(&sample_rate) {
        return Ok(());
    }

    Err(YamlWorkflowRunError::InvalidInput {
        message: format!(
            "telemetry.sample_rate must be between 0.0 and 1.0 inclusive; received {sample_rate}"
        ),
    })
}

fn should_sample_trace(trace_id: &str, sample_rate: f32) -> bool {
    if sample_rate >= 1.0 {
        return true;
    }
    if sample_rate <= 0.0 {
        return false;
    }

    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in trace_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    let ratio = (hash as f64) / (u64::MAX as f64);
    ratio < (sample_rate as f64)
}

fn trace_id_from_traceparent(traceparent: &str) -> Option<String> {
    let mut parts = traceparent.split('-');
    let version = parts.next()?;
    let trace_id = parts.next()?;
    let _span_id = parts.next()?;
    let _flags = parts.next()?;
    if parts.next().is_some() {
        return None;
    }

    if version.len() != 2 || trace_id.len() != 32 {
        return None;
    }

    if !version.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    if !trace_id.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    if trace_id.chars().all(|ch| ch == '0') {
        return None;
    }

    Some(trace_id.to_ascii_lowercase())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TraceIdSource {
    Disabled,
    ExplicitTraceId,
    ParentTraceId,
    Traceparent,
    ParentTraceparent,
    Generated,
}

#[derive(Debug, Clone)]
struct ResolvedTelemetryContext {
    trace_id: Option<String>,
    sampled: bool,
    trace_id_source: TraceIdSource,
}

fn resolve_run_trace_id_with_source(
    options: &YamlWorkflowRunOptions,
    parent_trace_context: Option<&TraceContext>,
) -> (Option<String>, TraceIdSource) {
    if !options.telemetry.enabled {
        return (None, TraceIdSource::Disabled);
    }

    if let Some(trace_id) = options
        .trace
        .context
        .as_ref()
        .and_then(|context| context.trace_id.clone())
    {
        return (Some(trace_id), TraceIdSource::ExplicitTraceId);
    }

    if let Some(trace_id) = parent_trace_context.and_then(|context| context.trace_id.clone()) {
        return (Some(trace_id), TraceIdSource::ParentTraceId);
    }

    if let Some(trace_id) = options
        .trace
        .context
        .as_ref()
        .and_then(|context| context.traceparent.as_deref())
        .and_then(trace_id_from_traceparent)
    {
        return (Some(trace_id), TraceIdSource::Traceparent);
    }

    if let Some(trace_id) = parent_trace_context
        .and_then(|context| context.traceparent.as_deref())
        .and_then(trace_id_from_traceparent)
    {
        return (Some(trace_id), TraceIdSource::ParentTraceparent);
    }

    (Some(generate_trace_id()), TraceIdSource::Generated)
}

fn resolve_telemetry_context(
    options: &YamlWorkflowRunOptions,
    parent_trace_context: Option<&TraceContext>,
) -> ResolvedTelemetryContext {
    let (trace_id, trace_id_source) =
        resolve_run_trace_id_with_source(options, parent_trace_context);
    let sampled = trace_id
        .as_deref()
        .map(|value| should_sample_trace(value, options.telemetry.sample_rate))
        .unwrap_or(false);

    ResolvedTelemetryContext {
        trace_id,
        sampled,
        trace_id_source,
    }
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

fn workflow_metadata_with_trace(
    options: &YamlWorkflowRunOptions,
    trace_id: &str,
    sampled: bool,
    trace_id_source: TraceIdSource,
) -> Value {
    json!({
        "telemetry": {
            "trace_id": trace_id,
            "trace_id_source": match trace_id_source {
                TraceIdSource::Disabled => "disabled",
                TraceIdSource::ExplicitTraceId => "explicit_trace_id",
                TraceIdSource::ParentTraceId => "parent_trace_id",
                TraceIdSource::Traceparent => "traceparent",
                TraceIdSource::ParentTraceparent => "parent_traceparent",
                TraceIdSource::Generated => "generated",
            },
            "enabled": options.telemetry.enabled,
            "sampled": sampled,
            "nerdstats": options.telemetry.nerdstats,
            "sample_rate": options.telemetry.sample_rate,
            "payload_mode": match options.telemetry.payload_mode {
                YamlWorkflowPayloadMode::FullPayload => "full_payload",
                YamlWorkflowPayloadMode::RedactedPayload => "redacted_payload",
            },
            "retention_days": options.telemetry.retention_days,
            "multi_tenant": options.telemetry.multi_tenant,
            "tool_trace_mode": match options.telemetry.tool_trace_mode {
                YamlToolTraceMode::Full => "full",
                YamlToolTraceMode::Redacted => "redacted",
                YamlToolTraceMode::Off => "off",
            },
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

fn apply_trace_identity_attributes(span: &mut dyn WorkflowSpan, trace_id: Option<&str>) {
    if let Some(value) = trace_id {
        span.set_attribute("trace_id", value);
    }
}

fn apply_trace_tenant_attributes_from_tenant(
    span: &mut dyn WorkflowSpan,
    tenant: &YamlWorkflowTraceTenantContext,
) {
    if let Some(workspace_id) = tenant.workspace_id.as_deref() {
        span.set_attribute("tenant.workspace_id", workspace_id);
    }
    if let Some(user_id) = tenant.user_id.as_deref() {
        span.set_attribute("tenant.user_id", user_id);
        span.set_attribute("user.id", user_id);
        span.set_attribute("langfuse.user.id", user_id);
    }
    if let Some(conversation_id) = tenant.conversation_id.as_deref() {
        span.set_attribute("tenant.conversation_id", conversation_id);
        span.set_attribute("session.id", conversation_id);
        span.set_attribute("langfuse.session.id", conversation_id);
    }
    if let Some(request_id) = tenant.request_id.as_deref() {
        span.set_attribute("tenant.request_id", request_id);
    }
    if let Some(run_id) = tenant.run_id.as_deref() {
        span.set_attribute("tenant.run_id", run_id);
    }
}

fn apply_trace_tenant_attributes(span: &mut dyn WorkflowSpan, options: &YamlWorkflowRunOptions) {
    apply_trace_tenant_attributes_from_tenant(span, &options.trace.tenant);
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
    let total_reasoning_tokens = if token_metrics_available {
        json!(output.total_reasoning_tokens)
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
        "step_details": output.step_timings,
        "total_input_tokens": total_input_tokens,
        "total_output_tokens": total_output_tokens,
        "total_tokens": total_tokens,
        "total_reasoning_tokens": total_reasoning_tokens,
        "tokens_per_second": tokens_per_second,
        "trace_id": output.trace_id,
        "token_metrics_available": token_metrics_available,
        "token_metrics_source": token_metrics_source,
        "llm_nodes_without_usage": llm_nodes_without_usage,
    })
}

fn apply_langfuse_nerdstats_attributes(
    span: &mut dyn WorkflowSpan,
    output: &YamlWorkflowRunOutput,
    enabled: bool,
) {
    if !enabled {
        return;
    }

    let nerdstats = workflow_nerdstats(output);
    let nerdstats_json = nerdstats.to_string();
    span.set_attribute("langfuse.trace.metadata.nerdstats", nerdstats_json.as_str());

    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.workflow_id",
        output.workflow_id.as_str(),
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.terminal_node",
        output.terminal_node.as_str(),
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.total_elapsed_ms",
        output.total_elapsed_ms.to_string().as_str(),
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.step_details_count",
        output.step_timings.len().to_string().as_str(),
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.total_input_tokens",
        output.total_input_tokens.to_string().as_str(),
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.total_output_tokens",
        output.total_output_tokens.to_string().as_str(),
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.total_tokens",
        output.total_tokens.to_string().as_str(),
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.tokens_per_second",
        output.tokens_per_second.to_string().as_str(),
    );

    if let Some(ttft_ms) = output.ttft_ms {
        span.set_attribute(
            "langfuse.trace.metadata.nerdstats.ttft_ms",
            ttft_ms.to_string().as_str(),
        );
    }

    if let Some(reasoning_tokens) = output.total_reasoning_tokens {
        span.set_attribute(
            "langfuse.trace.metadata.nerdstats.total_reasoning_tokens",
            reasoning_tokens.to_string().as_str(),
        );
    }

    let llm_nodes_without_usage_count = output
        .step_timings
        .iter()
        .filter(|step| step.node_kind == "llm_call" && step.total_tokens.is_none())
        .count();
    let token_metrics_available = llm_nodes_without_usage_count == 0;
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.token_metrics_available",
        if token_metrics_available {
            "true"
        } else {
            "false"
        },
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.token_metrics_source",
        if token_metrics_available {
            "provider_usage"
        } else {
            "provider_stream_usage_unavailable"
        },
    );
    span.set_attribute(
        "langfuse.trace.metadata.nerdstats.llm_nodes_without_usage_count",
        llm_nodes_without_usage_count.to_string().as_str(),
    );
}

fn apply_langfuse_trace_input_output_attributes(
    span: &mut dyn WorkflowSpan,
    workflow_input: &Value,
    output: &YamlWorkflowRunOutput,
    payload_mode: YamlWorkflowPayloadMode,
) {
    let trace_input = payload_for_span(payload_mode, workflow_input);
    span.set_attribute("langfuse.trace.input", trace_input.as_str());

    if let Some(terminal_output) = output.terminal_output.as_ref() {
        let trace_output = payload_for_span(payload_mode, terminal_output);
        span.set_attribute("langfuse.trace.output", trace_output.as_str());
    }

    let usage_details = json!({
        "input": output.total_input_tokens,
        "output": output.total_output_tokens,
        "total": output.total_tokens,
        "reasoning": output.total_reasoning_tokens,
    })
    .to_string();
    span.set_attribute(
        "langfuse.trace.metadata.usage_details",
        usage_details.as_str(),
    );
}

fn apply_langfuse_observation_usage_attributes(
    span: &mut dyn WorkflowSpan,
    usage: &YamlLlmTokenUsage,
) {
    let usage_details = json!({
        "input": usage.prompt_tokens,
        "output": usage.completion_tokens,
        "total": usage.total_tokens,
        "reasoning": usage.reasoning_tokens,
    })
    .to_string();
    span.set_attribute("langfuse.observation.usage_details", usage_details.as_str());
    span.set_attribute(
        "gen_ai.usage.input_tokens",
        usage.prompt_tokens.to_string().as_str(),
    );
    span.set_attribute(
        "gen_ai.usage.output_tokens",
        usage.completion_tokens.to_string().as_str(),
    );
    span.set_attribute(
        "gen_ai.usage.total_tokens",
        usage.total_tokens.to_string().as_str(),
    );
    if let Some(reasoning_tokens) = usage.reasoning_tokens {
        span.set_attribute(
            "gen_ai.usage.reasoning_tokens",
            reasoning_tokens.to_string().as_str(),
        );
    }
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

fn payload_for_tool_trace(mode: YamlToolTraceMode, payload: &Value) -> Value {
    match mode {
        YamlToolTraceMode::Full => payload.clone(),
        YamlToolTraceMode::Redacted => json!({
            "redacted": true,
            "value_type": json_type_name(payload),
        }),
        YamlToolTraceMode::Off => Value::Null,
    }
}

fn validate_json_schema(schema: &Value) -> Result<(), String> {
    jsonschema::JSONSchema::compile(schema)
        .map(|_| ())
        .map_err(|error| format!("invalid JSON schema: {error}"))
}

fn validate_schema_instance(schema: &Value, instance: &Value) -> Result<(), String> {
    let validator = jsonschema::JSONSchema::compile(schema)
        .map_err(|error| format!("invalid JSON schema: {error}"))?;
    if let Err(errors) = validator.validate(instance) {
        let message = errors
            .into_iter()
            .next()
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown schema validation error".to_string());
        return Err(format!("schema validation failed: {message}"));
    }
    Ok(())
}

fn schema_type(schema: &Value) -> Option<&str> {
    schema.get("type").and_then(Value::as_str)
}

fn schema_expects_object(schema: &Value) -> bool {
    schema_type(schema) == Some("object")
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
    let mut tool_node_ids: Vec<String> = Vec::new();

    for node in &workflow.nodes {
        let label = format!("{}\\n({})", node.id, node.kind_name());
        lines.push(format!(
            "  {}[\"{}\"]",
            sanitize_mermaid_id(&node.id),
            escape_mermaid_label(label.as_str()),
        ));

        if let Some(llm) = node.node_type.llm_call.as_ref() {
            for (idx, tool_name) in llm_tool_names(llm).into_iter().enumerate() {
                let tool_id = sanitize_mermaid_id(format!("{}__tool_{}", node.id, idx).as_str());
                lines.push(format!(
                    "  {}([\"{}\"])",
                    tool_id,
                    escape_mermaid_label(format!("tool: {tool_name}").as_str())
                ));
                lines.push(format!(
                    "  {} -.-> {}",
                    sanitize_mermaid_id(&node.id),
                    tool_id
                ));
                tool_node_ids.push(tool_id);
            }
        }
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

    if !tool_node_ids.is_empty() {
        lines.push("  classDef toolNode fill:#FFF4D6,stroke:#D97706,color:#7C2D12;".to_string());
        lines.push(format!("  class {} toolNode;", tool_node_ids.join(",")));
    }

    lines.join("\n")
}

fn llm_tool_names(llm: &YamlLlmCall) -> Vec<String> {
    llm.tools
        .iter()
        .map(|tool| match tool {
            YamlToolDeclaration::OpenAi(openai) => openai.function.name.clone(),
            YamlToolDeclaration::Simplified(simple) => simple.name.clone(),
        })
        .collect()
}

/// Load a YAML workflow file and render it as Mermaid flowchart.
pub fn yaml_workflow_file_to_mermaid(workflow_path: &Path) -> Result<String, YamlWorkflowRunError> {
    let (canonical_path, workflow) = load_workflow_yaml_file(workflow_path)?;

    let referenced_subgraphs = discover_referenced_subgraphs(canonical_path.as_path(), &workflow)?;
    if referenced_subgraphs.is_empty() {
        return Ok(yaml_workflow_to_mermaid(&workflow));
    }

    Ok(yaml_workflow_to_mermaid_with_subgraphs(
        &workflow,
        &referenced_subgraphs,
    ))
}

#[derive(Debug, Clone)]
struct MermaidSubgraphWorkflow {
    alias: String,
    workflow: YamlWorkflow,
}

#[derive(Debug, Default)]
struct MermaidBlockRender {
    lines: Vec<String>,
    tool_node_ids: Vec<String>,
    run_workflow_tool_node_ids: Vec<String>,
    entry_node_id: String,
}

fn yaml_workflow_to_mermaid_with_subgraphs(
    workflow: &YamlWorkflow,
    subgraphs: &[MermaidSubgraphWorkflow],
) -> String {
    let mut lines = vec!["flowchart TD".to_string()];

    let main_block = render_mermaid_block(workflow, "main");
    lines.push(format!(
        "  subgraph main_graph[\"Main: {}\"]",
        escape_mermaid_label(&workflow.id)
    ));
    for line in &main_block.lines {
        lines.push(format!("    {line}"));
    }
    lines.push("  end".to_string());

    let mut all_tool_nodes = main_block.tool_node_ids.clone();

    for (index, subgraph) in subgraphs.iter().enumerate() {
        let block_id = format!("subgraph_{}", index + 1);
        let block = render_mermaid_block(&subgraph.workflow, &block_id);
        lines.push(format!(
            "  subgraph {}[\"Subgraph: {}\"]",
            sanitize_mermaid_id(&format!("{}_cluster", block_id)),
            escape_mermaid_label(&subgraph.alias)
        ));
        for line in &block.lines {
            lines.push(format!("    {line}"));
        }
        lines.push("  end".to_string());

        for tool_node in &main_block.run_workflow_tool_node_ids {
            lines.push(format!(
                "  {} -. \"{}\" .-> {}",
                tool_node,
                escape_mermaid_label(&format!("calls {}", subgraph.alias)),
                block.entry_node_id
            ));
        }

        all_tool_nodes.extend(block.tool_node_ids);
    }

    if !all_tool_nodes.is_empty() {
        lines.push("  classDef toolNode fill:#FFF4D6,stroke:#D97706,color:#7C2D12;".to_string());
        lines.push(format!("  class {} toolNode;", all_tool_nodes.join(",")));
    }

    lines.join("\n")
}

fn render_mermaid_block(workflow: &YamlWorkflow, prefix: &str) -> MermaidBlockRender {
    let mut block = MermaidBlockRender {
        entry_node_id: prefixed_mermaid_id(prefix, &workflow.entry_node),
        ..Default::default()
    };

    for node in &workflow.nodes {
        let node_id = prefixed_mermaid_id(prefix, &node.id);
        let label = format!("{}\\n({})", node.id, node.kind_name());
        block
            .lines
            .push(format!("{}[\"{}\"]", node_id, escape_mermaid_label(&label)));

        if let Some(llm) = node.node_type.llm_call.as_ref() {
            for (idx, tool) in llm.tools.iter().enumerate() {
                let tool_name = tool_declaration_name(tool);
                let tool_id = prefixed_mermaid_id(prefix, &format!("{}__tool_{}", node.id, idx));
                block.lines.push(format!(
                    "{}([\"{}\"])",
                    tool_id,
                    escape_mermaid_label(format!("tool: {tool_name}").as_str())
                ));
                block.lines.push(format!("{} -.-> {}", node_id, tool_id));
                block.tool_node_ids.push(tool_id.clone());

                if tool_name == "run_workflow_graph" {
                    block.run_workflow_tool_node_ids.push(tool_id);
                }
            }
        }
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
        let from_id = prefixed_mermaid_id(prefix, &from);
        let to_id = prefixed_mermaid_id(prefix, &to);
        if label.is_empty() {
            block.lines.push(format!("{} --> {}", from_id, to_id));
        } else {
            block.lines.push(format!(
                "{} -- \"{}\" --> {}",
                from_id,
                escape_mermaid_label(&label),
                to_id
            ));
        }
    }

    block
}

fn discover_referenced_subgraphs(
    workflow_path: &Path,
    workflow: &YamlWorkflow,
) -> Result<Vec<MermaidSubgraphWorkflow>, YamlWorkflowRunError> {
    let workflow_ids = referenced_workflow_ids(workflow);
    if workflow_ids.is_empty() {
        return Ok(Vec::new());
    }

    let parent_dir = workflow_path.parent().unwrap_or(Path::new("."));
    let sibling_workflows = load_yaml_sibling_workflows(parent_dir, workflow_path)?;

    let mut discovered = Vec::new();
    let mut seen = HashSet::new();

    for workflow_id in workflow_ids {
        if seen.contains(&workflow_id) {
            continue;
        }

        if let Some((_, subworkflow)) = sibling_workflows
            .iter()
            .find(|(key, _)| key == &workflow_id)
        {
            discovered.push(MermaidSubgraphWorkflow {
                alias: workflow_id.clone(),
                workflow: subworkflow.clone(),
            });
            seen.insert(workflow_id);
        }
    }

    Ok(discovered)
}

fn load_yaml_sibling_workflows(
    parent_dir: &Path,
    workflow_path: &Path,
) -> Result<Vec<(String, YamlWorkflow)>, YamlWorkflowRunError> {
    let mut results = Vec::new();
    let canonical_target =
        std::fs::canonicalize(workflow_path).unwrap_or_else(|_| workflow_path.to_path_buf());
    let entries = std::fs::read_dir(parent_dir).map_err(|source| YamlWorkflowRunError::Read {
        path: parent_dir.display().to_string(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| YamlWorkflowRunError::Read {
            path: parent_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !is_yaml_file(&path) {
            continue;
        }

        let canonical_path = std::fs::canonicalize(&path).unwrap_or(path.clone());
        if canonical_path == canonical_target {
            continue;
        }

        let (_, subworkflow) = load_workflow_yaml_file(path.as_path())?;

        if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
            results.push((stem.to_string(), subworkflow.clone()));
        }
        results.push((subworkflow.id.clone(), subworkflow));
    }

    Ok(results)
}

fn referenced_workflow_ids(workflow: &YamlWorkflow) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    for node in &workflow.nodes {
        let prompt = node
            .config
            .as_ref()
            .and_then(|config| config.prompt.as_deref());

        if let Some(llm) = node.node_type.llm_call.as_ref() {
            for tool in &llm.tools {
                if tool_declaration_name(tool) != "run_workflow_graph" {
                    continue;
                }

                for workflow_id in referenced_workflow_ids_from_tool(tool) {
                    if seen.insert(workflow_id.clone()) {
                        ids.push(workflow_id);
                    }
                }

                if let Some(prompt_text) = prompt {
                    for workflow_id in referenced_workflow_ids_from_prompt(prompt_text) {
                        if seen.insert(workflow_id.clone()) {
                            ids.push(workflow_id);
                        }
                    }
                }
            }
        }
    }

    ids
}

fn referenced_workflow_ids_from_tool(tool: &YamlToolDeclaration) -> Vec<String> {
    let schema = match tool {
        YamlToolDeclaration::OpenAi(openai) => openai.function.parameters.as_ref(),
        YamlToolDeclaration::Simplified(simple) => Some(&simple.input_schema),
    };

    let mut ids = Vec::new();
    if let Some(schema) = schema {
        if let Some(workflow_prop) = schema
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("workflow_id"))
        {
            if let Some(value) = workflow_prop.get("const").and_then(Value::as_str) {
                ids.push(value.to_string());
            }

            if let Some(enum_values) = workflow_prop.get("enum").and_then(Value::as_array) {
                for value in enum_values {
                    if let Some(value) = value.as_str() {
                        ids.push(value.to_string());
                    }
                }
            }
        }
    }

    ids
}

fn referenced_workflow_ids_from_prompt(prompt: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut search = prompt;

    while let Some(index) = search.find("\"workflow_id\"") {
        let remainder = &search[index + "\"workflow_id\"".len()..];
        let Some(colon_index) = remainder.find(':') else {
            break;
        };

        let candidate = remainder[colon_index + 1..].trim_start();
        if let Some(rest) = candidate.strip_prefix('"') {
            if let Some(end_quote_index) = rest.find('"') {
                let workflow_id = rest[..end_quote_index].trim();
                if !workflow_id.is_empty() {
                    ids.push(workflow_id.to_string());
                }
                search = &rest[end_quote_index + 1..];
                continue;
            }
        }

        search = &remainder[colon_index + 1..];
    }

    ids
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml") | Some("yml")
    )
}

fn yaml_value_depth(value: &serde_yaml::Value, depth: usize) -> usize {
    match value {
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .map(|item| yaml_value_depth(item, depth + 1))
            .max()
            .unwrap_or(depth),
        serde_yaml::Value::Mapping(map) => map
            .values()
            .map(|item| yaml_value_depth(item, depth + 1))
            .max()
            .unwrap_or(depth),
        _ => depth,
    }
}

fn load_workflow_yaml_file(
    workflow_path: &Path,
) -> Result<(PathBuf, YamlWorkflow), YamlWorkflowRunError> {
    let canonical_path =
        std::fs::canonicalize(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let metadata =
        std::fs::metadata(&canonical_path).map_err(|source| YamlWorkflowRunError::Read {
            path: canonical_path.display().to_string(),
            source,
        })?;

    if !metadata.is_file() {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: "path must reference a regular file".to_string(),
        });
    }

    if !is_yaml_file(&canonical_path) {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: "workflow file extension must be .yaml or .yml".to_string(),
        });
    }

    if metadata.len() > MAX_WORKFLOW_YAML_BYTES {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: format!(
                "workflow yaml is too large ({} bytes > {} bytes)",
                metadata.len(),
                MAX_WORKFLOW_YAML_BYTES
            ),
        });
    }

    let contents =
        std::fs::read_to_string(&canonical_path).map_err(|source| YamlWorkflowRunError::Read {
            path: canonical_path.display().to_string(),
            source,
        })?;

    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: canonical_path.display().to_string(),
            source,
        })?;

    let depth = yaml_value_depth(&yaml_value, 1);
    if depth > MAX_WORKFLOW_YAML_DEPTH {
        return Err(YamlWorkflowRunError::FileRejected {
            path: canonical_path.display().to_string(),
            reason: format!(
                "workflow yaml nesting depth {} exceeds limit {}",
                depth, MAX_WORKFLOW_YAML_DEPTH
            ),
        });
    }

    let workflow: YamlWorkflow =
        serde_yaml::from_value(yaml_value).map_err(|source| YamlWorkflowRunError::Parse {
            path: canonical_path.display().to_string(),
            source,
        })?;

    Ok((canonical_path, workflow))
}

fn prefixed_mermaid_id(prefix: &str, id: &str) -> String {
    sanitize_mermaid_id(format!("{}__{}", prefix, id).as_str())
}

fn tool_declaration_name(tool: &YamlToolDeclaration) -> &str {
    match tool {
        YamlToolDeclaration::OpenAi(openai) => &openai.function.name,
        YamlToolDeclaration::Simplified(simple) => &simple.name,
    }
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

            if !llm.tools.is_empty() {
                return Err(YamlToIrError::UnsupportedNode {
                    node_id: node.id.clone(),
                    reason: "llm_call.tools are not represented in canonical IR llm nodes yet"
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
                        "max_tokens": llm.max_tokens,
                        "temperature": llm.temperature,
                        "top_p": llm.top_p,
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
                    input: {
                        let mut payload = node
                            .config
                            .as_ref()
                            .and_then(|c| c.payload.clone())
                            .unwrap_or_else(|| json!({}));
                        if let Some(payload_obj) = payload.as_object_mut() {
                            payload_obj.insert(
                                "__handler_file".to_string(),
                                worker
                                    .handler_file
                                    .as_ref()
                                    .map(|value| Value::String(value.clone()))
                                    .unwrap_or(Value::Null),
                            );
                        }
                        payload
                    },
                    next,
                },
            });
            continue;
        }

        if let Some(switch) = node.node_type.switch.as_ref() {
            let mut routes = Vec::with_capacity(switch.branches.len());
            for branch in &switch.branches {
                let rewritten =
                    rewrite_yaml_condition_to_ir(&branch.condition).map_err(|reason| {
                        YamlToIrError::UnsupportedNode {
                            node_id: node.id.clone(),
                            reason: format!(
                                "failed to rewrite switch condition '{}': {}",
                                branch.condition, reason
                            ),
                        }
                    })?;
                routes.push(RouterRoute {
                    when: rewritten,
                    next: branch.target.clone(),
                });
            }
            nodes.push(Node {
                id: node.id.clone(),
                kind: NodeKind::Router {
                    routes,
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

fn rewrite_yaml_condition_to_ir(expr: &str) -> Result<String, String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::with_capacity(expr.len());
    let mut index = 0usize;
    let mut quote_context: Option<char> = None;
    let mut escaped = false;

    while index < chars.len() {
        let current = chars[index];

        if let Some(quote) = quote_context {
            out.push(current);
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == quote {
                quote_context = None;
            }
            index += 1;
            continue;
        }

        if current == '"' || current == '\'' {
            quote_context = Some(current);
            out.push(current);
            index += 1;
            continue;
        }

        if starts_with_chars(&chars, index, "$.nodes.") {
            out.push_str("$.node_outputs.");
            index += "$.nodes.".chars().count();
            continue;
        }

        if starts_with_chars(&chars, index, ".output")
            && is_output_segment_boundary(chars.get(index + ".output".chars().count()).copied())
        {
            index += ".output".chars().count();
            continue;
        }

        out.push(current);
        index += 1;
    }

    if quote_context.is_some() {
        return Err("condition contains an unterminated quoted string".to_string());
    }

    Ok(out)
}

fn starts_with_chars(chars: &[char], start: usize, pattern: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    if start + pattern_chars.len() > chars.len() {
        return false;
    }
    chars[start..start + pattern_chars.len()] == pattern_chars
}

fn is_output_segment_boundary(next: Option<char>) -> bool {
    match next {
        None => true,
        Some(ch) => {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '.' | ')' | ']' | '}' | ',' | '=' | '!' | '<' | '>' | '&' | '|'
                )
        }
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
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub messages: Option<Vec<Message>>,
    pub append_prompt_as_user: bool,
    pub prompt: String,
    pub prompt_template: String,
    pub prompt_bindings: Vec<YamlTemplateBinding>,
    pub schema: Value,
    pub stream: bool,
    pub heal: bool,
    pub tools: Vec<YamlResolvedTool>,
    pub tool_choice: Option<ToolChoice>,
    pub max_tool_roundtrips: u8,
    pub tool_calls_global_key: Option<String>,
    pub tool_trace_mode: YamlToolTraceMode,
    pub execution_context: Value,
    pub email_text: String,
    pub trace_id: Option<String>,
    pub trace_context: Option<TraceContext>,
    pub tenant_context: YamlWorkflowTraceTenantContext,
    pub trace_sampled: bool,
}

#[derive(Debug, Clone)]
pub struct YamlResolvedTool {
    pub definition: ToolDefinition,
    pub output_schema: Option<Value>,
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
        handler_file: Option<&str>,
        payload: &Value,
        email_text: &str,
        context: &Value,
    ) -> Result<Value, String>;
}

pub async fn run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let executor = BorrowedClientExecutor {
        client,
        custom_worker,
        run_options: options.clone(),
    };
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

pub async fn run_workflow_yaml_with_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    execute::run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
        workflow,
        workflow_input,
        executor,
        custom_worker,
        event_sink,
        options,
    )
    .await
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

    if validate_and_normalize(&ir).is_err() {
        return Ok(None);
    }

    let parent_trace_context = trace_context_from_options(options);
    let telemetry_context = resolve_telemetry_context(options, parent_trace_context.as_ref());

    let tracer = workflow_tracer();
    let mut workflow_span_context: Option<TraceContext> = None;
    let mut workflow_span = if telemetry_context.sampled {
        let (span_context, mut span) = tracer.start_span(
            "workflow.run",
            SpanKind::Workflow,
            parent_trace_context.as_ref(),
        );
        apply_trace_identity_attributes(span.as_mut(), telemetry_context.trace_id.as_deref());
        apply_trace_tenant_attributes(span.as_mut(), options);
        workflow_span_context = Some(span_context);
        Some(span)
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
        node_models: std::sync::Mutex<BTreeMap<String, String>>,
        model_override: Option<String>,
        trace_id: Option<String>,
        trace_context: Option<TraceContext>,
        trace_input_context: Option<YamlWorkflowTraceContextInput>,
        tenant_context: YamlWorkflowTraceTenantContext,
        payload_mode: YamlWorkflowPayloadMode,
        trace_sampled: bool,
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
                let resolved_model =
                    resolve_requested_model(self.model_override.as_deref(), &model);
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
                let max_tokens = input
                    .input
                    .get("max_tokens")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok());
                let temperature = input
                    .input
                    .get("temperature")
                    .and_then(Value::as_f64)
                    .map(|value| value as f32);
                let top_p = input
                    .input
                    .get("top_p")
                    .and_then(Value::as_f64)
                    .map(|value| value as f32);
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
                let email_text = context
                    .get("input")
                    .and_then(|v| v.get("email_text"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
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
                    model: resolved_model.clone(),
                    max_tokens,
                    temperature,
                    top_p,
                    messages,
                    append_prompt_as_user,
                    prompt,
                    prompt_template,
                    prompt_bindings,
                    schema,
                    stream,
                    heal,
                    tools: Vec::new(),
                    tool_choice: None,
                    max_tool_roundtrips: 1,
                    tool_calls_global_key: None,
                    tool_trace_mode: YamlToolTraceMode::Off,
                    execution_context: context.clone(),
                    email_text: email_text.to_string(),
                    trace_id: self.trace_id.clone(),
                    trace_context: self.trace_context.clone(),
                    tenant_context: self.tenant_context.clone(),
                    trace_sampled: self.trace_sampled,
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
                            usage_map.insert(node_id_for_metrics.clone(), usage.clone());
                        }
                    }
                    if let Ok(mut model_map) = self.node_models.lock() {
                        model_map.insert(node_id_for_metrics, resolved_model);
                    }
                }

                return llm_result.map(|result| result.payload);
            }

            let worker = self
                .custom_worker
                .ok_or_else(|| ToolExecutionError::NotFound {
                    tool: input.tool.clone(),
                })?;

            let mut payload = input.input.clone();
            let handler_file = payload
                .as_object_mut()
                .and_then(|obj| obj.remove("__handler_file"))
                .and_then(|value| value.as_str().map(ToString::to_string));
            let email_text = context
                .get("input")
                .and_then(|v| v.get("email_text"))
                .and_then(Value::as_str)
                .unwrap_or_default();

            let tracer = workflow_tracer();
            let mut handler_span_context: Option<TraceContext> = None;
            let mut handler_span = if self.trace_sampled {
                let (span_context, mut span) = tracer.start_span(
                    "handler.invoke",
                    SpanKind::Node,
                    self.trace_context.as_ref(),
                );
                handler_span_context = Some(span_context);
                apply_trace_identity_attributes(span.as_mut(), self.trace_id.as_deref());
                span.set_attribute("handler_name", input.tool.as_str());
                apply_trace_tenant_attributes_from_tenant(span.as_mut(), &self.tenant_context);
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
                model: None,
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
                .execute(
                    &input.tool,
                    handler_file.as_deref(),
                    &payload,
                    email_text,
                    &worker_context,
                )
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
        node_models: std::sync::Mutex::new(BTreeMap::new()),
        model_override: options.model.clone(),
        trace_id: telemetry_context.trace_id.clone(),
        trace_context: workflow_span_context.clone(),
        trace_input_context: options.trace.context.clone(),
        tenant_context: options.trace.tenant.clone(),
        payload_mode: options.telemetry.payload_mode,
        trace_sampled: telemetry_context.sampled,
    };

    let runtime_options = WorkflowRuntimeOptions {
        validate_before_run: false,
        ..WorkflowRuntimeOptions::default()
    };
    let runtime = WorkflowRuntime::new(ir, &NoopLlm, Some(&tool_executor), runtime_options);

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
    let llm_node_models = tool_executor
        .node_models
        .lock()
        .map(|models| models.clone())
        .unwrap_or_default();
    let mut llm_node_metrics: BTreeMap<String, YamlLlmNodeMetrics> = BTreeMap::new();
    for execution in result.node_executions {
        if execution.node_id == YAML_START_NODE_ID {
            continue;
        }
        let node_id = execution.node_id;
        trace.push(node_id.clone());
        let usage = node_usage_map.get(&node_id);
        if let Some(usage) = usage {
            llm_node_metrics.insert(
                node_id.clone(),
                YamlLlmNodeMetrics {
                    elapsed_ms: 0,
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                    total_tokens: usage.total_tokens,
                    reasoning_tokens: usage.reasoning_tokens,
                    tokens_per_second: completion_tokens_per_second(usage.completion_tokens, 0),
                },
            );
        }
        step_timings.push(YamlStepTiming {
            node_id: node_id.clone(),
            node_kind: "ir_runtime".to_string(),
            model_name: llm_node_models.get(&node_id).cloned(),
            elapsed_ms: 0,
            prompt_tokens: usage.map(|value| value.prompt_tokens),
            completion_tokens: usage.map(|value| value.completion_tokens),
            total_tokens: usage.map(|value| value.total_tokens),
            reasoning_tokens: usage.and_then(|value| value.reasoning_tokens),
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

    let output = YamlWorkflowRunOutput {
        workflow_id: workflow.id.clone(),
        entry_node: workflow.entry_node.clone(),
        email_text,
        trace,
        outputs,
        terminal_node,
        terminal_output,
        step_timings,
        llm_node_metrics,
        llm_node_models,
        total_elapsed_ms,
        ttft_ms: None,
        total_input_tokens: token_totals.input_tokens,
        total_output_tokens: token_totals.output_tokens,
        total_tokens: token_totals.total_tokens,
        total_reasoning_tokens: token_totals.reasoning_tokens,
        tokens_per_second: token_totals.tokens_per_second(total_elapsed_ms),
        trace_id: telemetry_context.trace_id.clone(),
        metadata: telemetry_context.trace_id.as_ref().map(|value| {
            workflow_metadata_with_trace(
                options,
                value,
                telemetry_context.sampled,
                telemetry_context.trace_id_source,
            )
        }),
    };

    if let Some(mut span) = workflow_span.take() {
        span.set_attribute("workflow_id", workflow.id.as_str());
        apply_trace_identity_attributes(span.as_mut(), telemetry_context.trace_id.as_deref());
        apply_langfuse_trace_input_output_attributes(
            span.as_mut(),
            workflow_input,
            &output,
            options.telemetry.payload_mode,
        );
        apply_langfuse_nerdstats_attributes(span.as_mut(), &output, options.telemetry.nerdstats);
        span.end();
        flush_workflow_tracer();
    }

    Ok(Some(output))
}

async fn execute_subworkflow_tool_call(
    payload: &Value,
    context: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    parent_options: &YamlWorkflowRunOptions,
    parent_trace_context: Option<&TraceContext>,
    resolved_trace_id: Option<&str>,
) -> Result<Value, String> {
    let workflow_id = payload
        .get("workflow_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "run_workflow_graph requires payload.workflow_id".to_string())?;

    let input_context = context
        .get("input")
        .and_then(Value::as_object)
        .ok_or_else(|| "run_workflow_graph requires context.input".to_string())?;

    let registry = input_context
        .get("workflow_registry")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "run_workflow_graph requires input.workflow_registry map of workflow_id -> yaml_path"
                .to_string()
        })?;

    let workflow_path = registry
        .get(workflow_id)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "workflow_registry has no entry for workflow_id '{}'",
                workflow_id
            )
        })?;

    let parent_depth = input_context
        .get("__subgraph_depth")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let max_depth = input_context
        .get("__subgraph_max_depth")
        .and_then(Value::as_u64)
        .unwrap_or(3);

    if parent_depth >= max_depth {
        return Err(format!(
            "run_workflow_graph depth limit reached (depth={}, max={})",
            parent_depth, max_depth
        ));
    }

    let mut subworkflow_input = payload
        .get("input")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if !subworkflow_input.contains_key("messages") {
        if let Some(messages) = input_context.get("messages") {
            subworkflow_input.insert("messages".to_string(), messages.clone());
        }
    }

    if !subworkflow_input.contains_key("email_text") {
        if let Some(email_text) = input_context.get("email_text") {
            subworkflow_input.insert("email_text".to_string(), email_text.clone());
        }
    }

    if !subworkflow_input.contains_key("workflow_registry") {
        subworkflow_input.insert(
            "workflow_registry".to_string(),
            Value::Object(registry.clone()),
        );
    }

    subworkflow_input.insert(
        "__subgraph_depth".to_string(),
        Value::Number(serde_json::Number::from(parent_depth + 1)),
    );
    subworkflow_input.insert(
        "__subgraph_max_depth".to_string(),
        Value::Number(serde_json::Number::from(max_depth)),
    );

    let subworkflow_options =
        build_subworkflow_options(parent_options, parent_trace_context, resolved_trace_id);

    let output = run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
        Path::new(workflow_path),
        &Value::Object(subworkflow_input),
        client,
        custom_worker,
        None,
        &subworkflow_options,
    )
    .await
    .map_err(|error| format!("subworkflow '{}' failed: {}", workflow_id, error))?;

    Ok(json!({
        "workflow_id": workflow_id,
        "workflow_path": workflow_path,
        "terminal_node": output.terminal_node,
        "terminal_output": output.terminal_output,
        "trace": output.trace,
    }))
}

fn build_subworkflow_options(
    parent_options: &YamlWorkflowRunOptions,
    parent_trace_context: Option<&TraceContext>,
    resolved_trace_id: Option<&str>,
) -> YamlWorkflowRunOptions {
    let mut subworkflow_options = parent_options.clone();
    if parent_trace_context.is_some() || resolved_trace_id.is_some() {
        let trace_context = YamlWorkflowTraceContextInput {
            trace_id: resolved_trace_id
                .map(|value| value.to_string())
                .or_else(|| parent_trace_context.and_then(|ctx| ctx.trace_id.clone())),
            span_id: parent_trace_context.and_then(|ctx| ctx.span_id.clone()),
            parent_span_id: parent_trace_context.and_then(|ctx| ctx.parent_span_id.clone()),
            traceparent: parent_trace_context.and_then(|ctx| ctx.traceparent.clone()),
            tracestate: parent_trace_context.and_then(|ctx| ctx.tracestate.clone()),
            baggage: parent_trace_context
                .map(|ctx| ctx.baggage.clone())
                .unwrap_or_default(),
        };
        subworkflow_options.trace.context = Some(trace_context);
    }
    subworkflow_options
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
        } else if self.node_type.end.is_some() {
            "end"
        } else {
            "unknown"
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlNodeType {
    pub llm_call: Option<YamlLlmCall>,
    pub switch: Option<YamlSwitch>,
    pub custom_worker: Option<YamlCustomWorker>,
    pub end: Option<Value>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YamlToolFormat {
    #[default]
    Openai,
    Simplified,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum YamlToolDeclaration {
    OpenAi(YamlOpenAiToolDeclaration),
    Simplified(YamlSimplifiedToolDeclaration),
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlOpenAiToolDeclaration {
    #[serde(rename = "type")]
    pub tool_type: Option<ToolType>,
    pub function: YamlOpenAiToolFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlOpenAiToolFunction {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlSimplifiedToolDeclaration {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum YamlToolChoiceConfig {
    Mode(ToolChoiceMode),
    Function { function: String },
    OpenAi(ToolChoiceTool),
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
#[serde(deny_unknown_fields)]
pub struct YamlCustomWorker {
    pub handler: String,
    pub handler_file: Option<String>,
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
    use simple_agent_type::provider::{Provider, ProviderRequest, ProviderResponse};
    use simple_agent_type::response::{CompletionChoice, CompletionResponse, Usage};
    use simple_agent_type::tool::{ToolCallFunction, ToolType};
    use simple_agent_type::{Result as SaResult, SimpleAgentsError};
    use simple_agents_core::SimpleAgentsClientBuilder;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

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

    struct HandlerFileCapturingWorker {
        handler_file: Mutex<Option<String>>,
    }

    struct ToolLoopProvider;

    struct UnknownToolProvider;

    struct ReasoningUsageProvider;

    struct ToolLoopReasoningProvider;

    #[derive(Default)]
    struct CapturingSpan {
        attributes: BTreeMap<String, String>,
    }

    impl WorkflowSpan for CapturingSpan {
        fn set_attribute(&mut self, key: &str, value: &str) {
            self.attributes.insert(key.to_string(), value.to_string());
        }

        fn add_event(&mut self, _name: &str) {}

        fn end(self: Box<Self>) {}
    }

    #[async_trait]
    impl Provider for ToolLoopProvider {
        fn name(&self) -> &str {
            "openai"
        }

        fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
            let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
            Ok(ProviderRequest::new("mock://tool-loop").with_body(body))
        }

        async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
            let request: CompletionRequest =
                serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

            let has_tools = request
                .tools
                .as_ref()
                .is_some_and(|tools| !tools.is_empty());
            let has_tool_result = request.messages.iter().any(|m| m.role == Role::Tool);

            let response = if has_tools && !has_tool_result {
                CompletionResponse {
                    id: "resp_tool_1".to_string(),
                    model: request.model.clone(),
                    choices: vec![CompletionChoice {
                        index: 0,
                        message: Message::assistant("").with_tool_calls(vec![ToolCall {
                            id: "call_get_context".to_string(),
                            tool_type: ToolType::Function,
                            function: ToolCallFunction {
                                name: "get_customer_context".to_string(),
                                arguments: "{\"order_id\":\"123\"}".to_string(),
                            },
                        }]),
                        finish_reason: FinishReason::ToolCalls,
                        logprobs: None,
                    }],
                    usage: Usage::new(10, 5),
                    created: None,
                    provider: Some(self.name().to_string()),
                    healing_metadata: None,
                }
            } else if has_tools && has_tool_result {
                CompletionResponse {
                    id: "resp_tool_2".to_string(),
                    model: request.model.clone(),
                    choices: vec![CompletionChoice {
                        index: 0,
                        message: Message::assistant("{\"state\":\"done\"}"),
                        finish_reason: FinishReason::Stop,
                        logprobs: None,
                    }],
                    usage: Usage::new(12, 6),
                    created: None,
                    provider: Some(self.name().to_string()),
                    healing_metadata: None,
                }
            } else {
                let prompt = request
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::User)
                    .map(|m| m.content.clone())
                    .unwrap_or_default();
                let payload = json!({
                    "subject": "ok",
                    "body": prompt,
                })
                .to_string();
                CompletionResponse {
                    id: "resp_final".to_string(),
                    model: request.model.clone(),
                    choices: vec![CompletionChoice {
                        index: 0,
                        message: Message::assistant(payload),
                        finish_reason: FinishReason::Stop,
                        logprobs: None,
                    }],
                    usage: Usage::new(8, 4),
                    created: None,
                    provider: Some(self.name().to_string()),
                    healing_metadata: None,
                }
            };

            let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
            Ok(ProviderResponse::new(200, body))
        }

        fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
            serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
        }
    }

    #[async_trait]
    impl Provider for UnknownToolProvider {
        fn name(&self) -> &str {
            "openai"
        }

        fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
            let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
            Ok(ProviderRequest::new("mock://unknown-tool").with_body(body))
        }

        async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
            let request: CompletionRequest =
                serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

            let response = CompletionResponse {
                id: "resp_unknown_tool".to_string(),
                model: request.model,
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("").with_tool_calls(vec![ToolCall {
                        id: "call_unknown".to_string(),
                        tool_type: ToolType::Function,
                        function: ToolCallFunction {
                            name: "unknown_tool".to_string(),
                            arguments: "{}".to_string(),
                        },
                    }]),
                    finish_reason: FinishReason::ToolCalls,
                    logprobs: None,
                }],
                usage: Usage::new(5, 2),
                created: None,
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            };

            let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
            Ok(ProviderResponse::new(200, body))
        }

        fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
            serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
        }
    }

    #[async_trait]
    impl Provider for ReasoningUsageProvider {
        fn name(&self) -> &str {
            "openai"
        }

        fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
            let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
            Ok(ProviderRequest::new("mock://reasoning-usage").with_body(body))
        }

        async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
            let request: CompletionRequest =
                serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

            let mut usage = Usage::new(9, 5);
            usage.reasoning_tokens = Some(4);
            let response = CompletionResponse {
                id: "resp_reasoning".to_string(),
                model: request.model,
                choices: vec![CompletionChoice {
                    index: 0,
                    message: Message::assistant("{\"state\":\"ok\"}"),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                }],
                usage,
                created: None,
                provider: Some(self.name().to_string()),
                healing_metadata: None,
            };

            let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
            Ok(ProviderResponse::new(200, body))
        }

        fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
            serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
        }
    }

    #[async_trait]
    impl Provider for ToolLoopReasoningProvider {
        fn name(&self) -> &str {
            "openai"
        }

        fn transform_request(&self, req: &CompletionRequest) -> SaResult<ProviderRequest> {
            let body = serde_json::to_value(req).map_err(SimpleAgentsError::from)?;
            Ok(ProviderRequest::new("mock://tool-loop-reasoning").with_body(body))
        }

        async fn execute(&self, req: ProviderRequest) -> SaResult<ProviderResponse> {
            let request: CompletionRequest =
                serde_json::from_value(req.body).map_err(SimpleAgentsError::from)?;

            let has_tools = request
                .tools
                .as_ref()
                .is_some_and(|tools| !tools.is_empty());
            let has_tool_result = request.messages.iter().any(|m| m.role == Role::Tool);

            let response = if has_tools && !has_tool_result {
                let mut usage = Usage::new(10, 5);
                usage.reasoning_tokens = Some(2);
                CompletionResponse {
                    id: "resp_tool_reasoning_1".to_string(),
                    model: request.model.clone(),
                    choices: vec![CompletionChoice {
                        index: 0,
                        message: Message::assistant("").with_tool_calls(vec![ToolCall {
                            id: "call_get_context".to_string(),
                            tool_type: ToolType::Function,
                            function: ToolCallFunction {
                                name: "get_customer_context".to_string(),
                                arguments: "{\"order_id\":\"123\"}".to_string(),
                            },
                        }]),
                        finish_reason: FinishReason::ToolCalls,
                        logprobs: None,
                    }],
                    usage,
                    created: None,
                    provider: Some(self.name().to_string()),
                    healing_metadata: None,
                }
            } else {
                let mut usage = Usage::new(12, 6);
                usage.reasoning_tokens = Some(3);
                CompletionResponse {
                    id: "resp_tool_reasoning_2".to_string(),
                    model: request.model,
                    choices: vec![CompletionChoice {
                        index: 0,
                        message: Message::assistant("{\"state\":\"done\"}"),
                        finish_reason: FinishReason::Stop,
                        logprobs: None,
                    }],
                    usage,
                    created: None,
                    provider: Some(self.name().to_string()),
                    healing_metadata: None,
                }
            };

            let body = serde_json::to_value(response).map_err(SimpleAgentsError::from)?;
            Ok(ProviderResponse::new(200, body))
        }

        fn transform_response(&self, resp: ProviderResponse) -> SaResult<CompletionResponse> {
            serde_json::from_value(resp.body).map_err(SimpleAgentsError::from)
        }
    }

    struct FixedToolWorker {
        payload: Value,
    }

    struct CountingToolWorker {
        execute_calls: AtomicUsize,
    }

    #[async_trait]
    impl YamlWorkflowCustomWorkerExecutor for FixedToolWorker {
        async fn execute(
            &self,
            _handler: &str,
            _handler_file: Option<&str>,
            _payload: &Value,
            _email_text: &str,
            _context: &Value,
        ) -> Result<Value, String> {
            Ok(self.payload.clone())
        }
    }

    #[async_trait]
    impl YamlWorkflowCustomWorkerExecutor for CountingToolWorker {
        async fn execute(
            &self,
            _handler: &str,
            _handler_file: Option<&str>,
            _payload: &Value,
            _email_text: &str,
            _context: &Value,
        ) -> Result<Value, String> {
            self.execute_calls.fetch_add(1, Ordering::SeqCst);
            Ok(json!({"ok": true}))
        }
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
                tool_calls: Vec::new(),
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
            _handler_file: Option<&str>,
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
    impl YamlWorkflowCustomWorkerExecutor for HandlerFileCapturingWorker {
        async fn execute(
            &self,
            _handler: &str,
            handler_file: Option<&str>,
            _payload: &Value,
            _email_text: &str,
            _context: &Value,
        ) -> Result<Value, String> {
            let mut guard = self
                .handler_file
                .lock()
                .map_err(|_| "handler-file lock should not be poisoned".to_string())?;
            *guard = handler_file.map(ToString::to_string);
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
                        reasoning_tokens: None,
                    }),
                    ttft_ms: None,
                    tool_calls: Vec::new(),
                });
            }
            if prompt.contains("Determine termination subtype") {
                return Ok(YamlLlmExecutionResult {
                    payload: json!({"subtype":"repeated_offense","reason":"mock"}),
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: 12,
                        completion_tokens: 6,
                        total_tokens: 18,
                        reasoning_tokens: None,
                    }),
                    ttft_ms: None,
                    tool_calls: Vec::new(),
                });
            }
            if prompt.contains("Determine supply chain subtype") {
                return Ok(YamlLlmExecutionResult {
                    payload: json!({"subtype":"order_replacement","reason":"mock"}),
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: 11,
                        completion_tokens: 4,
                        total_tokens: 15,
                        reasoning_tokens: None,
                    }),
                    ttft_ms: None,
                    tool_calls: Vec::new(),
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
        let custom_worker = FixedToolWorker {
            payload: json!({"context": "mock"}),
        };
        let output = run_email_workflow_yaml_with_custom_worker(
            &workflow,
            "test",
            &MockExecutor,
            Some(&custom_worker),
        )
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
    async fn workflow_runner_executes_inline_workflow_with_executor() {
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
        let output = WorkflowRunner::from_workflow(&workflow)
            .with_email_text("test")
            .with_executor(&MockExecutor)
            .run()
            .await
            .expect("builder execution should succeed");

        assert_eq!(output.workflow_id, "email-intake-classification");
        assert!(output.outputs.contains_key("classify_top_level"));
    }

    #[tokio::test]
    async fn workflow_runner_requires_executor() {
        let yaml = r#"
id: missing-executor
entry_node: end
nodes:
  - id: end
    node_type:
      end: {}
"#;
        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");

        let err = WorkflowRunner::from_workflow(&workflow)
            .with_input(&json!({"email_text":"x"}))
            .run()
            .await
            .expect_err("missing executor should fail");

        assert!(matches!(
            err,
            YamlWorkflowRunError::InvalidInput { message }
                if message.contains("workflow executor is required")
        ));
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
        assert!(nerdstats.get("step_timings").is_none());
        assert!(nerdstats.get("llm_node_metrics").is_none());
        assert!(nerdstats.get("step_details").is_some());
        assert!(nerdstats.get("llm_node_models").is_none());
        assert_eq!(
            nerdstats["step_details"][0]["model_name"],
            Value::String("gpt-4.1".to_string())
        );
        assert_eq!(
            nerdstats["step_details"][0]["node_id"],
            Value::String("classify".to_string())
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
                    reasoning_tokens: None,
                }),
                ttft_ms: if request.stream { Some(12) } else { None },
                tool_calls: Vec::new(),
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
        assert!(nerdstats.get("step_timings").is_none());
        assert!(nerdstats.get("llm_node_metrics").is_none());
        assert_eq!(
            nerdstats["step_details"][0]["model_name"],
            Value::String("gpt-4.1".to_string())
        );
        assert_eq!(
            nerdstats["step_details"][0]["node_id"],
            Value::String("classify".to_string())
        );
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
                model_name: Some("gpt-4.1".to_string()),
                elapsed_ms: 100,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                reasoning_tokens: None,
                tokens_per_second: None,
            }],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 100,
            ttft_ms: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            total_reasoning_tokens: None,
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
        assert!(nerdstats.get("step_timings").is_none());
        assert!(nerdstats.get("llm_node_metrics").is_none());
        assert_eq!(
            nerdstats["step_details"][0]["node_id"],
            Value::String("llm_node".to_string())
        );
        assert_eq!(
            nerdstats["step_details"][0]["model_name"],
            Value::String("gpt-4.1".to_string())
        );
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
                model_name: Some("gpt-4.1".to_string()),
                elapsed_ms: 100,
                prompt_tokens: Some(10),
                completion_tokens: Some(15),
                total_tokens: Some(25),
                reasoning_tokens: None,
                tokens_per_second: Some(150.0),
            }],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 100,
            ttft_ms: Some(42),
            total_input_tokens: 10,
            total_output_tokens: 15,
            total_tokens: 25,
            total_reasoning_tokens: None,
            tokens_per_second: 150.0,
            trace_id: Some("trace-2".to_string()),
            metadata: None,
        };

        let nerdstats = workflow_nerdstats(&output);
        assert_eq!(nerdstats["ttft_ms"], Value::Number(42u64.into()));
        assert!(nerdstats.get("step_timings").is_none());
        assert!(nerdstats.get("llm_node_metrics").is_none());
        assert_eq!(
            nerdstats["step_details"][0]["node_id"],
            Value::String("llm_node".to_string())
        );
        assert_eq!(
            nerdstats["step_details"][0]["model_name"],
            Value::String("gpt-4.1".to_string())
        );
    }

    #[test]
    fn workflow_nerdstats_schema_contract_is_stable() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "schema-workflow".to_string(),
            entry_node: "start".to_string(),
            email_text: "hello".to_string(),
            trace: vec!["classify".to_string(), "route".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "route".to_string(),
            terminal_output: None,
            step_timings: vec![
                YamlStepTiming {
                    node_id: "classify".to_string(),
                    node_kind: "llm_call".to_string(),
                    model_name: Some("gpt-4.1".to_string()),
                    elapsed_ms: 100,
                    prompt_tokens: Some(11),
                    completion_tokens: Some(22),
                    total_tokens: Some(33),
                    reasoning_tokens: Some(7),
                    tokens_per_second: Some(220.0),
                },
                YamlStepTiming {
                    node_id: "route".to_string(),
                    node_kind: "switch".to_string(),
                    model_name: None,
                    elapsed_ms: 0,
                    prompt_tokens: None,
                    completion_tokens: None,
                    total_tokens: None,
                    reasoning_tokens: None,
                    tokens_per_second: None,
                },
            ],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 100,
            ttft_ms: Some(9),
            total_input_tokens: 11,
            total_output_tokens: 22,
            total_tokens: 33,
            total_reasoning_tokens: Some(7),
            tokens_per_second: 220.0,
            trace_id: Some("trace-schema".to_string()),
            metadata: None,
        };

        let nerdstats = workflow_nerdstats(&output);
        let expected = json!({
            "workflow_id": "schema-workflow",
            "terminal_node": "route",
            "total_elapsed_ms": 100,
            "ttft_ms": 9,
            "step_details": [
                {
                    "node_id": "classify",
                    "node_kind": "llm_call",
                    "model_name": "gpt-4.1",
                    "elapsed_ms": 100,
                    "prompt_tokens": 11,
                    "completion_tokens": 22,
                    "total_tokens": 33,
                    "reasoning_tokens": 7,
                    "tokens_per_second": 220.0
                },
                {
                    "node_id": "route",
                    "node_kind": "switch",
                    "elapsed_ms": 0
                }
            ],
            "total_input_tokens": 11,
            "total_output_tokens": 22,
            "total_tokens": 33,
            "total_reasoning_tokens": 7,
            "tokens_per_second": 220.0,
            "trace_id": "trace-schema",
            "token_metrics_available": true,
            "token_metrics_source": "provider_usage",
            "llm_nodes_without_usage": []
        });

        assert_eq!(nerdstats, expected);
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
                    reasoning_tokens: None,
                }),
                ttft_ms: None,
                tool_calls: Vec::new(),
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
    async fn yaml_llm_tool_calling_captures_traces_and_supports_globals_reference() {
        let yaml = r#"
id: tool-calling-workflow
entry_node: generate_with_tool
nodes:
  - id: generate_with_tool
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: simplified
        max_tool_roundtrips: 1
        tool_calls_global_key: audit
        tools:
          - name: get_customer_context
            description: Fetch customer context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
              additionalProperties: false
            output_schema:
              type: object
              properties:
                customer_name: { type: string }
              required: [customer_name]
              additionalProperties: false
    config:
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
  - id: personalize
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Write an email greeting for {{ globals.audit.0.output.customer_name }}.
      output_schema:
        type: object
        properties:
          subject: { type: string }
          body: { type: string }
        required: [subject, body]
edges:
  - from: generate_with_tool
    to: personalize
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(Arc::new(ToolLoopProvider))
            .build()
            .expect("client should build");
        let worker = FixedToolWorker {
            payload: json!({"customer_name": "Ava"}),
        };

        let output = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &client,
            Some(&worker),
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect("workflow should execute");

        assert_eq!(output.trace, vec!["generate_with_tool", "personalize"]);
        assert_eq!(
            output.outputs["generate_with_tool"]["tool_calls"][0]["output"]["customer_name"],
            Value::String("Ava".to_string())
        );
        let body = output.outputs["personalize"]["output"]["body"]
            .as_str()
            .expect("body should be string");
        assert!(body.contains("Ava"));
    }

    #[tokio::test]
    async fn workflow_with_client_preserves_reasoning_tokens_in_output_and_nerdstats() {
        let yaml = r#"
id: reasoning-usage-workflow
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      prompt: |
        Return JSON only:
        {"state":"ok"}
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(Arc::new(ReasoningUsageProvider))
            .build()
            .expect("client should build");

        let output = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &client,
            None,
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect("workflow should execute");

        assert_eq!(output.total_reasoning_tokens, Some(4));
        assert_eq!(output.step_timings.len(), 1);
        assert_eq!(output.step_timings[0].reasoning_tokens, Some(4));

        let nerdstats = workflow_nerdstats(&output);
        assert_eq!(
            nerdstats["total_reasoning_tokens"],
            Value::Number(4u64.into())
        );
        assert_eq!(
            nerdstats["step_details"][0]["reasoning_tokens"],
            Value::Number(4u64.into())
        );
    }

    #[tokio::test]
    async fn workflow_with_tools_accumulates_reasoning_tokens_across_roundtrips() {
        let yaml = r#"
id: tool-reasoning-workflow
entry_node: generate_with_tool
nodes:
  - id: generate_with_tool
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: simplified
        max_tool_roundtrips: 1
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
              additionalProperties: false
            output_schema:
              type: object
              properties:
                customer_name: { type: string }
              required: [customer_name]
              additionalProperties: false
    config:
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(Arc::new(ToolLoopReasoningProvider))
            .build()
            .expect("client should build");
        let worker = FixedToolWorker {
            payload: json!({"customer_name": "Ava"}),
        };

        let output = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &client,
            Some(&worker),
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect("workflow should execute");

        assert_eq!(output.total_reasoning_tokens, Some(5));
        assert_eq!(output.step_timings.len(), 1);
        assert_eq!(output.step_timings[0].reasoning_tokens, Some(5));

        let nerdstats = workflow_nerdstats(&output);
        assert_eq!(
            nerdstats["total_reasoning_tokens"],
            Value::Number(5u64.into())
        );
        assert_eq!(
            nerdstats["step_details"][0]["reasoning_tokens"],
            Value::Number(5u64.into())
        );
    }

    #[tokio::test]
    async fn yaml_llm_tool_output_schema_mismatch_hard_fails_node() {
        let yaml = r#"
id: tool-calling-schema-fail
entry_node: generate_with_tool
nodes:
  - id: generate_with_tool
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: simplified
        max_tool_roundtrips: 1
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
              additionalProperties: false
            output_schema:
              type: object
              properties:
                customer_name: { type: string }
              required: [customer_name]
              additionalProperties: false
    config:
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(Arc::new(ToolLoopProvider))
            .build()
            .expect("client should build");
        let worker = FixedToolWorker {
            payload: json!({"unexpected": "shape"}),
        };

        let error = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &client,
            Some(&worker),
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect_err("workflow should hard-fail on schema mismatch");

        match error {
            YamlWorkflowRunError::Llm { message, .. } => {
                assert!(message.contains("output failed schema validation"));
            }
            other => panic!("expected llm error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn yaml_llm_unknown_tool_is_rejected_before_custom_worker_execution() {
        let yaml = r#"
id: unknown-tool-rejected
entry_node: generate_with_tool
nodes:
  - id: generate_with_tool
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: simplified
        max_tool_roundtrips: 1
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
    config:
      output_schema:
        type: object
        properties:
          state: { type: string }
        required: [state]
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(Arc::new(UnknownToolProvider))
            .build()
            .expect("client should build");
        let worker = CountingToolWorker {
            execute_calls: AtomicUsize::new(0),
        };

        let error = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &client,
            Some(&worker),
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect_err("workflow should reject unknown tool before executing worker");

        match error {
            YamlWorkflowRunError::Llm { message, .. } => {
                assert!(message.contains("model requested unknown tool 'unknown_tool'"));
            }
            other => panic!("expected llm error, got {other:?}"),
        }

        assert_eq!(worker.execute_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn validates_tools_format_mismatch() {
        let yaml = r#"
id: mismatch
entry_node: generate
nodes:
  - id: generate
    node_type:
      llm_call:
        model: gpt-4.1
        tools_format: openai
        tools:
          - name: get_customer_context
            input_schema:
              type: object
              properties:
                order_id: { type: string }
              required: [order_id]
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let diagnostics = verify_yaml_workflow(&workflow);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "invalid_tools_format"));
    }

    #[tokio::test]
    async fn custom_worker_node_requires_executor() {
        let yaml = r#"
id: missing-custom-worker
entry_node: enrich
nodes:
  - id: enrich
    node_type:
      custom_worker:
        handler: GetEmployeeRecord
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(Arc::new(ToolLoopProvider))
            .build()
            .expect("client should build");

        let error = run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text": "hello"}),
            &client,
            None,
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect_err("workflow should fail without custom worker executor");

        match error {
            YamlWorkflowRunError::CustomWorker { node_id, message } => {
                assert_eq!(node_id, "enrich");
                assert!(message.contains("requires a configured custom worker executor"));
            }
            other => panic!("expected custom worker error, got {other:?}"),
        }
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
    fn renders_yaml_workflow_tools_as_colored_tool_nodes() {
        let yaml = r#"
id: tool-graph
entry_node: chat
nodes:
  - id: chat
    node_type:
      llm_call:
        model: gemini-3-flash
        tools_format: simplified
        tools:
          - name: run_workflow_graph
            input_schema:
              type: object
              properties:
                workflow_id: { type: string }
              required: [workflow_id]
edges: []
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let mermaid = yaml_workflow_to_mermaid(&workflow);

        assert!(mermaid.contains("chat__tool_0"));
        assert!(mermaid.contains("chat -.-> chat__tool_0"));
        assert!(mermaid.contains("classDef toolNode"));
        assert!(mermaid.contains("class chat__tool_0 toolNode;"));
    }

    #[test]
    fn renders_yaml_workflow_file_to_mermaid_with_subgraph_cluster_when_present() {
        let base_dir = std::env::temp_dir().join(format!(
            "simple_agents_mermaid_subgraph_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base_dir).expect("temp dir should be created");

        let orchestrator_path = base_dir.join("email-chat-orchestrator-with-subgraph-tool.yaml");
        let subgraph_path = base_dir.join("hr-warning-email-subgraph.yaml");

        let orchestrator_yaml = r#"
id: email-chat-orchestrator-with-subgraph-tool
entry_node: respond_casual
nodes:
  - id: respond_casual
    node_type:
      llm_call:
        model: gemini-3-flash
        tools_format: simplified
        tools:
          - name: run_workflow_graph
            input_schema:
              type: object
              properties:
                workflow_id: { type: string }
              required: [workflow_id]
    config:
      prompt: |
        Call with:
        {
          "workflow_id": "hr-warning-email-subgraph"
        }
edges: []
"#;

        let subgraph_yaml = r#"
id: hr-warning-email-subgraph
entry_node: draft_hr_warning_email
nodes:
  - id: draft_hr_warning_email
    node_type:
      llm_call:
        model: gemini-3-flash
edges: []
"#;

        fs::write(&orchestrator_path, orchestrator_yaml).expect("orchestrator yaml written");
        fs::write(&subgraph_path, subgraph_yaml).expect("subgraph yaml written");

        let mermaid =
            yaml_workflow_file_to_mermaid(&orchestrator_path).expect("mermaid should render");

        assert!(mermaid.contains("Main: email-chat-orchestrator-with-subgraph-tool"));
        assert!(mermaid.contains("Subgraph: hr-warning-email-subgraph"));
        assert!(mermaid.contains("calls hr-warning-email-subgraph"));
        assert!(mermaid.contains("subgraph_1__draft_hr_warning_email"));

        fs::remove_dir_all(base_dir).expect("temp dir removed");
    }

    #[tokio::test]
    async fn rejects_non_yaml_workflow_file_extension() {
        let file_path = std::env::temp_dir().join(format!(
            "simple_agents_workflow_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        fs::write(&file_path, "not yaml").expect("temporary file should be created");

        let result =
            run_workflow_yaml_file(&file_path, &json!({"email_text": "hello"}), &MockExecutor)
                .await;

        match result {
            Err(YamlWorkflowRunError::FileRejected { path, reason }) => {
                assert!(path.ends_with(".txt"));
                assert!(reason.contains(".yaml or .yml"));
            }
            other => panic!("expected file rejection error, got {other:?}"),
        }

        let _ = fs::remove_file(&file_path);
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

    #[test]
    fn yaml_to_ir_preserves_llm_sampling_controls() {
        let yaml = r#"
id: llm-controls
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
        max_tokens: 120
        temperature: 0.3
        top_p: 0.9
    config:
      prompt: "classify"
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let ir = yaml_workflow_to_ir(&workflow).expect("yaml should convert to ir");
        let llm_tool_input = ir
            .nodes
            .iter()
            .find_map(|node| {
                if let crate::ir::NodeKind::Tool { tool, input, .. } = &node.kind {
                    if tool == "__yaml_llm_call" {
                        return Some(input);
                    }
                }
                None
            })
            .expect("expected __yaml_llm_call tool node");

        assert_eq!(
            llm_tool_input.get("max_tokens").and_then(Value::as_u64),
            Some(120)
        );
        let temperature = llm_tool_input
            .get("temperature")
            .and_then(Value::as_f64)
            .expect("temperature should be preserved");
        assert!((temperature - 0.3).abs() < 1e-6);
        let top_p = llm_tool_input
            .get("top_p")
            .and_then(Value::as_f64)
            .expect("top_p should be preserved");
        assert!((top_p - 0.9).abs() < 1e-6);
    }

    #[tokio::test]
    async fn ir_custom_worker_path_preserves_handler_file() {
        let yaml = r#"
id: worker-handler-file
entry_node: worker
nodes:
  - id: worker
    node_type:
      custom_worker:
        handler: GetRagData
        handler_file: handlers.py
    config:
      payload:
        topic: clarification
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let worker = HandlerFileCapturingWorker {
            handler_file: Mutex::new(None),
        };

        run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &MockExecutor,
            Some(&worker),
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect("workflow should execute");

        let captured = worker
            .handler_file
            .lock()
            .expect("handler-file lock should not be poisoned")
            .clone();
        assert_eq!(captured.as_deref(), Some("handlers.py"));
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
    async fn workflow_run_options_sample_rate_zero_marks_trace_unsampled() {
        let yaml = r#"
id: sample-rate-zero
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
                sample_rate: 0.0,
                ..YamlWorkflowTelemetryConfig::default()
            },
            trace: YamlWorkflowTraceOptions {
                context: Some(YamlWorkflowTraceContextInput {
                    trace_id: Some("trace-sample-zero".to_string()),
                    ..YamlWorkflowTraceContextInput::default()
                }),
                tenant: YamlWorkflowTraceTenantContext::default(),
            },
            model: None,
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

        assert_eq!(output.trace_id.as_deref(), Some("trace-sample-zero"));
        assert_eq!(
            output
                .metadata
                .as_ref()
                .and_then(|value| value.get("telemetry"))
                .and_then(|telemetry| telemetry.get("sampled"))
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn trace_id_from_traceparent_parses_w3c_header() {
        let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        assert_eq!(
            trace_id_from_traceparent(traceparent).as_deref(),
            Some("4bf92f3577b34da6a3ce929d0e0e4736")
        );
    }

    #[test]
    fn resolve_telemetry_context_marks_traceparent_source() {
        let mut options = YamlWorkflowRunOptions::default();
        options.trace.context = Some(YamlWorkflowTraceContextInput {
            traceparent: Some(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
            ),
            ..YamlWorkflowTraceContextInput::default()
        });

        let context = resolve_telemetry_context(&options, None);

        assert_eq!(context.trace_id_source, TraceIdSource::Traceparent);
        assert_eq!(
            context.trace_id.as_deref(),
            Some("4bf92f3577b34da6a3ce929d0e0e4736")
        );
    }

    #[tokio::test]
    async fn workflow_run_options_derives_trace_id_from_traceparent_when_trace_id_missing() {
        let yaml = r#"
id: traceparent-derived
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
        let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let options = YamlWorkflowRunOptions {
            telemetry: YamlWorkflowTelemetryConfig {
                sample_rate: 0.0,
                ..YamlWorkflowTelemetryConfig::default()
            },
            trace: YamlWorkflowTraceOptions {
                context: Some(YamlWorkflowTraceContextInput {
                    traceparent: Some(traceparent.to_string()),
                    ..YamlWorkflowTraceContextInput::default()
                }),
                tenant: YamlWorkflowTraceTenantContext::default(),
            },
            model: None,
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

        assert_eq!(
            output.trace_id.as_deref(),
            Some("4bf92f3577b34da6a3ce929d0e0e4736")
        );
        assert_eq!(
            output
                .metadata
                .as_ref()
                .and_then(|value| value.get("telemetry"))
                .and_then(|telemetry| telemetry.get("sampled"))
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[tokio::test]
    async fn workflow_run_options_sample_rate_one_marks_trace_sampled() {
        let yaml = r#"
id: sample-rate-one
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
                sample_rate: 1.0,
                ..YamlWorkflowTelemetryConfig::default()
            },
            trace: YamlWorkflowTraceOptions {
                context: Some(YamlWorkflowTraceContextInput {
                    trace_id: Some("trace-sample-one".to_string()),
                    ..YamlWorkflowTraceContextInput::default()
                }),
                tenant: YamlWorkflowTraceTenantContext::default(),
            },
            model: None,
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

        assert_eq!(output.trace_id.as_deref(), Some("trace-sample-one"));
        assert_eq!(
            output
                .metadata
                .as_ref()
                .and_then(|value| value.get("telemetry"))
                .and_then(|telemetry| telemetry.get("sampled"))
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn workflow_run_options_sampling_is_deterministic_for_fixed_trace_id() {
        let yaml = r#"
id: sample-rate-deterministic
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
                sample_rate: 0.5,
                ..YamlWorkflowTelemetryConfig::default()
            },
            trace: YamlWorkflowTraceOptions {
                context: Some(YamlWorkflowTraceContextInput {
                    trace_id: Some("trace-sample-deterministic".to_string()),
                    ..YamlWorkflowTraceContextInput::default()
                }),
                tenant: YamlWorkflowTraceTenantContext::default(),
            },
            model: None,
        };

        let mut sampled_values = Vec::new();
        for _ in 0..3 {
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

            let sampled = output
                .metadata
                .as_ref()
                .and_then(|value| value.get("telemetry"))
                .and_then(|telemetry| telemetry.get("sampled"))
                .and_then(Value::as_bool)
                .expect("sampled flag should be present");
            sampled_values.push(sampled);
        }

        assert!(sampled_values
            .iter()
            .all(|value| *value == sampled_values[0]));
    }

    #[tokio::test]
    async fn workflow_run_options_reject_invalid_sample_rate() {
        let yaml = r#"
id: sample-rate-invalid
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
                sample_rate: 1.1,
                ..YamlWorkflowTelemetryConfig::default()
            },
            ..YamlWorkflowRunOptions::default()
        };

        let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &MockExecutor,
            None,
            None,
            &options,
        )
        .await
        .expect_err("invalid sample_rate should fail");

        match err {
            YamlWorkflowRunError::InvalidInput { message } => {
                assert!(message.contains("telemetry.sample_rate"));
            }
            _ => panic!("expected invalid input error for sample_rate"),
        }
    }

    #[tokio::test]
    async fn workflow_run_options_reject_nan_sample_rate() {
        let yaml = r#"
id: sample-rate-invalid
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
                sample_rate: f32::NAN,
                ..YamlWorkflowTelemetryConfig::default()
            },
            ..YamlWorkflowRunOptions::default()
        };

        let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text":"hello"}),
            &MockExecutor,
            None,
            None,
            &options,
        )
        .await
        .expect_err("nan sample_rate should fail");

        assert!(matches!(err, YamlWorkflowRunError::InvalidInput { .. }));
    }

    #[test]
    fn subworkflow_options_inherit_parent_telemetry_configuration() {
        let mut parent_options = YamlWorkflowRunOptions::default();
        parent_options.telemetry.sample_rate = 0.0;
        parent_options.telemetry.payload_mode = YamlWorkflowPayloadMode::RedactedPayload;
        parent_options.telemetry.tool_trace_mode = YamlToolTraceMode::Redacted;
        parent_options.trace.tenant.conversation_id = Some("conv-123".to_string());

        let parent_context = TraceContext {
            trace_id: Some("trace-parent".to_string()),
            span_id: Some("span-parent".to_string()),
            parent_span_id: None,
            traceparent: Some("00-trace-parent-span-parent-01".to_string()),
            tracestate: None,
            baggage: BTreeMap::new(),
        };

        let subworkflow_options =
            build_subworkflow_options(&parent_options, Some(&parent_context), None);

        assert_eq!(subworkflow_options.telemetry.sample_rate, 0.0);
        assert_eq!(
            subworkflow_options.telemetry.payload_mode,
            YamlWorkflowPayloadMode::RedactedPayload
        );
        assert_eq!(
            subworkflow_options.telemetry.tool_trace_mode,
            YamlToolTraceMode::Redacted
        );
        assert_eq!(
            subworkflow_options.trace.tenant.conversation_id.as_deref(),
            Some("conv-123")
        );
        assert_eq!(
            subworkflow_options
                .trace
                .context
                .as_ref()
                .and_then(|ctx| ctx.trace_id.as_deref()),
            Some("trace-parent")
        );
    }

    #[test]
    fn trace_tenant_attributes_include_langfuse_aliases() {
        let tenant = YamlWorkflowTraceTenantContext {
            workspace_id: Some("ws-1".to_string()),
            user_id: Some("user-1".to_string()),
            conversation_id: Some("conv-1".to_string()),
            request_id: Some("req-1".to_string()),
            run_id: Some("run-1".to_string()),
        };
        let mut span = CapturingSpan::default();
        apply_trace_tenant_attributes_from_tenant(&mut span, &tenant);

        assert_eq!(
            span.attributes
                .get("tenant.workspace_id")
                .map(String::as_str),
            Some("ws-1")
        );
        assert_eq!(
            span.attributes.get("tenant.user_id").map(String::as_str),
            Some("user-1")
        );
        assert_eq!(
            span.attributes
                .get("tenant.conversation_id")
                .map(String::as_str),
            Some("conv-1")
        );
        assert_eq!(
            span.attributes.get("langfuse.user.id").map(String::as_str),
            Some("user-1")
        );
        assert_eq!(
            span.attributes
                .get("langfuse.session.id")
                .map(String::as_str),
            Some("conv-1")
        );
    }

    #[test]
    fn langfuse_nerdstats_attributes_are_written_when_enabled() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "wf-1".to_string(),
            entry_node: "start".to_string(),
            email_text: "email".to_string(),
            trace: vec!["node-1".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "node-1".to_string(),
            terminal_output: None,
            step_timings: vec![YamlStepTiming {
                node_id: "node-1".to_string(),
                node_kind: "llm_call".to_string(),
                model_name: Some("gpt-4.1-mini".to_string()),
                elapsed_ms: 42,
                prompt_tokens: Some(4),
                completion_tokens: Some(6),
                total_tokens: Some(10),
                reasoning_tokens: Some(2),
                tokens_per_second: Some(14.0),
            }],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 42,
            ttft_ms: Some(7),
            total_input_tokens: 4,
            total_output_tokens: 6,
            total_tokens: 10,
            total_reasoning_tokens: Some(2),
            tokens_per_second: 14.0,
            trace_id: Some("trace-1".to_string()),
            metadata: None,
        };

        let mut span = CapturingSpan::default();
        apply_langfuse_nerdstats_attributes(&mut span, &output, true);

        assert_eq!(
            span.attributes
                .get("langfuse.trace.metadata.nerdstats.workflow_id")
                .map(String::as_str),
            Some("wf-1")
        );
        assert_eq!(
            span.attributes
                .get("langfuse.trace.metadata.nerdstats.total_tokens")
                .map(String::as_str),
            Some("10")
        );
        assert_eq!(
            span.attributes
                .get("langfuse.trace.metadata.nerdstats.token_metrics_available")
                .map(String::as_str),
            Some("true")
        );
        assert!(span
            .attributes
            .contains_key("langfuse.trace.metadata.nerdstats"));
    }

    #[test]
    fn langfuse_trace_input_output_and_usage_are_written() {
        let output = YamlWorkflowRunOutput {
            workflow_id: "wf-1".to_string(),
            entry_node: "start".to_string(),
            email_text: "email".to_string(),
            trace: vec!["node-1".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "node-1".to_string(),
            terminal_output: Some(json!({"final":"ok"})),
            step_timings: Vec::new(),
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 10,
            ttft_ms: None,
            total_input_tokens: 11,
            total_output_tokens: 22,
            total_tokens: 33,
            total_reasoning_tokens: Some(4),
            tokens_per_second: 1.5,
            trace_id: Some("trace-1".to_string()),
            metadata: None,
        };

        let mut span = CapturingSpan::default();
        let input = json!({"email_text":"hello"});
        apply_langfuse_trace_input_output_attributes(
            &mut span,
            &input,
            &output,
            YamlWorkflowPayloadMode::FullPayload,
        );

        assert_eq!(
            span.attributes
                .get("langfuse.trace.input")
                .map(String::as_str),
            Some("{\"email_text\":\"hello\"}")
        );
        assert_eq!(
            span.attributes
                .get("langfuse.trace.output")
                .map(String::as_str),
            Some("{\"final\":\"ok\"}")
        );
        assert!(span
            .attributes
            .contains_key("langfuse.trace.metadata.usage_details"));
    }

    #[test]
    fn langfuse_observation_usage_attributes_are_written() {
        let usage = YamlLlmTokenUsage {
            prompt_tokens: 7,
            completion_tokens: 9,
            total_tokens: 16,
            reasoning_tokens: Some(3),
        };
        let mut span = CapturingSpan::default();
        apply_langfuse_observation_usage_attributes(&mut span, &usage);

        assert_eq!(
            span.attributes
                .get("gen_ai.usage.input_tokens")
                .map(String::as_str),
            Some("7")
        );
        assert_eq!(
            span.attributes
                .get("gen_ai.usage.output_tokens")
                .map(String::as_str),
            Some("9")
        );
        assert_eq!(
            span.attributes
                .get("gen_ai.usage.total_tokens")
                .map(String::as_str),
            Some("16")
        );
        assert_eq!(
            span.attributes
                .get("gen_ai.usage.reasoning_tokens")
                .map(String::as_str),
            Some("3")
        );
        assert!(span
            .attributes
            .contains_key("langfuse.observation.usage_details"));
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
            model: None,
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
        let rewritten = rewrite_yaml_condition_to_ir(expr).expect("condition should rewrite");
        assert_eq!(rewritten, "$.node_outputs.classify.output_total == 1");
    }

    #[test]
    fn rewrite_yaml_condition_does_not_mutate_paths_in_string_literals() {
        let expr = "$.nodes.classify.output.state == \"$.nodes.keep.output\"";
        let rewritten = rewrite_yaml_condition_to_ir(expr).expect("condition should rewrite");
        assert_eq!(
            rewritten,
            "$.node_outputs.classify.state == \"$.nodes.keep.output\""
        );
    }

    #[test]
    fn rewrite_yaml_condition_rejects_unterminated_quotes() {
        let err = rewrite_yaml_condition_to_ir("$.nodes.classify.output.state == \"broken")
            .expect_err("unterminated quote should fail");
        assert!(err.contains("unterminated quoted string"));
    }

    #[test]
    fn yaml_llm_call_rejects_unknown_provider_field() {
        let yaml = r#"
id: wf
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        provider: openai
        model: gpt-4.1-mini
    config:
      prompt: hi
"#;

        let err = serde_yaml::from_str::<YamlWorkflow>(yaml)
            .expect_err("unknown llm_call fields should fail parsing");
        let message = err.to_string();
        assert!(message.contains("provider"));
        assert!(message.contains("unknown field"));
    }

    #[test]
    fn yaml_custom_worker_rejects_unknown_language_field() {
        let yaml = r#"
id: wf
entry_node: worker
nodes:
  - id: worker
    node_type:
      custom_worker:
        language: python
        handler: get_rag_data
"#;

        let err = serde_yaml::from_str::<YamlWorkflow>(yaml)
            .expect_err("unknown custom_worker fields should fail parsing");
        let message = err.to_string();
        assert!(message.contains("language"));
        assert!(message.contains("unknown field"));
    }

    #[tokio::test]
    async fn custom_worker_handler_file_requires_executor() {
        let yaml = r#"
id: wf
entry_node: lookup
nodes:
  - id: lookup
    node_type:
      custom_worker:
        handler: get_rag_data
        handler_file: handlers.py
"#;

        let workflow: YamlWorkflow = serde_yaml::from_str(yaml).expect("yaml should parse");
        let err = run_workflow_yaml_with_custom_worker_and_events_and_options(
            &workflow,
            &json!({"email_text": "hello"}),
            &MockExecutor,
            None,
            None,
            &YamlWorkflowRunOptions::default(),
        )
        .await
        .expect_err("handler_file without executor should fail");

        let rendered = err.to_string();
        assert!(rendered.contains("handler_file"));
        assert!(rendered.contains("no custom worker executor configured"));
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

    #[test]
    fn to_typed_output_maps_node_kinds_from_workflow_definition() {
        let workflow_yaml = r#"
id: typed-output-demo
entry_node: classify
nodes:
  - id: classify
    node_type:
      llm_call:
        model: gpt-4.1
  - id: route
    node_type:
      switch:
        branches:
          - condition: '$.nodes.classify.output.state == "ready"'
            target: worker
        default: worker
  - id: worker
    node_type:
      custom_worker:
        handler: do_work
edges:
  - from: classify
    to: route
  - from: route
    to: worker
"#;
        let workflow: YamlWorkflow =
            serde_yaml::from_str(workflow_yaml).expect("workflow should parse");
        let output = YamlWorkflowRunOutput {
            workflow_id: "typed-output-demo".to_string(),
            entry_node: "classify".to_string(),
            email_text: String::new(),
            trace: vec![
                "classify".to_string(),
                "route".to_string(),
                "worker".to_string(),
            ],
            outputs: BTreeMap::from([
                ("classify".to_string(), json!({"state": "ready"})),
                ("route".to_string(), json!("worker")),
                ("worker".to_string(), json!({"ok": true})),
            ]),
            terminal_node: "worker".to_string(),
            terminal_output: Some(json!({"ok": true})),
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
        };

        let typed = output.to_typed_output(&workflow);
        assert_eq!(typed.node_outputs.len(), 3);
        assert_eq!(
            typed
                .node_outputs
                .iter()
                .find(|record| record.node_id == "classify")
                .map(|record| record.node_kind),
            Some(YamlWorkflowNodeKind::LlmCall)
        );
        assert_eq!(
            typed
                .node_outputs
                .iter()
                .find(|record| record.node_id == "route")
                .map(|record| record.node_kind),
            Some(YamlWorkflowNodeKind::Switch)
        );
        assert_eq!(
            typed
                .node_outputs
                .iter()
                .find(|record| record.node_id == "worker")
                .map(|record| record.node_kind),
            Some(YamlWorkflowNodeKind::CustomWorker)
        );
        assert_eq!(
            typed
                .terminal_output
                .as_ref()
                .map(|record| record.node_kind),
            Some(YamlWorkflowNodeKind::CustomWorker)
        );
    }

    #[test]
    fn to_typed_output_marks_unknown_node_ids() {
        let workflow_yaml = r#"
id: typed-output-unknown
entry_node: start
nodes:
  - id: start
    node_type:
      llm_call:
        model: gpt-4.1
"#;
        let workflow: YamlWorkflow =
            serde_yaml::from_str(workflow_yaml).expect("workflow should parse");
        let output = YamlWorkflowRunOutput {
            workflow_id: "typed-output-unknown".to_string(),
            entry_node: "start".to_string(),
            email_text: String::new(),
            trace: vec!["start".to_string(), "not-in-graph".to_string()],
            outputs: BTreeMap::from([("not-in-graph".to_string(), json!({"v": 1}))]),
            terminal_node: "not-in-graph".to_string(),
            terminal_output: Some(json!({"v": 1})),
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
        };

        let typed = output.to_typed_output(&workflow);
        assert_eq!(typed.node_outputs.len(), 1);
        assert_eq!(
            typed.node_outputs[0].node_kind,
            YamlWorkflowNodeKind::Unknown
        );
        assert_eq!(
            typed
                .terminal_output
                .as_ref()
                .map(|record| record.node_kind),
            Some(YamlWorkflowNodeKind::Unknown)
        );
    }

    #[test]
    fn to_typed_event_maps_known_event_type() {
        let event = YamlWorkflowEvent {
            event_type: "node_completed".to_string(),
            node_id: Some("classify".to_string()),
            step_id: Some("classify".to_string()),
            node_kind: Some("llm_call".to_string()),
            streamable: Some(false),
            message: Some("done".to_string()),
            delta: None,
            token_kind: None,
            is_terminal_node_token: Some(true),
            elapsed_ms: Some(11),
            metadata: Some(json!({"k": "v"})),
        };

        let typed = event.to_typed_event();
        assert_eq!(typed.event_type, YamlWorkflowEventType::NodeCompleted);
        assert_eq!(typed.raw_event_type, "node_completed");
        assert_eq!(typed.node_id.as_deref(), Some("classify"));
        assert_eq!(
            typed.metadata.as_ref().and_then(|value| value.get("k")),
            Some(&json!("v"))
        );
    }

    #[test]
    fn to_typed_event_marks_unknown_event_type() {
        let event = YamlWorkflowEvent {
            event_type: "custom_event_not_recognized".to_string(),
            node_id: None,
            step_id: None,
            node_kind: None,
            streamable: None,
            message: None,
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: None,
            metadata: None,
        };

        let typed = event.to_typed_event();
        assert_eq!(typed.event_type, YamlWorkflowEventType::Unknown);
        assert_eq!(typed.raw_event_type, "custom_event_not_recognized");
    }
}
