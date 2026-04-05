use super::events::{
    emit_node_completed, emit_node_started, emit_workflow_completed, emit_workflow_started,
    ensure_event_sink_active,
};
use super::node_execution::{
    execute_custom_worker_node, execute_llm_node, CustomWorkerEnv, CustomWorkerState, LlmNodeEnv,
    LlmNodeState,
};
use super::spans::{finish_node_span, finish_workflow_span, start_node_span, start_workflow_span};
use super::*;

pub(super) async fn run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
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

    if let Some(output) =
        try_run_yaml_via_ir_runtime(workflow, workflow_input, executor, custom_worker, options)
            .await?
    {
        return Ok(output);
    }

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
        let mut run_context = prepare_run_context(workflow)?;

        emit_workflow_started(event_sink, workflow.id.as_str());
        ensure_event_sink_active(event_sink)?;

        loop {
            ensure_event_sink_active(event_sink)?;

            let next = execute_single_node_step(
                workflow_input,
                executor,
                custom_worker,
                event_sink,
                options,
                tracer,
                &telemetry_context,
                workflow_span_context.as_ref(),
                &run_context.node_map,
                &run_context.edge_map,
                &mut run_context.state,
            )
            .await?;

            if let Some(next) = next {
                run_context.state.current = next;
                continue;
            }
            break;
        }

        let WorkflowRunState {
            trace,
            outputs,
            step_timings,
            llm_node_metrics,
            llm_node_models,
            token_totals,
            workflow_ttft_ms,
            started,
            ..
        } = run_context.state;

        finalize_workflow_output(
            workflow,
            options,
            event_sink,
            &telemetry_context,
            started,
            trace,
            outputs,
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

fn validate_custom_worker_handler_files(
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
    }

    Ok(())
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

struct PreparedRunContext<'a> {
    node_map: HashMap<&'a str, &'a YamlNode>,
    edge_map: HashMap<&'a str, &'a str>,
    state: WorkflowRunState,
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
}

fn prepare_run_context(
    workflow: &YamlWorkflow,
) -> Result<PreparedRunContext<'_>, YamlWorkflowRunError> {
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

    Ok(PreparedRunContext {
        node_map,
        edge_map,
        state: WorkflowRunState {
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
        },
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_single_node_step(
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
    tracer: &dyn crate::observability::tracing::WorkflowTracer,
    telemetry_context: &ResolvedTelemetryContext,
    workflow_span_context: Option<&TraceContext>,
    node_map: &HashMap<&str, &YamlNode>,
    edge_map: &HashMap<&str, &str>,
    state: &mut WorkflowRunState,
) -> Result<Option<String>, YamlWorkflowRunError> {
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

    let node_streamable = node
        .node_type
        .llm_call
        .as_ref()
        .map(|llm| llm.stream.unwrap_or(false) && !llm.heal.unwrap_or(false));
    let workflow_elapsed_before_node_ms = state.started.elapsed().as_millis();

    emit_node_started(
        event_sink,
        node.id.as_str(),
        node.kind_name(),
        node_streamable,
        workflow_elapsed_before_node_ms,
    );
    ensure_event_sink_active(event_sink)?;

    let mut node_usage: Option<YamlLlmTokenUsage> = None;
    let mut node_model_name: Option<String> = None;
    let is_terminal_node = !edge_map.contains_key(node.id.as_str());
    let node_result: Result<Option<String>, YamlWorkflowRunError> =
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
                outcome.next
            })
        } else if let Some(switch) = &node.node_type.switch {
            let context = build_execution_context(workflow_input, &state.outputs, &state.globals);
            resolve_switch_target(node.id.as_str(), switch, &context).map(Some)
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

    let next = node_result?;

    record_node_timing_and_metrics(
        node,
        elapsed_ms,
        node_model_name.as_deref(),
        node_usage.as_ref(),
        &mut state.step_timings,
        &mut state.llm_node_metrics,
    );

    emit_node_completed(
        event_sink,
        node.id.as_str(),
        node.kind_name(),
        node_streamable,
        elapsed_ms,
    );
    ensure_event_sink_active(event_sink)?;

    Ok(next)
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
    trace: Vec<String>,
    outputs: BTreeMap<String, Value>,
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

    let total_elapsed_ms = started.elapsed().as_millis();
    let output = YamlWorkflowRunOutput {
        workflow_id: workflow.id.clone(),
        entry_node: workflow.entry_node.clone(),
        trace,
        outputs,
        terminal_node,
        terminal_output,
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

    emit_workflow_completed(
        event_sink,
        output.terminal_node.as_str(),
        output.total_elapsed_ms,
        event_metadata,
    );
    ensure_event_sink_active(event_sink)?;

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
