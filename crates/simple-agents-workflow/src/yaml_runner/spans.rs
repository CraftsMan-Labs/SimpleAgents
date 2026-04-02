use super::*;

pub(super) fn start_workflow_span(
    tracer: &dyn crate::observability::tracing::WorkflowTracer,
    telemetry_context: &ResolvedTelemetryContext,
    parent_trace_context: Option<&TraceContext>,
    options: &YamlWorkflowRunOptions,
) -> (
    Option<TraceContext>,
    Option<Box<dyn crate::observability::tracing::WorkflowSpan>>,
) {
    if !telemetry_context.sampled {
        return (None, None);
    }

    let (span_context, mut span) =
        tracer.start_span("workflow.run", SpanKind::Workflow, parent_trace_context);
    apply_trace_identity_attributes(span.as_mut(), telemetry_context.trace_id.as_deref());
    apply_trace_tenant_attributes(span.as_mut(), options);
    (Some(span_context), Some(span))
}

pub(super) fn start_node_span(
    tracer: &dyn crate::observability::tracing::WorkflowTracer,
    telemetry_context: &ResolvedTelemetryContext,
    workflow_span_context: Option<&TraceContext>,
    options: &YamlWorkflowRunOptions,
    node_id: &str,
    node_kind: &str,
) -> (
    Option<TraceContext>,
    Option<Box<dyn crate::observability::tracing::WorkflowSpan>>,
) {
    if !telemetry_context.sampled {
        return (None, None);
    }

    let (span_context, mut span) = tracer.start_span(
        "workflow.node.execute",
        SpanKind::Node,
        workflow_span_context,
    );
    apply_trace_identity_attributes(span.as_mut(), telemetry_context.trace_id.as_deref());
    apply_trace_tenant_attributes(span.as_mut(), options);
    span.set_attribute("node_id", node_id);
    span.set_attribute("node_kind", node_kind);
    if node_kind == "llm_call" {
        span.set_attribute("langfuse.observation.type", "generation");
    }

    (Some(span_context), Some(span))
}

pub(super) fn finish_node_span(
    mut node_span: Option<Box<dyn crate::observability::tracing::WorkflowSpan>>,
    node_model_name: Option<&str>,
    node_usage: Option<&YamlLlmTokenUsage>,
    elapsed_ms: u128,
) {
    if let Some(mut span) = node_span.take() {
        if let Some(model_name) = node_model_name {
            span.set_attribute("langfuse.observation.model.name", model_name);
            span.set_attribute("gen_ai.request.model", model_name);
        }
        if let Some(usage) = node_usage {
            apply_langfuse_observation_usage_attributes(span.as_mut(), usage);
        }
        span.set_attribute("elapsed_ms", elapsed_ms.to_string().as_str());
        span.add_event("node_completed");
        span.end();
    }
}

pub(super) fn finish_workflow_span(
    mut workflow_span: Option<Box<dyn crate::observability::tracing::WorkflowSpan>>,
    workflow: &YamlWorkflow,
    telemetry_context: &ResolvedTelemetryContext,
    workflow_input: &Value,
    output: &YamlWorkflowRunOutput,
    options: &YamlWorkflowRunOptions,
) {
    if let Some(mut span) = workflow_span.take() {
        span.set_attribute("workflow_id", workflow.id.as_str());
        apply_trace_identity_attributes(span.as_mut(), telemetry_context.trace_id.as_deref());
        apply_langfuse_trace_input_output_attributes(
            span.as_mut(),
            workflow_input,
            output,
            options.telemetry.payload_mode,
        );
        apply_langfuse_nerdstats_attributes(span.as_mut(), output, options.telemetry.nerdstats);
        span.end();
        flush_workflow_tracer();
    }
}
