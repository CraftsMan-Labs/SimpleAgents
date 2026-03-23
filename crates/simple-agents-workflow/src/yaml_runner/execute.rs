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
    let mut llm_node_models: BTreeMap<String, String> = BTreeMap::new();
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

        let mut node_span_context: Option<TraceContext> = None;
        let mut node_span = if telemetry_context.sampled {
            let (span_context, mut span) = tracer.start_span(
                "workflow.node.execute",
                SpanKind::Node,
                workflow_span_context.as_ref(),
            );
            node_span_context = Some(span_context);
            apply_trace_identity_attributes(span.as_mut(), telemetry_context.trace_id.as_deref());
            apply_trace_tenant_attributes(span.as_mut(), options);
            span.set_attribute("node_id", node.id.as_str());
            span.set_attribute("node_kind", node.kind_name());
            if node.kind_name() == "llm_call" {
                span.set_attribute("langfuse.observation.type", "generation");
            }
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
        let mut node_model_name: Option<String> = None;
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
                model: resolve_requested_model(options.model.as_deref(), &llm.model),
                messages,
                append_prompt_as_user: llm.append_prompt_as_user.unwrap_or(true),
                prompt,
                prompt_template: prompt_template.to_string(),
                prompt_bindings,
                schema,
                stream: llm.stream.unwrap_or(false),
                heal: llm.heal.unwrap_or(false),
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
                email_text: email_text.to_string(),
                trace_id: telemetry_context.trace_id.clone(),
                trace_context: node_span_context.clone(),
                tenant_context: options.trace.tenant.clone(),
                trace_sampled: telemetry_context.sampled,
            };

            if let Some(span) = node_span.as_mut() {
                let node_input = payload_for_span(options.telemetry.payload_mode, &context);
                span.set_attribute("node_input", node_input.as_str());
                span.set_attribute("langfuse.observation.input", node_input.as_str());
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
                        "tools_count": request.tools.len(),
                        "max_tool_roundtrips": request.max_tool_roundtrips,
                    })),
                });
            }

            node_model_name = Some(request.model.clone());
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
                workflow_ttft_ms = llm_result
                    .ttft_ms
                    .map(|node_ttft_ms| workflow_elapsed_before_node_ms + node_ttft_ms);
            }
            node_usage = llm_result.usage;

            let payload = llm_result.payload;
            let tool_calls = llm_result.tool_calls;

            let mut node_output = json!({ "output": payload });
            if !tool_calls.is_empty() {
                if let Some(output_obj) = node_output.as_object_mut() {
                    output_obj.insert("tool_calls".to_string(), json!(tool_calls));
                }
            }
            outputs.insert(node.id.clone(), node_output);
            if let Some(span) = node_span.as_mut() {
                if let Some(output_payload) = outputs.get(node.id.as_str()) {
                    let node_output =
                        payload_for_span(options.telemetry.payload_mode, output_payload);
                    span.set_attribute("node_output", node_output.as_str());
                    span.set_attribute("langfuse.observation.output", node_output.as_str());
                }
            }
            apply_set_globals(node, &outputs, workflow_input, &mut globals);
            apply_update_globals(node, &outputs, workflow_input, &mut globals);
            if let Some(global_key) = llm.tool_calls_global_key.as_ref() {
                if let Some(node_tool_calls) = outputs
                    .get(node.id.as_str())
                    .and_then(|value| value.get("tool_calls"))
                    .cloned()
                {
                    globals.insert(global_key.clone(), node_tool_calls);
                }
            }
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
                let node_input = payload_for_span(options.telemetry.payload_mode, &payload);
                span.set_attribute("node_input", node_input.as_str());
                span.set_attribute("langfuse.observation.input", node_input.as_str());
            }

            let mut handler_span_context: Option<TraceContext> = None;
            let mut handler_span = if telemetry_context.sampled {
                let (span_context, mut span) = tracer.start_span(
                    "handler.invoke",
                    SpanKind::Node,
                    workflow_span_context.as_ref(),
                );
                handler_span_context = Some(span_context);
                apply_trace_identity_attributes(
                    span.as_mut(),
                    telemetry_context.trace_id.as_deref(),
                );
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
            if let Some(span) = node_span.as_mut() {
                if let Some(output_payload) = outputs.get(node.id.as_str()) {
                    let node_output =
                        payload_for_span(options.telemetry.payload_mode, output_payload);
                    span.set_attribute("node_output", node_output.as_str());
                    span.set_attribute("langfuse.observation.output", node_output.as_str());
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
            model_name: node_model_name.clone(),
            elapsed_ms,
            prompt_tokens: node_usage.as_ref().map(|usage| usage.prompt_tokens),
            completion_tokens: node_usage.as_ref().map(|usage| usage.completion_tokens),
            total_tokens: node_usage.as_ref().map(|usage| usage.total_tokens),
            reasoning_tokens: node_usage.as_ref().and_then(|usage| usage.reasoning_tokens),
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
                    reasoning_tokens: usage.reasoning_tokens,
                    tokens_per_second: completion_tokens_per_second(
                        usage.completion_tokens,
                        elapsed_ms,
                    ),
                },
            );
        }

        if let Some(mut span) = node_span.take() {
            if let Some(model_name) = node_model_name.as_deref() {
                span.set_attribute("langfuse.observation.model.name", model_name);
                span.set_attribute("gen_ai.request.model", model_name);
            }
            if let Some(usage) = node_usage.as_ref() {
                apply_langfuse_observation_usage_attributes(span.as_mut(), usage);
            }
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

    Ok(output)
}
