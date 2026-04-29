use serde_json::{json, Value};

use crate::observability::tracing::{TraceContext, WorkflowSpan};

use super::context::json_type_name;
use super::contracts::YamlLlmExecutionRequest;
use super::types::{
    YamlLlmTokenUsage, YamlToolTraceMode, YamlWorkflowPayloadMode, YamlWorkflowRunOptions,
    YamlWorkflowRunOutput, YamlWorkflowTraceTenantContext,
};

const MAX_SPAN_PAYLOAD_CHARS: usize = 32 * 1024;

static TRACE_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) fn should_sample_trace(trace_id: &str, sample_rate: f32) -> bool {
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

pub(crate) fn trace_id_from_traceparent(traceparent: &str) -> Option<String> {
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
pub(crate) enum TraceIdSource {
    Disabled,
    ExplicitTraceId,
    ParentTraceId,
    Traceparent,
    ParentTraceparent,
    Generated,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedTelemetryContext {
    pub(crate) trace_id: Option<String>,
    pub(crate) sampled: bool,
    pub(crate) trace_id_source: TraceIdSource,
}

pub(crate) fn resolve_run_trace_id_with_source(
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

pub(crate) fn resolve_telemetry_context(
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

pub(crate) fn generate_trace_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = u128::from(TRACE_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    format!("{:032x}", now_nanos ^ sequence)
}

pub(crate) fn workflow_metadata_with_trace(
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

pub(crate) fn apply_trace_identity_attributes(span: &mut dyn WorkflowSpan, trace_id: Option<&str>) {
    if let Some(value) = trace_id {
        span.set_attribute("trace_id", value);
    }
}

pub(crate) fn apply_trace_tenant_attributes_from_tenant(
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

pub(crate) fn apply_trace_tenant_attributes(
    span: &mut dyn WorkflowSpan,
    options: &YamlWorkflowRunOptions,
) {
    apply_trace_tenant_attributes_from_tenant(span, &options.trace.tenant);
}

pub(crate) fn apply_langfuse_nerdstats_attributes(
    span: &mut dyn WorkflowSpan,
    output: &YamlWorkflowRunOutput,
    enabled: bool,
) {
    if !enabled {
        return;
    }

    let nerdstats = super::nerdstats::workflow_nerdstats(output);
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

pub(crate) fn apply_langfuse_trace_input_output_attributes(
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

pub(crate) fn apply_langfuse_observation_usage_attributes(
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

pub(crate) fn payload_for_span(mode: YamlWorkflowPayloadMode, payload: &Value) -> String {
    match mode {
        YamlWorkflowPayloadMode::FullPayload => truncate_span_payload(payload.to_string()),
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

fn truncate_span_payload(payload: String) -> String {
    if payload.len() <= MAX_SPAN_PAYLOAD_CHARS {
        return payload;
    }

    let total_len = payload.len();
    let mut truncated = payload;
    let mut truncate_at = MAX_SPAN_PAYLOAD_CHARS;
    while !truncated.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }
    truncated.truncate(truncate_at);
    format!("{truncated}...[truncated, {total_len} bytes total]")
}

pub(crate) fn payload_for_tool_trace(mode: YamlToolTraceMode, payload: &Value) -> Value {
    match mode {
        YamlToolTraceMode::Full => payload.clone(),
        YamlToolTraceMode::Redacted => json!({
            "redacted": true,
            "value_type": json_type_name(payload),
        }),
        YamlToolTraceMode::Off => Value::Null,
    }
}

pub(crate) fn trace_context_from_options(options: &YamlWorkflowRunOptions) -> Option<TraceContext> {
    options.trace.context.as_ref().map(|input| TraceContext {
        trace_id: input.trace_id.clone(),
        span_id: input.span_id.clone(),
        parent_span_id: input.parent_span_id.clone(),
        traceparent: input.traceparent.clone(),
        tracestate: input.tracestate.clone(),
        baggage: input.baggage.clone(),
    })
}

pub(crate) fn merged_trace_context_for_worker(
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

pub(crate) fn custom_worker_context_with_trace(
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

/// Whether split thinking/output stream events are enabled for this LLM request.
pub(crate) fn split_stream_deltas_enabled(request: &YamlLlmExecutionRequest) -> bool {
    request.split_stream_deltas
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use super::super::types::{
        YamlStepTiming, YamlWorkflowTraceContextInput, YamlWorkflowTraceOptions,
    };
    use crate::observability::tracing::WorkflowSpan;

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

    #[test]
    fn subworkflow_options_inherit_parent_telemetry_configuration() {
        let parent_options = YamlWorkflowRunOptions {
            telemetry: super::super::types::YamlWorkflowTelemetryConfig {
                enabled: true,
                nerdstats: false,
                sample_rate: 0.5,
                ..Default::default()
            },
            trace: YamlWorkflowTraceOptions {
                context: None,
                tenant: YamlWorkflowTraceTenantContext {
                    workspace_id: Some("ws-1".to_string()),
                    user_id: Some("user-1".to_string()),
                    conversation_id: Some("conv-1".to_string()),
                    request_id: Some("req-1".to_string()),
                    run_id: Some("run-1".to_string()),
                },
            },
            model: Some("gpt-4.1".to_string()),
        };

        let trace_context = TraceContext {
            trace_id: Some("abc123".to_string()),
            span_id: Some("span-1".to_string()),
            parent_span_id: None,
            traceparent: Some("00-abc123-span1-01".to_string()),
            tracestate: None,
            baggage: BTreeMap::new(),
        };

        let sub_options = super::super::subworkflow::build_subworkflow_options(
            &parent_options,
            Some(&trace_context),
            Some("abc123"),
        );

        assert!(sub_options.telemetry.enabled);
        assert!(!sub_options.telemetry.nerdstats);
        assert!((sub_options.telemetry.sample_rate - 0.5).abs() < f32::EPSILON);
        assert_eq!(sub_options.model, Some("gpt-4.1".to_string()));
        assert_eq!(
            sub_options.trace.tenant.workspace_id,
            Some("ws-1".to_string())
        );

        let sub_trace = sub_options
            .trace
            .context
            .as_ref()
            .expect("subworkflow should have trace context");
        assert_eq!(sub_trace.trace_id, Some("abc123".to_string()));
        assert_eq!(sub_trace.span_id, Some("span-1".to_string()));
    }

    #[test]
    fn trace_tenant_attributes_include_langfuse_aliases() {
        let mut span = CapturingSpan::default();
        let tenant = YamlWorkflowTraceTenantContext {
            workspace_id: Some("ws-42".to_string()),
            user_id: Some("user-7".to_string()),
            conversation_id: Some("conv-99".to_string()),
            request_id: Some("req-3".to_string()),
            run_id: Some("run-5".to_string()),
        };

        apply_trace_tenant_attributes_from_tenant(&mut span, &tenant);

        assert_eq!(
            span.attributes
                .get("tenant.workspace_id")
                .map(String::as_str),
            Some("ws-42"),
        );
        assert_eq!(
            span.attributes.get("tenant.user_id").map(String::as_str),
            Some("user-7"),
        );
        assert_eq!(
            span.attributes.get("user.id").map(String::as_str),
            Some("user-7"),
        );
        assert_eq!(
            span.attributes.get("langfuse.user.id").map(String::as_str),
            Some("user-7"),
        );
        assert_eq!(
            span.attributes
                .get("tenant.conversation_id")
                .map(String::as_str),
            Some("conv-99"),
        );
        assert_eq!(
            span.attributes.get("session.id").map(String::as_str),
            Some("conv-99"),
        );
        assert_eq!(
            span.attributes
                .get("langfuse.session.id")
                .map(String::as_str),
            Some("conv-99"),
        );
        assert_eq!(
            span.attributes.get("tenant.request_id").map(String::as_str),
            Some("req-3"),
        );
        assert_eq!(
            span.attributes.get("tenant.run_id").map(String::as_str),
            Some("run-5"),
        );
    }

    #[test]
    fn langfuse_nerdstats_attributes_are_written_when_enabled() {
        let mut span = CapturingSpan::default();
        let output = YamlWorkflowRunOutput {
            workflow_id: "nerdstats-attrs".to_string(),
            entry_node: "start".to_string(),
            trace: vec!["start".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "start".to_string(),
            terminal_output: Some(serde_json::json!({"ok": true})),
            step_timings: vec![YamlStepTiming {
                node_id: "start".to_string(),
                node_kind: "llm_call".to_string(),
                model_name: Some("gpt-4.1".to_string()),
                elapsed_ms: 50,
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                reasoning_tokens: None,
                tokens_per_second: None,
            }],
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 50,
            ttft_ms: Some(12),
            total_input_tokens: 10,
            total_output_tokens: 5,
            total_tokens: 15,
            total_reasoning_tokens: None,
            tokens_per_second: 100.0,
            trace_id: None,
            metadata: None,
        };

        apply_langfuse_nerdstats_attributes(&mut span, &output, true);

        assert!(span
            .attributes
            .contains_key("langfuse.trace.metadata.nerdstats"));
        assert_eq!(
            span.attributes
                .get("langfuse.trace.metadata.nerdstats.workflow_id")
                .map(String::as_str),
            Some("nerdstats-attrs"),
        );
        assert_eq!(
            span.attributes
                .get("langfuse.trace.metadata.nerdstats.ttft_ms")
                .map(String::as_str),
            Some("12"),
        );
        assert_eq!(
            span.attributes
                .get("langfuse.trace.metadata.nerdstats.token_metrics_available")
                .map(String::as_str),
            Some("true"),
        );
    }

    #[test]
    fn langfuse_trace_input_output_and_usage_are_written() {
        let mut span = CapturingSpan::default();
        let input = serde_json::json!({"email_text":"hi"});
        let output = YamlWorkflowRunOutput {
            workflow_id: "input-output-test".to_string(),
            entry_node: "start".to_string(),
            trace: vec!["start".to_string()],
            outputs: BTreeMap::new(),
            terminal_node: "start".to_string(),
            terminal_output: Some(serde_json::json!({"ok": true})),
            step_timings: Vec::new(),
            llm_node_metrics: BTreeMap::new(),
            llm_node_models: BTreeMap::new(),
            total_elapsed_ms: 10,
            ttft_ms: None,
            total_input_tokens: 100,
            total_output_tokens: 50,
            total_tokens: 150,
            total_reasoning_tokens: Some(20),
            tokens_per_second: 5000.0,
            trace_id: None,
            metadata: None,
        };

        apply_langfuse_trace_input_output_attributes(
            &mut span,
            &input,
            &output,
            YamlWorkflowPayloadMode::FullPayload,
        );

        assert!(span
            .attributes
            .get("langfuse.trace.input")
            .is_some_and(|v| v.contains("hi")));
        assert!(span
            .attributes
            .get("langfuse.trace.output")
            .is_some_and(|v| v.contains("ok")));
        assert!(span
            .attributes
            .get("langfuse.trace.metadata.usage_details")
            .is_some_and(|v| v.contains("\"input\":100")));
    }

    #[test]
    fn langfuse_observation_usage_attributes_are_written() {
        let mut span = CapturingSpan::default();
        let usage = YamlLlmTokenUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            reasoning_tokens: Some(3),
        };

        apply_langfuse_observation_usage_attributes(&mut span, &usage);

        assert!(span
            .attributes
            .contains_key("langfuse.observation.usage_details"));
        assert_eq!(
            span.attributes
                .get("gen_ai.usage.input_tokens")
                .map(String::as_str),
            Some("10"),
        );
        assert_eq!(
            span.attributes
                .get("gen_ai.usage.output_tokens")
                .map(String::as_str),
            Some("5"),
        );
        assert_eq!(
            span.attributes
                .get("gen_ai.usage.reasoning_tokens")
                .map(String::as_str),
            Some("3"),
        );
    }

    fn stub_llm_request(split_stream_deltas: bool) -> YamlLlmExecutionRequest {
        YamlLlmExecutionRequest {
            node_id: "test".to_string(),
            is_terminal_node: false,
            stream_json_as_text: false,
            model: "gpt-4.1".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            messages: None,
            append_prompt_as_user: true,
            prompt: "test".to_string(),
            prompt_template: "test".to_string(),
            prompt_bindings: Vec::new(),
            schema: serde_json::json!({"type":"object"}),
            stream: false,
            heal: false,
            send_schema: false,
            tools: Vec::new(),
            tool_choice: None,
            max_tool_roundtrips: 1,
            tool_calls_global_key: None,
            tool_trace_mode: YamlToolTraceMode::Off,
            execution_context: serde_json::json!({}),
            trace_id: None,
            trace_context: None,
            tenant_context: YamlWorkflowTraceTenantContext::default(),
            trace_sampled: false,
            split_stream_deltas,
        }
    }

    #[test]
    fn split_stream_deltas_enabled_follows_request_flag() {
        assert!(!split_stream_deltas_enabled(&stub_llm_request(false)));
        assert!(split_stream_deltas_enabled(&stub_llm_request(true)));
    }
}
