use super::*;

pub(super) fn ensure_event_sink_active(
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<(), YamlWorkflowRunError> {
    if event_sink_is_cancelled(event_sink) {
        return Err(YamlWorkflowRunError::EventSinkCancelled {
            message: workflow_event_sink_cancelled_message().to_string(),
        });
    }
    Ok(())
}

pub(super) fn emit_workflow_started(
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    workflow_id: &str,
) {
    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "workflow_started".to_string(),
            node_id: None,
            step_id: None,
            node_kind: None,
            streamable: None,
            message: Some(format!("workflow_id={workflow_id}")),
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(0),
            metadata: None,
        });
    }
}

pub(super) fn emit_node_started(
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    node_id: &str,
    node_kind: &str,
    node_streamable: Option<bool>,
    elapsed_ms: u128,
) {
    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "node_started".to_string(),
            node_id: Some(node_id.to_string()),
            step_id: Some(node_id.to_string()),
            node_kind: Some(node_kind.to_string()),
            streamable: node_streamable,
            message: if node_streamable == Some(false) {
                Some("Node is not streamable; status events only".to_string())
            } else {
                None
            },
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(elapsed_ms),
            metadata: None,
        });
    }
}

pub(super) fn emit_llm_input_resolved(
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    node_id: &str,
    elapsed_ms: u128,
    request: &YamlLlmExecutionRequest,
) {
    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "node_llm_input_resolved".to_string(),
            node_id: Some(node_id.to_string()),
            step_id: Some(node_id.to_string()),
            node_kind: Some("llm_call".to_string()),
            streamable: Some(request.stream),
            message: Some("resolved llm input for telemetry".to_string()),
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(elapsed_ms),
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
                "tools_count": request.tools.len(),
                "max_tool_roundtrips": request.max_tool_roundtrips,
            })),
        });
    }
}

pub(super) fn emit_node_completed(
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    node_id: &str,
    node_kind: &str,
    node_streamable: Option<bool>,
    elapsed_ms: u128,
) {
    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "node_completed".to_string(),
            node_id: Some(node_id.to_string()),
            step_id: Some(node_id.to_string()),
            node_kind: Some(node_kind.to_string()),
            streamable: node_streamable,
            message: None,
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(elapsed_ms),
            metadata: None,
        });
    }
}

pub(super) fn emit_workflow_completed(
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    terminal_node: &str,
    elapsed_ms: u128,
    metadata: Option<Value>,
) {
    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "workflow_completed".to_string(),
            node_id: None,
            step_id: None,
            node_kind: None,
            streamable: None,
            message: Some(format!("terminal_node={terminal_node}")),
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(elapsed_ms),
            metadata,
        });
    }
}
