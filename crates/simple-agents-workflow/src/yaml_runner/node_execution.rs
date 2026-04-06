use super::*;

pub(super) struct NodeExecutionOutcome {
    pub(super) next: Option<String>,
    pub(super) node_usage: Option<YamlLlmTokenUsage>,
    pub(super) node_model_name: Option<String>,
}

pub(super) struct LlmNodeEnv<'a> {
    pub(super) node: &'a YamlNode,
    pub(super) llm: &'a YamlLlmCall,
    pub(super) is_terminal_node: bool,
    pub(super) workflow_input: &'a Value,
    pub(super) edge_map: &'a HashMap<&'a str, &'a str>,
    pub(super) executor: &'a dyn YamlWorkflowLlmExecutor,
    pub(super) event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    pub(super) options: &'a YamlWorkflowRunOptions,
    pub(super) execution_flags: super::YamlWorkflowExecutionFlags,
    pub(super) telemetry_context: &'a ResolvedTelemetryContext,
    pub(super) node_span_context: Option<TraceContext>,
    pub(super) node_span: Option<&'a mut Box<dyn crate::observability::tracing::WorkflowSpan>>,
    pub(super) workflow_elapsed_before_node_ms: u128,
    pub(super) started: &'a Instant,
}

pub(super) struct LlmNodeState<'a> {
    pub(super) outputs: &'a mut BTreeMap<String, Value>,
    pub(super) globals: &'a mut serde_json::Map<String, Value>,
    pub(super) token_totals: &'a mut YamlTokenTotals,
    pub(super) workflow_ttft_ms: &'a mut Option<u128>,
    pub(super) llm_node_models: &'a mut BTreeMap<String, String>,
}

pub(super) struct CustomWorkerEnv<'a> {
    pub(super) node: &'a YamlNode,
    pub(super) custom: &'a YamlCustomWorker,
    pub(super) workflow_input: &'a Value,
    pub(super) edge_map: &'a HashMap<&'a str, &'a str>,
    pub(super) custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    pub(super) options: &'a YamlWorkflowRunOptions,
    pub(super) telemetry_context: &'a ResolvedTelemetryContext,
    pub(super) workflow_span_context: Option<&'a TraceContext>,
    pub(super) tracer: &'a dyn crate::observability::tracing::WorkflowTracer,
    pub(super) node_span: Option<&'a mut Box<dyn crate::observability::tracing::WorkflowSpan>>,
    pub(super) event_sink: Option<&'a dyn YamlWorkflowEventSink>,
}

pub(super) struct CustomWorkerState<'a> {
    pub(super) outputs: &'a mut BTreeMap<String, Value>,
    pub(super) globals: &'a mut serde_json::Map<String, Value>,
}

pub(super) async fn execute_llm_node(
    env: LlmNodeEnv<'_>,
    state: LlmNodeState<'_>,
) -> Result<NodeExecutionOutcome, YamlWorkflowRunError> {
    let LlmNodeEnv {
        node,
        llm,
        is_terminal_node,
        workflow_input,
        edge_map,
        executor,
        event_sink,
        options,
        execution_flags,
        telemetry_context,
        node_span_context,
        node_span,
        workflow_elapsed_before_node_ms,
        started,
    } = env;
    let LlmNodeState {
        outputs,
        globals,
        token_totals,
        workflow_ttft_ms,
        llm_node_models,
    } = state;

    let mut node_span = node_span;
    let prompt_template = node
        .config
        .as_ref()
        .and_then(|cfg| cfg.prompt.as_deref())
        .unwrap_or_default();
    let context = build_execution_context(workflow_input, outputs, globals);
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

    let yaml_heal = llm.heal.unwrap_or(false);
    let yaml_stream = llm.stream.unwrap_or(false);
    let heal = yaml_heal || execution_flags.healing;
    let stream = yaml_stream && execution_flags.node_llm_streaming;

    let request = YamlLlmExecutionRequest {
        node_id: node.id.clone(),
        is_terminal_node,
        stream_json_as_text: llm.stream_json_as_text.unwrap_or(false),
        model: resolve_requested_model(options.model.as_deref(), &llm.model),
        max_tokens: llm.max_tokens,
        temperature: llm.temperature,
        top_p: llm.top_p,
        messages,
        append_prompt_as_user: llm.append_prompt_as_user.unwrap_or(true),
        prompt,
        prompt_template: prompt_template.to_string(),
        prompt_bindings,
        schema,
        stream,
        heal,
        tools: normalize_llm_tools(llm).map_err(|message| YamlWorkflowRunError::Llm {
            node_id: node.id.clone(),
            message,
        })?,
        tool_choice: normalize_tool_choice(llm.tool_choice.clone()).map_err(|message| {
            YamlWorkflowRunError::Llm {
                node_id: node.id.clone(),
                message,
            }
        })?,
        max_tool_roundtrips: llm.max_tool_roundtrips.unwrap_or(1),
        tool_calls_global_key: llm.tool_calls_global_key.clone(),
        tool_trace_mode: options.telemetry.tool_trace_mode,
        execution_context: context.clone(),
        trace_id: telemetry_context.trace_id.clone(),
        trace_context: node_span_context,
        tenant_context: options.trace.tenant.clone(),
        trace_sampled: telemetry_context.sampled,
        split_stream_deltas: execution_flags.split_stream_deltas,
    };

    if let Some(span) = node_span.as_mut() {
        let node_input = payload_for_span(options.telemetry.payload_mode, &context);
        span.set_attribute("node_input", node_input.as_str());
        span.set_attribute("langfuse.observation.input", node_input.as_str());
    }

    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "resolved_llm_input".to_string(),
            node_id: Some(node.id.clone()),
            step_id: Some(node.id.clone()),
            node_kind: Some("llm_call".to_string()),
            streamable: None,
            message: None,
            delta: None,
            token_kind: None,
            is_terminal_node_token: None,
            elapsed_ms: Some(started.elapsed().as_millis()),
            metadata: Some(json!({
                "bindings": request.prompt_bindings,
                "model": request.model,
                "prompt": request.prompt,
            })),
        });
    }

    llm_node_models.insert(node.id.clone(), request.model.clone());
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
        *workflow_ttft_ms = llm_result
            .ttft_ms
            .map(|node_ttft_ms| workflow_elapsed_before_node_ms + node_ttft_ms);
    }
    let node_usage = llm_result.usage;

    let payload = llm_result.payload;
    let tool_calls = llm_result.tool_calls;

    let mut node_output = json!({ "output": payload });
    if !tool_calls.is_empty() {
        if let Some(output_obj) = node_output.as_object_mut() {
            output_obj.insert("tool_calls".to_string(), json!(tool_calls));
        }
    }
    outputs.insert(node.id.clone(), node_output);
    apply_node_output_span_attributes(
        node_span,
        options.telemetry.payload_mode,
        outputs.get(node.id.as_str()),
    );
    apply_set_globals(node, outputs, workflow_input, globals);
    apply_update_globals(node, outputs, workflow_input, globals);
    if let Some(global_key) = llm.tool_calls_global_key.as_ref() {
        if let Some(node_tool_calls) = outputs
            .get(node.id.as_str())
            .and_then(|value| value.get("tool_calls"))
            .cloned()
        {
            globals.insert(global_key.clone(), node_tool_calls);
        }
    }

    Ok(NodeExecutionOutcome {
        next: edge_map
            .get(node.id.as_str())
            .map(|value| value.to_string()),
        node_usage,
        node_model_name: llm_node_models.get(node.id.as_str()).cloned(),
    })
}

pub(super) async fn execute_custom_worker_node(
    env: CustomWorkerEnv<'_>,
    state: CustomWorkerState<'_>,
) -> Result<Option<String>, YamlWorkflowRunError> {
    let CustomWorkerEnv {
        node,
        custom,
        workflow_input,
        edge_map,
        custom_worker,
        options,
        telemetry_context,
        workflow_span_context,
        tracer,
        node_span,
        event_sink,
    } = env;
    let CustomWorkerState { outputs, globals } = state;

    let mut node_span = node_span;
    let payload = node
        .config
        .as_ref()
        .and_then(|cfg| cfg.payload.as_ref())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let context = build_execution_context(workflow_input, outputs, globals);

    if let Some(span) = node_span.as_mut() {
        span.set_attribute("handler_name", custom.handler.as_str());
        let node_input = payload_for_span(options.telemetry.payload_mode, &payload);
        span.set_attribute("node_input", node_input.as_str());
        span.set_attribute("langfuse.observation.input", node_input.as_str());
    }

    let mut handler_span_context: Option<TraceContext> = None;
    let mut handler_span = if telemetry_context.sampled {
        let (span_context, mut span) =
            tracer.start_span("handler.invoke", SpanKind::Node, workflow_span_context);
        handler_span_context = Some(span_context);
        apply_trace_identity_attributes(span.as_mut(), telemetry_context.trace_id.as_deref());
        span.set_attribute("handler_name", custom.handler.as_str());
        apply_trace_tenant_attributes(span.as_mut(), options);
        Some(span)
    } else {
        None
    };

    let worker_trace_context = merged_trace_context_for_worker(
        handler_span_context.as_ref(),
        telemetry_context.trace_id.as_deref(),
        options,
    );
    let worker_context =
        custom_worker_context_with_trace(&context, &worker_trace_context, &options.trace.tenant);

    if event_sink_is_cancelled(event_sink) {
        return Err(YamlWorkflowRunError::EventSinkCancelled {
            message: workflow_event_sink_cancelled_message().to_string(),
        });
    }

    let worker_output_result = if let Some(custom_worker_executor) = custom_worker {
        custom_worker_executor
            .execute(
                custom.handler.as_str(),
                custom.handler_file.as_deref(),
                &payload,
                &worker_context,
            )
            .await
            .map_err(|message| YamlWorkflowRunError::CustomWorker {
                node_id: node.id.clone(),
                message,
            })
    } else {
        Err(YamlWorkflowRunError::CustomWorker {
            node_id: node.id.clone(),
            message: format!(
                "custom worker '{}' requires a configured custom worker executor",
                custom.handler
            ),
        })
    };

    if let Some(span) = handler_span.take() {
        span.end();
    }

    let worker_output = worker_output_result?;
    outputs.insert(node.id.clone(), json!({ "output": worker_output }));
    apply_node_output_span_attributes(
        node_span,
        options.telemetry.payload_mode,
        outputs.get(node.id.as_str()),
    );
    apply_set_globals(node, outputs, workflow_input, globals);
    apply_update_globals(node, outputs, workflow_input, globals);

    Ok(edge_map
        .get(node.id.as_str())
        .map(|value| value.to_string()))
}

fn apply_node_output_span_attributes(
    node_span: Option<&mut Box<dyn crate::observability::tracing::WorkflowSpan>>,
    payload_mode: YamlWorkflowPayloadMode,
    output_payload: Option<&Value>,
) {
    if let (Some(span), Some(payload)) = (node_span, output_payload) {
        let node_output = payload_for_span(payload_mode, payload);
        span.set_attribute("node_output", node_output.as_str());
        span.set_attribute("langfuse.observation.output", node_output.as_str());
    }
}

fn build_execution_context(
    workflow_input: &Value,
    outputs: &BTreeMap<String, Value>,
    globals: &serde_json::Map<String, Value>,
) -> Value {
    json!({
        "input": workflow_input,
        "nodes": outputs,
        "globals": Value::Object(globals.clone())
    })
}
