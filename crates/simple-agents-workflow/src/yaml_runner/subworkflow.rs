use std::path::Path;

use serde_json::{json, Value};
use simple_agents_core::SimpleAgentsClient;

use super::api::workflow_execution;
use super::contracts::YamlWorkflowCustomWorkerExecutor;
use super::types::{
    YamlWorkflowExecutionFlags, YamlWorkflowExecutorBinding, YamlWorkflowRunOptions,
    YamlWorkflowSource, YamlWorkflowTraceContextInput,
};

use crate::observability::tracing::TraceContext;

pub(crate) async fn execute_subworkflow_tool_call(
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

    use super::types::YamlWorkflowExecutionRequest;
    let request = YamlWorkflowExecutionRequest {
        source: YamlWorkflowSource::File(Path::new(workflow_path)),
        workflow_input: &Value::Object(subworkflow_input),
        executor: YamlWorkflowExecutorBinding::Client(client),
        custom_worker,
        options: &subworkflow_options,
        flags: YamlWorkflowExecutionFlags::default(),
    };

    let output = workflow_execution::run(request)
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

pub(crate) fn build_subworkflow_options(
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
