use super::node_execution::{
    execute_custom_worker_node, execute_llm_node, CustomWorkerEnv, CustomWorkerState, LlmNodeEnv,
    LlmNodeState,
};
use super::spans::{finish_node_span, finish_workflow_span, start_node_span, start_workflow_span};
use super::*;

pub(super) struct YamlWorkflowRunDispatchRequest<'a> {
    pub workflow_input: &'a Value,
    pub executor: &'a dyn YamlWorkflowLlmExecutor,
    pub custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    pub event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    pub options: &'a YamlWorkflowRunOptions,
    pub execution_flags: YamlWorkflowExecutionFlags,
    pub resume: Option<&'a YamlWorkflowRunOutput>,
    pub human_response: Option<&'a Value>,
}

pub(super) async fn run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
    workflow: &YamlWorkflow,
    request: YamlWorkflowRunDispatchRequest<'_>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let YamlWorkflowRunDispatchRequest {
        workflow_input,
        executor,
        custom_worker,
        event_sink,
        options,
        execution_flags,
        resume,
        human_response,
    } = request;

    if !workflow_input.is_object() {
        return Err(YamlWorkflowRunError::InvalidInput {
            message: "workflow input must be a JSON object".to_string(),
        });
    }

    validate_sample_rate(options.telemetry.sample_rate)?;

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

    validate_custom_worker_handler_files(workflow, custom_worker)?;

    let parent_trace_context = trace_context_from_options(options);
    let telemetry_context = resolve_telemetry_context(options, parent_trace_context.as_ref());

    let tracer = workflow_tracer();
    let (workflow_span_context, mut workflow_span) = start_workflow_span(
        tracer,
        &telemetry_context,
        parent_trace_context.as_ref(),
        options,
    );

    let run_result = async {
        let mut run_context =
            prepare_run_context(workflow, workflow_input, resume, human_response)?;

        if let Some(sink) = event_sink {
            sink.emit(&YamlWorkflowEvent {
                event_type: "workflow_started".to_string(),
                node_id: None,
                step_id: None,
                node_kind: None,
                streamable: None,
                message: Some(format!("workflow '{}' started", workflow.id)),
                delta: None,
                snapshot: None,
                token_kind: None,
                is_terminal_node_token: None,
                elapsed_ms: None,
                metadata: None,
            });
        }
        if event_sink_is_cancelled(event_sink) {
            return Err(YamlWorkflowRunError::EventSinkCancelled {
                message: workflow_event_sink_cancelled_message().to_string(),
            });
        }
        if let (Some(resume_output), Some(response)) = (resume, human_response) {
            if let (Some(sink), Some(request)) = (event_sink, resume_output.human_request.as_ref())
            {
                sink.emit(&YamlWorkflowEvent {
                    event_type: "human_input_received".to_string(),
                    node_id: Some(request.node_id.clone()),
                    step_id: Some(request.node_id.clone()),
                    node_kind: Some("human_input".to_string()),
                    streamable: None,
                    message: None,
                    delta: None,
                    snapshot: None,
                    token_kind: None,
                    is_terminal_node_token: None,
                    elapsed_ms: Some(run_context.state.elapsed_before_resume_ms),
                    metadata: Some(response.clone()),
                });
            }
        }

        if !run_context.skip_execution_loop {
            loop {
                if event_sink_is_cancelled(event_sink) {
                    return Err(YamlWorkflowRunError::EventSinkCancelled {
                        message: workflow_event_sink_cancelled_message().to_string(),
                    });
                }

                let step_outcome = execute_single_node_step(
                    workflow_input,
                    executor,
                    custom_worker,
                    event_sink,
                    options,
                    execution_flags,
                    tracer,
                    &telemetry_context,
                    workflow_span_context.as_ref(),
                    &run_context.node_map,
                    &run_context.edge_map,
                    &mut run_context.state,
                )
                .await?;

                match step_outcome {
                    NodeStepOutcome::Next(next) => {
                        run_context.state.current = next;
                    }
                    NodeStepOutcome::Terminated => break,
                    NodeStepOutcome::Paused(request) => {
                        let WorkflowRunState {
                            trace,
                            outputs,
                            globals,
                            step_timings,
                            llm_node_metrics,
                            llm_node_models,
                            token_totals,
                            workflow_ttft_ms,
                            started,
                            elapsed_before_resume_ms,
                            ..
                        } = run_context.state;

                        return finalize_workflow_output(
                            workflow,
                            options,
                            event_sink,
                            &telemetry_context,
                            started,
                            elapsed_before_resume_ms,
                            trace,
                            outputs,
                            globals,
                            YamlWorkflowRunStatus::AwaitingHumanInput,
                            Some(request),
                            step_timings,
                            llm_node_metrics,
                            llm_node_models,
                            token_totals,
                            workflow_ttft_ms,
                        );
                    }
                }
            }
        }

        let WorkflowRunState {
            trace,
            outputs,
            globals,
            step_timings,
            llm_node_metrics,
            llm_node_models,
            token_totals,
            workflow_ttft_ms,
            started,
            elapsed_before_resume_ms,
            ..
        } = run_context.state;

        finalize_workflow_output(
            workflow,
            options,
            event_sink,
            &telemetry_context,
            started,
            elapsed_before_resume_ms,
            trace,
            outputs,
            globals,
            YamlWorkflowRunStatus::Completed,
            None,
            step_timings,
            llm_node_metrics,
            llm_node_models,
            token_totals,
            workflow_ttft_ms,
        )
    }
    .await;

    match run_result {
        Ok(output) => {
            finish_workflow_span(
                workflow_span.take(),
                workflow,
                &telemetry_context,
                workflow_input,
                &output,
                options,
            );
            Ok(output)
        }
        Err(error) => {
            if let Some(mut span) = workflow_span.take() {
                span.set_attribute("workflow_id", workflow.id.as_str());
                apply_trace_identity_attributes(
                    span.as_mut(),
                    telemetry_context.trace_id.as_deref(),
                );
                span.add_event("workflow.error");
                span.set_attribute("error", error.to_string().as_str());
                span.end();
                flush_workflow_tracer();
            }
            Err(error)
        }
    }
}

pub(crate) fn validate_custom_worker_handler_files(
    workflow: &YamlWorkflow,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<(), YamlWorkflowRunError> {
    if custom_worker.is_some() {
        return Ok(());
    }

    for node in &workflow.nodes {
        let Some(worker) = node.node_type.custom_worker.as_ref() else {
            continue;
        };

        if let Some(handler_file) = worker.handler_file.as_deref() {
            return Err(YamlWorkflowRunError::InvalidInput {
                message: format!(
                    "node '{}' sets custom_worker.handler_file='{}', but this runtime has no custom worker executor configured",
                    node.id, handler_file
                ),
            });
        }

        return Err(YamlWorkflowRunError::InvalidInput {
            message: format!(
                "node '{}' declares custom_worker with handler='{}', but no custom worker executor is configured; register a custom worker executor or remove this node",
                node.id, worker.handler
            ),
        });
    }

    Ok(())
}

pub(super) fn build_execution_context(
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

struct PreparedRunContext<'a> {
    node_map: HashMap<&'a str, &'a YamlNode>,
    edge_map: HashMap<&'a str, &'a str>,
    state: WorkflowRunState,
    skip_execution_loop: bool,
}

struct WorkflowRunState {
    current: String,
    trace: Vec<String>,
    outputs: BTreeMap<String, Value>,
    globals: serde_json::Map<String, Value>,
    step_timings: Vec<YamlStepTiming>,
    llm_node_metrics: BTreeMap<String, YamlLlmNodeMetrics>,
    llm_node_models: BTreeMap<String, String>,
    token_totals: YamlTokenTotals,
    workflow_ttft_ms: Option<u128>,
    started: Instant,
    elapsed_before_resume_ms: u128,
}

fn prepare_run_context<'a>(
    workflow: &'a YamlWorkflow,
    workflow_input: &Value,
    resume: Option<&YamlWorkflowRunOutput>,
    human_response: Option<&Value>,
) -> Result<PreparedRunContext<'a>, YamlWorkflowRunError> {
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

    let mut state = WorkflowRunState {
        current: workflow.entry_node.clone(),
        trace: Vec::new(),
        outputs: BTreeMap::new(),
        globals: serde_json::Map::new(),
        step_timings: Vec::new(),
        llm_node_metrics: BTreeMap::new(),
        llm_node_models: BTreeMap::new(),
        token_totals: YamlTokenTotals::default(),
        workflow_ttft_ms: None,
        started: Instant::now(),
        elapsed_before_resume_ms: 0,
    };
    let mut skip_execution_loop = false;

    if let Some(resume_output) = resume {
        if resume_output.workflow_id != workflow.id {
            return Err(YamlWorkflowRunError::InvalidInput {
                message: format!(
                    "resume.workflow_id '{}' does not match requested workflow '{}'",
                    resume_output.workflow_id, workflow.id
                ),
            });
        }
        if resume_output.status != YamlWorkflowRunStatus::AwaitingHumanInput {
            return Err(YamlWorkflowRunError::InvalidInput {
                message: "resume.status must be 'awaiting_human_input'".to_string(),
            });
        }
        let request = resume_output.human_request.as_ref().ok_or_else(|| {
            YamlWorkflowRunError::InvalidInput {
                message: "resume.human_request is required when status=awaiting_human_input"
                    .to_string(),
            }
        })?;
        let response = human_response.ok_or_else(|| YamlWorkflowRunError::InvalidInput {
            message: "human_response is required when resume is provided".to_string(),
        })?;

        state.trace = resume_output.trace.clone();
        state.outputs = resume_output.outputs.clone();
        state.globals = resume_output
            .globals
            .clone()
            .into_iter()
            .collect::<serde_json::Map<String, Value>>();
        state.step_timings = resume_output.step_timings.clone();
        state.llm_node_metrics = resume_output.llm_node_metrics.clone();
        state.llm_node_models = resume_output.llm_node_models.clone();
        state.token_totals = YamlTokenTotals {
            input_tokens: resume_output.total_input_tokens,
            output_tokens: resume_output.total_output_tokens,
            total_tokens: resume_output.total_tokens,
            reasoning_tokens: resume_output.total_reasoning_tokens,
        };
        state.workflow_ttft_ms = resume_output.ttft_ms;
        state.started = Instant::now();
        state.elapsed_before_resume_ms = resume_output.total_elapsed_ms;

        let next_after_human = apply_human_response_for_resume(
            workflow_input,
            &node_map,
            &edge_map,
            &mut state,
            request,
            response,
        )?;
        match next_after_human {
            Some(next) => state.current = next,
            None => skip_execution_loop = true,
        }
    } else if human_response.is_some() {
        return Err(YamlWorkflowRunError::InvalidInput {
            message: "human_response cannot be provided without resume".to_string(),
        });
    }

    Ok(PreparedRunContext {
        node_map,
        edge_map,
        state,
        skip_execution_loop,
    })
}

#[derive(Debug, Clone, PartialEq)]
enum NodeStepOutcome {
    Next(String),
    Terminated,
    Paused(HumanRequest),
}

fn apply_human_response_for_resume(
    workflow_input: &Value,
    node_map: &HashMap<&str, &YamlNode>,
    edge_map: &HashMap<&str, &str>,
    state: &mut WorkflowRunState,
    request: &HumanRequest,
    human_response: &Value,
) -> Result<Option<String>, YamlWorkflowRunError> {
    let node = *node_map.get(request.node_id.as_str()).ok_or_else(|| {
        YamlWorkflowRunError::MissingNode {
            node_id: request.node_id.clone(),
        }
    })?;
    let human =
        node.node_type
            .human_input
            .as_ref()
            .ok_or_else(|| YamlWorkflowRunError::InvalidInput {
                message: format!(
                    "resume.human_request.node_id '{}' is not a human_input node",
                    request.node_id
                ),
            })?;
    validate_human_response(request, human_response)?;

    let pending_human_context = state
        .outputs
        .get(request.node_id.as_str())
        .and_then(|value| value.get("human_input_metadata"))
        .cloned()
        .unwrap_or_else(|| build_human_pending_metadata(request));

    state.outputs.insert(
        request.node_id.clone(),
        build_human_completed_output(
            request,
            pending_human_context,
            human_response.clone(),
            human.input_type == YamlHumanInputType::Form,
        ),
    );

    if !state
        .trace
        .iter()
        .any(|node_id| node_id == request.node_id.as_str())
    {
        state.trace.push(request.node_id.clone());
    }

    apply_set_globals(node, &state.outputs, workflow_input, &mut state.globals);
    apply_update_globals(node, &state.outputs, workflow_input, &mut state.globals);

    Ok(edge_map
        .get(request.node_id.as_str())
        .map(|value| value.to_string()))
}

fn validate_human_response(
    request: &HumanRequest,
    human_response: &Value,
) -> Result<(), YamlWorkflowRunError> {
    match request.input_type {
        YamlHumanInputType::Choice => {
            let selected =
                human_response
                    .as_str()
                    .ok_or_else(|| YamlWorkflowRunError::InvalidInput {
                        message: format!(
                            "human_response for choice node '{}' must be a string",
                            request.node_id
                        ),
                    })?;
            let options =
                request
                    .options
                    .as_ref()
                    .ok_or_else(|| YamlWorkflowRunError::InvalidInput {
                        message: format!(
                            "human_request for choice node '{}' is missing options",
                            request.node_id
                        ),
                    })?;
            if !options.iter().any(|option| option.value == selected) {
                return Err(YamlWorkflowRunError::InvalidInput {
                    message: format!(
                        "human_response '{selected}' is not a valid option for node '{}'",
                        request.node_id
                    ),
                });
            }
        }
        YamlHumanInputType::Text => {
            if !human_response.is_string() {
                return Err(YamlWorkflowRunError::InvalidInput {
                    message: format!(
                        "human_response for text node '{}' must be a string",
                        request.node_id
                    ),
                });
            }
        }
        YamlHumanInputType::Form => {
            if let Some(schema) = request.form_schema.as_ref() {
                super::validation::validate_schema_instance(schema, human_response).map_err(
                    |message| YamlWorkflowRunError::InvalidInput {
                        message: format!(
                            "human_response for form node '{}' failed schema validation: {}",
                            request.node_id, message
                        ),
                    },
                )?;
            }
        }
    }

    Ok(())
}

fn build_human_pending_metadata(request: &HumanRequest) -> Value {
    json!({
        "input_type": request.input_type,
        "prompt_shown": request.prompt,
        "input_to_human": {
            "options": request.options,
            "form_schema": request.form_schema,
            "form_data": request.form_data,
        },
        "human_response": Value::Null,
        "modified": false,
        "modifications_diff": Value::Null,
    })
}

fn build_human_completed_output(
    request: &HumanRequest,
    mut pending_metadata: Value,
    human_response: Value,
    is_form: bool,
) -> Value {
    let before_form = request.form_data.clone().unwrap_or(Value::Null);
    let modified = is_form && before_form != human_response;
    let modifications_diff = if modified {
        json!({
            "before": before_form,
            "after": human_response.clone(),
        })
    } else {
        Value::Null
    };

    if let Some(object) = pending_metadata.as_object_mut() {
        object.insert("human_response".to_string(), human_response.clone());
        object.insert("modified".to_string(), Value::Bool(modified));
        object.insert("modifications_diff".to_string(), modifications_diff);
    }

    json!({
        "output": human_response,
        "human_input_metadata": pending_metadata,
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_single_node_step(
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
    execution_flags: YamlWorkflowExecutionFlags,
    tracer: &dyn crate::observability::tracing::WorkflowTracer,
    telemetry_context: &ResolvedTelemetryContext,
    workflow_span_context: Option<&TraceContext>,
    node_map: &HashMap<&str, &YamlNode>,
    edge_map: &HashMap<&str, &str>,
    state: &mut WorkflowRunState,
) -> Result<NodeStepOutcome, YamlWorkflowRunError> {
    let node =
        *node_map
            .get(state.current.as_str())
            .ok_or_else(|| YamlWorkflowRunError::MissingNode {
                node_id: state.current.clone(),
            })?;

    state.trace.push(node.id.clone());
    let step_started = Instant::now();

    let (node_span_context, mut node_span) = start_node_span(
        tracer,
        telemetry_context,
        workflow_span_context,
        options,
        node.id.as_str(),
        node.kind_name(),
    );

    let node_streamable = node.node_type.llm_call.as_ref().map(|llm| {
        let yaml_heal = llm.heal.unwrap_or(false);
        let yaml_stream = llm.stream.unwrap_or(false);
        let effective_heal = yaml_heal || execution_flags.healing;
        let effective_stream = yaml_stream && execution_flags.node_llm_streaming;
        effective_stream && !effective_heal
    });
    let workflow_elapsed_before_node_ms = state.started.elapsed().as_millis();

    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "node_started".to_string(),
            node_id: Some(node.id.clone()),
            step_id: Some(node.id.clone()),
            node_kind: Some(node.kind_name().to_string()),
            streamable: node_streamable,
            message: None,
            delta: None,
            snapshot: None,
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
    let mut node_model_name: Option<String> = None;
    let is_terminal_node = !edge_map.contains_key(node.id.as_str());
    let node_result: Result<NodeStepOutcome, YamlWorkflowRunError> =
        if let Some(llm) = &node.node_type.llm_call {
            execute_llm_node(
                LlmNodeEnv {
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
                    node_span_context: node_span_context.clone(),
                    node_span: node_span.as_mut(),
                    workflow_elapsed_before_node_ms,
                    started: &state.started,
                },
                LlmNodeState {
                    outputs: &mut state.outputs,
                    globals: &mut state.globals,
                    token_totals: &mut state.token_totals,
                    workflow_ttft_ms: &mut state.workflow_ttft_ms,
                    llm_node_models: &mut state.llm_node_models,
                },
            )
            .await
            .map(|outcome| {
                node_usage = outcome.node_usage;
                node_model_name = outcome.node_model_name;
                match outcome.next {
                    Some(next) => NodeStepOutcome::Next(next),
                    None => NodeStepOutcome::Terminated,
                }
            })
        } else if let Some(switch) = &node.node_type.switch {
            let context = build_execution_context(workflow_input, &state.outputs, &state.globals);
            resolve_switch_target(node.id.as_str(), switch, &context).map(NodeStepOutcome::Next)
        } else if let Some(custom) = &node.node_type.custom_worker {
            execute_custom_worker_node(
                CustomWorkerEnv {
                    node,
                    custom,
                    workflow_input,
                    edge_map,
                    custom_worker,
                    options,
                    telemetry_context,
                    workflow_span_context,
                    tracer,
                    node_span: node_span.as_mut(),
                    event_sink,
                },
                CustomWorkerState {
                    outputs: &mut state.outputs,
                    globals: &mut state.globals,
                },
            )
            .await
            .map(|next| match next {
                Some(next) => NodeStepOutcome::Next(next),
                None => NodeStepOutcome::Terminated,
            })
        } else if let Some(human) = &node.node_type.human_input {
            let context = build_execution_context(workflow_input, &state.outputs, &state.globals);
            let request = build_human_request(node, human, &context)?;
            if let Some(span) = node_span.as_mut() {
                let human_input_type = match request.input_type {
                    YamlHumanInputType::Choice => "choice",
                    YamlHumanInputType::Text => "text",
                    YamlHumanInputType::Form => "form",
                };
                span.set_attribute("human_input.type", human_input_type);
                if let Some(prompt) = request.prompt.as_deref() {
                    span.set_attribute("human_input.prompt", prompt);
                }
                if request.input_type == YamlHumanInputType::Form {
                    span.set_attribute(
                        "human_input.has_form_schema",
                        request.form_schema.is_some().to_string().as_str(),
                    );
                }
            }
            let pending_metadata = build_human_pending_metadata(&request);
            state.outputs.insert(
                node.id.clone(),
                json!({
                    "status": "awaiting_human_input",
                    "human_input_metadata": pending_metadata,
                }),
            );
            if let Some(sink) = event_sink {
                sink.emit(&YamlWorkflowEvent {
                    event_type: "human_input_requested".to_string(),
                    node_id: Some(node.id.clone()),
                    step_id: Some(node.id.clone()),
                    node_kind: Some(node.kind_name().to_string()),
                    streamable: None,
                    message: None,
                    delta: None,
                    snapshot: state.outputs.get(node.id.as_str()).cloned(),
                    token_kind: None,
                    is_terminal_node_token: None,
                    elapsed_ms: Some(workflow_elapsed_before_node_ms),
                    metadata: Some(serde_json::to_value(&request).unwrap_or(Value::Null)),
                });
            }
            Ok(NodeStepOutcome::Paused(request))
        } else if node.node_type.end.is_some() {
            apply_set_globals(node, &state.outputs, workflow_input, &mut state.globals);
            apply_update_globals(node, &state.outputs, workflow_input, &mut state.globals);
            if let Some(end_config) = node.node_type.end.as_ref() {
                if end_config.is_object() && !end_config.as_object().unwrap().is_empty() {
                    state
                        .outputs
                        .insert(node.id.clone(), json!({ "output": end_config }));
                }
            }
            Ok(NodeStepOutcome::Terminated)
        } else {
            Err(YamlWorkflowRunError::UnsupportedNodeType {
                node_id: node.id.clone(),
            })
        };

    let elapsed_ms = step_started.elapsed().as_millis();
    finish_node_span(
        node_span.take(),
        node_model_name.as_deref(),
        node_usage.as_ref(),
        elapsed_ms,
    );

    let step_outcome = node_result?;

    record_node_timing_and_metrics(
        node,
        elapsed_ms,
        node_model_name.as_deref(),
        node_usage.as_ref(),
        &mut state.step_timings,
        &mut state.llm_node_metrics,
    );

    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "node_completed".to_string(),
            node_id: Some(node.id.clone()),
            step_id: Some(node.id.clone()),
            node_kind: Some(node.kind_name().to_string()),
            streamable: node_streamable,
            message: None,
            delta: None,
            snapshot: state.outputs.get(node.id.as_str()).cloned(),
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

    Ok(step_outcome)
}

fn build_human_request(
    node: &YamlNode,
    human: &YamlHumanInput,
    context: &Value,
) -> Result<HumanRequest, YamlWorkflowRunError> {
    let prompt = human
        .prompt
        .as_deref()
        .map(|value| interpolate_template(value, context));
    let form_data = human
        .form_prefill
        .as_deref()
        .map(|value| resolve_human_form_data(value, context));

    Ok(HumanRequest {
        node_id: node.id.clone(),
        input_type: human.input_type,
        prompt,
        options: human.options.clone(),
        form_schema: human.form_schema.clone(),
        form_data,
    })
}

fn resolve_human_form_data(form_prefill: &str, context: &Value) -> Value {
    let trimmed = form_prefill.trim();
    if let Some(expr) = trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
    {
        let source_path = expr.trim().trim_start_matches("$.");
        if let Some(value) = resolve_path(context, source_path) {
            return value.clone();
        }
        return Value::Null;
    }
    Value::String(interpolate_template(trimmed, context))
}

fn record_node_timing_and_metrics(
    node: &YamlNode,
    elapsed_ms: u128,
    node_model_name: Option<&str>,
    node_usage: Option<&YamlLlmTokenUsage>,
    step_timings: &mut Vec<YamlStepTiming>,
    llm_node_metrics: &mut BTreeMap<String, YamlLlmNodeMetrics>,
) {
    step_timings.push(YamlStepTiming {
        node_id: node.id.clone(),
        node_kind: node.kind_name().to_string(),
        model_name: node_model_name.map(ToString::to_string),
        elapsed_ms,
        prompt_tokens: node_usage.map(|usage| usage.prompt_tokens),
        completion_tokens: node_usage.map(|usage| usage.completion_tokens),
        total_tokens: node_usage.map(|usage| usage.total_tokens),
        reasoning_tokens: node_usage.and_then(|usage| usage.reasoning_tokens),
        tokens_per_second: node_usage
            .map(|usage| completion_tokens_per_second(usage.completion_tokens, elapsed_ms)),
    });

    if let Some(usage) = node_usage {
        llm_node_metrics.insert(
            node.id.clone(),
            YamlLlmNodeMetrics {
                elapsed_ms,
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                tokens_per_second: completion_tokens_per_second(
                    usage.completion_tokens,
                    elapsed_ms,
                ),
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_workflow_output(
    workflow: &YamlWorkflow,
    options: &YamlWorkflowRunOptions,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    telemetry_context: &ResolvedTelemetryContext,
    started: Instant,
    elapsed_before_resume_ms: u128,
    trace: Vec<String>,
    outputs: BTreeMap<String, Value>,
    globals: serde_json::Map<String, Value>,
    status: YamlWorkflowRunStatus,
    human_request: Option<HumanRequest>,
    step_timings: Vec<YamlStepTiming>,
    llm_node_metrics: BTreeMap<String, YamlLlmNodeMetrics>,
    llm_node_models: BTreeMap<String, String>,
    token_totals: YamlTokenTotals,
    workflow_ttft_ms: Option<u128>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
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

    let total_elapsed_ms = elapsed_before_resume_ms + started.elapsed().as_millis();
    let output = YamlWorkflowRunOutput {
        workflow_id: workflow.id.clone(),
        entry_node: workflow.entry_node.clone(),
        trace,
        outputs,
        globals: globals.into_iter().collect(),
        terminal_node,
        terminal_output,
        status,
        human_request,
        step_timings,
        llm_node_metrics,
        llm_node_models,
        total_elapsed_ms,
        ttft_ms: workflow_ttft_ms,
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

    let event_metadata = if options.telemetry.nerdstats {
        Some(json!({
            "nerdstats": workflow_nerdstats(&output),
        }))
    } else {
        None
    };

    if let Some(sink) = event_sink {
        let event_type = match output.status {
            YamlWorkflowRunStatus::Completed => "workflow_completed",
            YamlWorkflowRunStatus::AwaitingHumanInput => "workflow_paused_for_human_input",
        };
        sink.emit(&YamlWorkflowEvent {
            event_type: event_type.to_string(),
            node_id: Some(output.terminal_node.clone()),
            step_id: None,
            node_kind: None,
            streamable: None,
            message: None,
            delta: None,
            snapshot: serde_json::to_value(&output).ok(),
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

    Ok(output)
}

fn resolve_switch_target(
    node_id: &str,
    switch: &YamlSwitch,
    context: &Value,
) -> Result<String, YamlWorkflowRunError> {
    let mut chosen = Some(switch.default.clone());
    for branch in &switch.branches {
        if evaluate_switch_condition(branch.condition.as_str(), context)? {
            chosen = Some(branch.target.clone());
            break;
        }
    }

    chosen.ok_or_else(|| YamlWorkflowRunError::InvalidSwitchTarget {
        node_id: node_id.to_string(),
    })
}
