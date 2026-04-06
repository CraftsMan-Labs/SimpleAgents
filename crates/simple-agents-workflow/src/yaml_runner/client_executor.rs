use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{json, Value};
use simple_agent_type::message::Message;
use simple_agent_type::request::CompletionRequest;
use simple_agent_type::response::FinishReason;
use simple_agent_type::tool::{ToolCall, ToolType};
use simple_agents_core::{CompletionMode, CompletionOptions, CompletionOutcome, SimpleAgentsClient};

use crate::observability::tracing::{SpanKind, TraceContext};

use super::contracts::{
    event_sink_is_cancelled, workflow_event_sink_cancelled_message,
    YamlLlmExecutionRequest, YamlResolvedTool, YamlWorkflowCustomWorkerExecutor,
    YamlWorkflowEvent, YamlWorkflowEventSink, YamlWorkflowLlmExecutor,
    YamlWorkflowRunError, YamlWorkflowTokenKind,
};
use super::stream_filters::{StructuredJsonDeltaFilter, StreamJsonAsTextFormatter, parse_streamed_structured_payload};
use super::telemetry::{
    apply_trace_identity_attributes, apply_trace_tenant_attributes_from_tenant,
    payload_for_span, payload_for_tool_trace, split_stream_deltas_enabled,
};
use super::types::{
    YamlLlmExecutionResult, YamlLlmTokenUsage, YamlToolCallTrace, YamlToolTraceMode,
    YamlWorkflowExecutionFlags, YamlWorkflowRunOptions,
};
use super::validation::{schema_expects_object, validate_schema_instance};
use super::subworkflow::execute_subworkflow_tool_call;
use crate::observability::tracing::workflow_tracer;

pub(super) struct BorrowedClientExecutor<'a> {
    pub(super) client: &'a SimpleAgentsClient,
    pub(super) custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    pub(super) run_options: YamlWorkflowRunOptions,
}

fn elapsed_millis_with_min_one(started: &Instant) -> u128 {
    let elapsed = started.elapsed().as_millis();
    if elapsed == 0 {
        1
    } else {
        elapsed
    }
}

#[async_trait]
impl<'a> YamlWorkflowLlmExecutor for BorrowedClientExecutor<'a> {
    async fn complete_structured(
        &self,
        request: YamlLlmExecutionRequest,
        event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<YamlLlmExecutionResult, String> {
        let expects_object = schema_expects_object(&request.schema);
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

        if !request.tools.is_empty() {
            let mut tool_traces: Vec<YamlToolCallTrace> = Vec::new();
            let mut conversation = messages;
            let mut usage_total: Option<YamlLlmTokenUsage> = None;

            for roundtrip in 0..=request.max_tool_roundtrips {
                let mut builder = CompletionRequest::builder()
                    .model(&request.model)
                    .messages(conversation.clone())
                    .tools(request.tools.iter().map(|t| t.definition.clone()).collect());

                if let Some(max_tokens) = request.max_tokens {
                    builder = builder.max_tokens(max_tokens);
                }
                if let Some(temperature) = request.temperature {
                    builder = builder.temperature(temperature);
                }
                if let Some(top_p) = request.top_p {
                    builder = builder.top_p(top_p);
                }

                if request.heal && expects_object {
                    builder = builder.json_schema("workflow_step", request.schema.clone());
                }

                if let Some(choice) = request.tool_choice.clone() {
                    builder = builder.tool_choice(choice);
                }

                if request.stream {
                    builder = builder.stream(true);
                }

                let completion_request = builder
                    .build()
                    .map_err(|error| format!("failed to build completion request: {error}"))?;

                let outcome = self
                    .client
                    .complete(&completion_request, CompletionOptions::default())
                    .await
                    .map_err(|error| error.to_string())?;

                let mut streamed_tool_calls: Option<Vec<ToolCall>> = None;
                let mut streamed_content = String::new();
                let mut finish_reason = FinishReason::Stop;

                match outcome {
                    CompletionOutcome::Response(response) => {
                        if let Some(usage) = usage_total.as_mut() {
                            usage.prompt_tokens += response.usage.prompt_tokens;
                            usage.completion_tokens += response.usage.completion_tokens;
                            usage.total_tokens += response.usage.total_tokens;
                            if let Some(reasoning_tokens) = response.usage.reasoning_tokens {
                                usage.reasoning_tokens =
                                    Some(usage.reasoning_tokens.unwrap_or(0) + reasoning_tokens);
                            }
                        } else {
                            usage_total = Some(YamlLlmTokenUsage {
                                prompt_tokens: response.usage.prompt_tokens,
                                completion_tokens: response.usage.completion_tokens,
                                total_tokens: response.usage.total_tokens,
                                reasoning_tokens: response.usage.reasoning_tokens,
                            });
                        }

                        let choice = response
                            .choices
                            .first()
                            .ok_or_else(|| "completion returned no choices".to_string())?;
                        streamed_content = choice.message.content_text().to_string();
                        streamed_tool_calls = choice.message.tool_calls.clone();
                        finish_reason = choice.finish_reason;
                    }
                    CompletionOutcome::HealedJson(healed) => {
                        let response = healed.response;
                        if let Some(usage) = usage_total.as_mut() {
                            usage.prompt_tokens += response.usage.prompt_tokens;
                            usage.completion_tokens += response.usage.completion_tokens;
                            usage.total_tokens += response.usage.total_tokens;
                            if let Some(reasoning_tokens) = response.usage.reasoning_tokens {
                                usage.reasoning_tokens =
                                    Some(usage.reasoning_tokens.unwrap_or(0) + reasoning_tokens);
                            }
                        } else {
                            usage_total = Some(YamlLlmTokenUsage {
                                prompt_tokens: response.usage.prompt_tokens,
                                completion_tokens: response.usage.completion_tokens,
                                total_tokens: response.usage.total_tokens,
                                reasoning_tokens: response.usage.reasoning_tokens,
                            });
                        }

                        let choice = response
                            .choices
                            .first()
                            .ok_or_else(|| "completion returned no choices".to_string())?;
                        streamed_content = choice.message.content_text().to_string();
                        streamed_tool_calls = choice.message.tool_calls.clone();
                        finish_reason = choice.finish_reason;
                    }
                    CompletionOutcome::CoercedSchema(coerced) => {
                        let response = coerced.response;
                        if let Some(usage) = usage_total.as_mut() {
                            usage.prompt_tokens += response.usage.prompt_tokens;
                            usage.completion_tokens += response.usage.completion_tokens;
                            usage.total_tokens += response.usage.total_tokens;
                            if let Some(reasoning_tokens) = response.usage.reasoning_tokens {
                                usage.reasoning_tokens =
                                    Some(usage.reasoning_tokens.unwrap_or(0) + reasoning_tokens);
                            }
                        } else {
                            usage_total = Some(YamlLlmTokenUsage {
                                prompt_tokens: response.usage.prompt_tokens,
                                completion_tokens: response.usage.completion_tokens,
                                total_tokens: response.usage.total_tokens,
                                reasoning_tokens: response.usage.reasoning_tokens,
                            });
                        }

                        let choice = response
                            .choices
                            .first()
                            .ok_or_else(|| "completion returned no choices".to_string())?;
                        streamed_content = choice.message.content_text().to_string();
                        streamed_tool_calls = choice.message.tool_calls.clone();
                        finish_reason = choice.finish_reason;
                    }
                    CompletionOutcome::Stream(mut stream) => {
                        let mut final_stream_usage: Option<simple_agent_type::response::Usage> =
                            None;
                        let mut delta_filter = StructuredJsonDeltaFilter::default();
                        let include_raw_debug = super::split_stream_deltas_enabled(&request);
                        let mut json_text_formatter = if request.stream_json_as_text {
                            Some(StreamJsonAsTextFormatter::default())
                        } else {
                            None
                        };
                        let mut tool_calls_by_index: HashMap<u32, ToolCall> = HashMap::new();

                        while let Some(chunk_result) = stream.next().await {
                            if event_sink_is_cancelled(event_sink) {
                                return Err(workflow_event_sink_cancelled_message().to_string());
                            }

                            let chunk = chunk_result.map_err(|error| error.to_string())?;
                            if let Some(usage) = chunk.usage {
                                final_stream_usage = Some(usage);
                            }

                            if let Some(choice) = chunk.choices.first() {
                                if let Some(chunk_finish_reason) = choice.finish_reason {
                                    finish_reason = chunk_finish_reason;
                                }

                                if include_raw_debug {
                                    if let Some(reasoning_delta) =
                                        choice.delta.reasoning_content.as_ref()
                                    {
                                        if let Some(sink) = event_sink {
                                            sink.emit(&YamlWorkflowEvent {
                                                event_type: "node_stream_thinking_delta"
                                                    .to_string(),
                                                node_id: Some(request.node_id.clone()),
                                                step_id: Some(request.node_id.clone()),
                                                node_kind: Some("llm_call".to_string()),
                                                streamable: Some(true),
                                                message: None,
                                                delta: Some(reasoning_delta.clone()),
                                                token_kind: Some(YamlWorkflowTokenKind::Thinking),
                                                is_terminal_node_token: Some(
                                                    request.is_terminal_node,
                                                ),
                                                elapsed_ms: None,
                                                metadata: None,
                                            });
                                        }
                                    }
                                }

                                if let Some(delta) = choice.delta.content.clone() {
                                    streamed_content.push_str(delta.as_str());
                                    let (output_delta, thinking_delta) = if expects_object {
                                        delta_filter.split(delta.as_str())
                                    } else {
                                        (Some(delta.clone()), None)
                                    };
                                    let rendered_output_delta =
                                        if let Some(output_chunk) = output_delta {
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
                                            if let Some(raw_thinking_delta) =
                                                thinking_delta.as_ref()
                                            {
                                                sink.emit(&YamlWorkflowEvent {
                                                    event_type: "node_stream_thinking_delta"
                                                        .to_string(),
                                                    node_id: Some(request.node_id.clone()),
                                                    step_id: Some(request.node_id.clone()),
                                                    node_kind: Some("llm_call".to_string()),
                                                    streamable: Some(true),
                                                    message: None,
                                                    delta: Some(raw_thinking_delta.clone()),
                                                    token_kind: Some(
                                                        YamlWorkflowTokenKind::Thinking,
                                                    ),
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
                                                    event_type: "node_stream_output_delta"
                                                        .to_string(),
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
                                                is_terminal_node_token: Some(
                                                    request.is_terminal_node,
                                                ),
                                                elapsed_ms: None,
                                                metadata: None,
                                            });
                                        }
                                    }
                                }

                                if let Some(tool_call_deltas) = choice.delta.tool_calls.as_ref() {
                                    for tool_call_delta in tool_call_deltas {
                                        let entry = tool_calls_by_index
                                            .entry(tool_call_delta.index)
                                            .or_insert_with(|| ToolCall {
                                                id: tool_call_delta.id.clone().unwrap_or_else(
                                                    || {
                                                        format!(
                                                            "tool_call_{}",
                                                            tool_call_delta.index
                                                        )
                                                    },
                                                ),
                                                tool_type: ToolType::Function,
                                                function:
                                                    simple_agent_type::tool::ToolCallFunction {
                                                        name: String::new(),
                                                        arguments: String::new(),
                                                    },
                                            });

                                        if let Some(id) = tool_call_delta.id.as_ref() {
                                            entry.id = id.clone();
                                        }
                                        if let Some(tool_type) = tool_call_delta.tool_type {
                                            entry.tool_type = tool_type;
                                        }
                                        if let Some(function_delta) =
                                            tool_call_delta.function.as_ref()
                                        {
                                            if let Some(name) = function_delta.name.as_ref() {
                                                entry.function.name = name.clone();
                                            }
                                            if let Some(arguments) =
                                                function_delta.arguments.as_ref()
                                            {
                                                entry.function.arguments.push_str(arguments);
                                            }
                                        }
                                    }
                                }
                            }

                            if event_sink_is_cancelled(event_sink) {
                                return Err(workflow_event_sink_cancelled_message().to_string());
                            }
                        }

                        if let Some(usage) = final_stream_usage {
                            if let Some(total) = usage_total.as_mut() {
                                total.prompt_tokens += usage.prompt_tokens;
                                total.completion_tokens += usage.completion_tokens;
                                total.total_tokens += usage.total_tokens;
                                if let Some(reasoning_tokens) = usage.reasoning_tokens {
                                    total.reasoning_tokens = Some(
                                        total.reasoning_tokens.unwrap_or(0) + reasoning_tokens,
                                    );
                                }
                            } else {
                                usage_total = Some(YamlLlmTokenUsage {
                                    prompt_tokens: usage.prompt_tokens,
                                    completion_tokens: usage.completion_tokens,
                                    total_tokens: usage.total_tokens,
                                    reasoning_tokens: usage.reasoning_tokens,
                                });
                            }
                        }

                        let mut ordered_tool_calls =
                            tool_calls_by_index.into_iter().collect::<Vec<_>>();
                        ordered_tool_calls.sort_by_key(|(index, _)| *index);
                        if !ordered_tool_calls.is_empty() {
                            streamed_tool_calls = Some(
                                ordered_tool_calls
                                    .into_iter()
                                    .map(|(_, tool_call)| tool_call)
                                    .collect::<Vec<_>>(),
                            );
                        }
                    }
                }

                let has_tool_calls = streamed_tool_calls
                    .as_ref()
                    .is_some_and(|calls| !calls.is_empty());
                if finish_reason != FinishReason::ToolCalls && !has_tool_calls {
                    let payload = if expects_object {
                        parse_streamed_structured_payload(streamed_content.as_str(), request.heal)
                            .map_err(|error| {
                                format!("failed to parse structured completion JSON: {error}")
                            })?
                            .payload
                    } else {
                        Value::String(streamed_content.clone())
                    };
                    return Ok(YamlLlmExecutionResult {
                        payload,
                        usage: usage_total,
                        ttft_ms: None,
                        tool_calls: tool_traces,
                    });
                }

                if roundtrip >= request.max_tool_roundtrips {
                    return Err(format!(
                        "tool call roundtrip limit reached for node '{}' (max={})",
                        request.node_id, request.max_tool_roundtrips
                    ));
                }

                let tool_calls: Vec<ToolCall> = streamed_tool_calls.ok_or_else(|| {
                    "finish_reason=tool_calls but no tool calls found".to_string()
                })?;
                if tool_calls
                    .iter()
                    .any(|tool_call| tool_call.function.name.trim().is_empty())
                {
                    return Err("streamed tool call missing function name".to_string());
                }

                let assistant_tool_message =
                    Message::assistant(&streamed_content).with_tool_calls(tool_calls.clone());
                conversation.push(assistant_tool_message);

                for tool_call in tool_calls {
                    let tool_call_id = tool_call.id.clone();
                    let tool_name = tool_call.function.name.clone();
                    let tool_started = Instant::now();
                    let arguments: Value = serde_json::from_str(&tool_call.function.arguments)
                        .map_err(|error| {
                            format!(
                                "tool '{}' arguments must be valid JSON: {}",
                                tool_name, error
                            )
                        })?;
                    let mut tool_span_context: Option<TraceContext> = None;
                    let mut tool_span = if request.trace_sampled {
                        let (span_context, mut span) = workflow_tracer().start_span(
                            "workflow.tool.execute",
                            SpanKind::Node,
                            request.trace_context.as_ref(),
                        );
                        tool_span_context = Some(span_context);
                        apply_trace_identity_attributes(span.as_mut(), request.trace_id.as_deref());
                        apply_trace_tenant_attributes_from_tenant(
                            span.as_mut(),
                            &request.tenant_context,
                        );
                        span.set_attribute("node_id", request.node_id.as_str());
                        span.set_attribute("node_kind", "llm_call");
                        span.set_attribute("tool_name", tool_name.as_str());
                        span.set_attribute("tool_call_id", tool_call_id.as_str());
                        let args_for_span =
                            payload_for_tool_trace(request.tool_trace_mode, &arguments).to_string();
                        span.set_attribute("tool_arguments", args_for_span.as_str());
                        Some(span)
                    } else {
                        None
                    };

                    if request.tool_trace_mode != YamlToolTraceMode::Off {
                        if let Some(sink) = event_sink {
                            sink.emit(&YamlWorkflowEvent {
                                event_type: "node_tool_call_requested".to_string(),
                                node_id: Some(request.node_id.clone()),
                                step_id: Some(request.node_id.clone()),
                                node_kind: Some("llm_call".to_string()),
                                streamable: Some(false),
                                message: Some(format!(
                                    "tool call requested: {}",
                                    tool_name
                                )),
                                delta: None,
                                token_kind: None,
                                is_terminal_node_token: None,
                                elapsed_ms: None,
                                metadata: Some(json!({
                                    "tool_call_id": tool_call_id.clone(),
                                    "tool_name": tool_name.clone(),
                                    "arguments": payload_for_tool_trace(request.tool_trace_mode, &arguments),
                                })),
                            });
                        }
                    }

                    let Some(tool_config) = request
                        .tools
                        .iter()
                        .find(|tool| tool.definition.function.name == tool_name)
                    else {
                        return Err(format!("model requested unknown tool '{}'", tool_name));
                    };

                    let tool_output_result = if tool_name == "run_workflow_graph" {
                        execute_subworkflow_tool_call(
                            &arguments,
                            &request.execution_context,
                            self.client,
                            self.custom_worker,
                            &self.run_options,
                            tool_span_context.as_ref(),
                            request.trace_id.as_deref(),
                        )
                        .await
                    } else if let Some(custom_worker) = self.custom_worker {
                        custom_worker
                            .execute(
                                tool_name.as_str(),
                                None,
                                &arguments,
                                &request.execution_context,
                            )
                            .await
                    } else {
                        Err(format!(
                            "tool '{}' requested but no custom worker executor is configured",
                            tool_name
                        ))
                    };

                    let tool_output = match tool_output_result {
                        Ok(output) => output,
                        Err(message) => {
                            let elapsed_ms = tool_started.elapsed().as_millis();
                            if let Some(span) = tool_span.as_mut() {
                                span.add_event("workflow.tool.execute.error");
                                span.set_attribute("tool_status", "error");
                                span.set_attribute("tool_error", message.as_str());
                                span.set_attribute("elapsed_ms", elapsed_ms.to_string().as_str());
                            }
                            if request.tool_trace_mode != YamlToolTraceMode::Off {
                                if let Some(sink) = event_sink {
                                    sink.emit(&YamlWorkflowEvent {
                                        event_type: "node_tool_call_failed".to_string(),
                                        node_id: Some(request.node_id.clone()),
                                        step_id: Some(request.node_id.clone()),
                                        node_kind: Some("llm_call".to_string()),
                                        streamable: Some(false),
                                        message: Some(message.clone()),
                                        delta: None,
                                        token_kind: None,
                                        is_terminal_node_token: None,
                                        elapsed_ms: Some(elapsed_ms),
                                        metadata: Some(json!({
                                            "tool_call_id": tool_call_id.clone(),
                                            "tool_name": tool_name.clone(),
                                        })),
                                    });
                                }
                            }
                            tool_traces.push(YamlToolCallTrace {
                                id: tool_call_id.clone(),
                                name: tool_name.clone(),
                                arguments,
                                output: None,
                                status: "error".to_string(),
                                elapsed_ms,
                                error: Some(message.clone()),
                            });
                            if let Some(span) = tool_span.take() {
                                span.end();
                            }
                            return Err(format!("tool '{}' failed: {}", tool_name, message));
                        }
                    };

                    if let Some(output_schema) = tool_config.output_schema.as_ref() {
                        validate_schema_instance(output_schema, &tool_output).map_err(
                            |message| {
                                format!(
                                    "tool '{}' output failed schema validation: {}",
                                    tool_name, message
                                )
                            },
                        )?;
                    }

                    let elapsed_ms = tool_started.elapsed().as_millis();
                    if let Some(span) = tool_span.as_mut() {
                        span.add_event("workflow.tool.execute.completed");
                        span.set_attribute("tool_status", "ok");
                        span.set_attribute("elapsed_ms", elapsed_ms.to_string().as_str());
                        let output_for_span =
                            payload_for_tool_trace(request.tool_trace_mode, &tool_output)
                                .to_string();
                        span.set_attribute("tool_output", output_for_span.as_str());
                    }
                    if request.tool_trace_mode != YamlToolTraceMode::Off {
                        if let Some(sink) = event_sink {
                            sink.emit(&YamlWorkflowEvent {
                                event_type: "node_tool_call_completed".to_string(),
                                node_id: Some(request.node_id.clone()),
                                step_id: Some(request.node_id.clone()),
                                node_kind: Some("llm_call".to_string()),
                                streamable: Some(false),
                                message: Some(format!(
                                    "tool call completed: {}",
                                    tool_name
                                )),
                                delta: None,
                                token_kind: None,
                                is_terminal_node_token: None,
                                elapsed_ms: Some(elapsed_ms),
                                metadata: Some(json!({
                                    "tool_call_id": tool_call_id.clone(),
                                    "tool_name": tool_name.clone(),
                                    "arguments": payload_for_tool_trace(request.tool_trace_mode, &arguments),
                                    "output": payload_for_tool_trace(request.tool_trace_mode, &tool_output),
                                })),
                            });
                        }
                    }

                    tool_traces.push(YamlToolCallTrace {
                        id: tool_call_id.clone(),
                        name: tool_name.clone(),
                        arguments: arguments.clone(),
                        output: Some(tool_output.clone()),
                        status: "ok".to_string(),
                        elapsed_ms,
                        error: None,
                    });

                    conversation.push(Message::tool(
                        serde_json::to_string(&tool_output)
                            .map_err(|error| format!("failed to serialize tool output: {error}"))?,
                        tool_call_id,
                    ));
                    if let Some(span) = tool_span.take() {
                        span.end();
                    }
                }

                if request.tool_trace_mode != YamlToolTraceMode::Off {
                    if let Some(sink) = event_sink {
                        sink.emit(&YamlWorkflowEvent {
                            event_type: "node_tool_roundtrip_completed".to_string(),
                            node_id: Some(request.node_id.clone()),
                            step_id: Some(request.node_id.clone()),
                            node_kind: Some("llm_call".to_string()),
                            streamable: Some(false),
                            message: Some(format!("tool roundtrip {} completed", roundtrip + 1)),
                            delta: None,
                            token_kind: None,
                            is_terminal_node_token: None,
                            elapsed_ms: None,
                            metadata: Some(json!({
                                "roundtrip": roundtrip + 1,
                                "max_tool_roundtrips": request.max_tool_roundtrips,
                            })),
                        });
                    }
                }
            }

            return Err(format!(
                "tool-enabled llm_call '{}' exhausted loop without final payload",
                request.node_id
            ));
        }

        let mut builder = CompletionRequest::builder()
            .model(&request.model)
            .messages(messages);

        if let Some(max_tokens) = request.max_tokens {
            builder = builder.max_tokens(max_tokens);
        }
        if let Some(temperature) = request.temperature {
            builder = builder.temperature(temperature);
        }
        if let Some(top_p) = request.top_p {
            builder = builder.top_p(top_p);
        }

        if request.heal && !request.stream && expects_object {
            builder = builder.json_schema("workflow_step", request.schema.clone());
        }

        if request.stream {
            builder = builder.stream(true);
        }

        let completion_request = builder
            .build()
            .map_err(|error| format!("failed to build completion request: {error}"))?;

        let completion_options = if request.heal && !request.stream && expects_object {
            CompletionOptions {
                mode: CompletionMode::HealedJson,
            }
        } else {
            CompletionOptions::default()
        };

        let request_started = Instant::now();
        let outcome = self
            .client
            .complete(&completion_request, completion_options)
            .await
            .map_err(|error| error.to_string())?;

        match outcome {
            CompletionOutcome::Stream(mut stream) => {
                let mut aggregated = String::new();
                let mut final_stream_usage: Option<simple_agent_type::response::Usage> = None;
                let stream_started = request_started;
                let mut ttft_ms: Option<u128> = None;
                let mut delta_filter = StructuredJsonDeltaFilter::default();
                let include_raw_debug = super::split_stream_deltas_enabled(&request);
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
                            ttft_ms = Some(elapsed_millis_with_min_one(&stream_started));
                        }
                        if include_raw_debug {
                            if let Some(reasoning_delta) = choice.delta.reasoning_content.as_ref() {
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
                            let (output_delta, thinking_delta) = if expects_object {
                                delta_filter.split(delta.as_str())
                            } else {
                                (Some(delta.clone()), None)
                            };
                            let rendered_output_delta = if let Some(output_chunk) = output_delta {
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
                                            event_type: "node_stream_thinking_delta".to_string(),
                                            node_id: Some(request.node_id.clone()),
                                            step_id: Some(request.node_id.clone()),
                                            node_kind: Some("llm_call".to_string()),
                                            streamable: Some(true),
                                            message: None,
                                            delta: Some(raw_thinking_delta.clone()),
                                            token_kind: Some(YamlWorkflowTokenKind::Thinking),
                                            is_terminal_node_token: Some(request.is_terminal_node),
                                            elapsed_ms: None,
                                            metadata: None,
                                        });
                                    }
                                    if let Some(raw_output_delta) = rendered_output_delta.as_ref() {
                                        sink.emit(&YamlWorkflowEvent {
                                            event_type: "node_stream_output_delta".to_string(),
                                            node_id: Some(request.node_id.clone()),
                                            step_id: Some(request.node_id.clone()),
                                            node_kind: Some("llm_call".to_string()),
                                            streamable: Some(true),
                                            message: None,
                                            delta: Some(raw_output_delta.clone()),
                                            token_kind: Some(YamlWorkflowTokenKind::Output),
                                            is_terminal_node_token: Some(request.is_terminal_node),
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

                let payload = if expects_object {
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
                    resolved.payload
                } else {
                    Value::String(aggregated)
                };

                Ok(YamlLlmExecutionResult {
                    payload,
                    usage: final_stream_usage.map(|usage| YamlLlmTokenUsage {
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        total_tokens: usage.total_tokens,
                        reasoning_tokens: usage.reasoning_tokens,
                    }),
                    ttft_ms,
                    tool_calls: Vec::new(),
                })
            }
            CompletionOutcome::Response(response) => {
                let payload = if expects_object {
                    let content = response
                        .content()
                        .ok_or_else(|| "completion returned empty content".to_string())?;
                    serde_json::from_str(content).map_err(|error| {
                        format!("failed to parse structured completion JSON: {error}")
                    })?
                } else {
                    Value::String(response.content().unwrap_or_default().to_string())
                };

                Ok(YamlLlmExecutionResult {
                    payload,
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: response.usage.prompt_tokens,
                        completion_tokens: response.usage.completion_tokens,
                        total_tokens: response.usage.total_tokens,
                        reasoning_tokens: response.usage.reasoning_tokens,
                    }),
                    ttft_ms: None,
                    tool_calls: Vec::new(),
                })
            }
            CompletionOutcome::HealedJson(healed) => {
                if !expects_object {
                    return Err(
                        "healed json outcome is unsupported for non-object schema".to_string()
                    );
                }
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
                        reasoning_tokens: healed.response.usage.reasoning_tokens,
                    }),
                    ttft_ms: None,
                    tool_calls: Vec::new(),
                })
            }
            CompletionOutcome::CoercedSchema(coerced) => {
                if !expects_object {
                    return Err(
                        "coerced schema outcome is unsupported for non-object schema".to_string(),
                    );
                }
                Ok(YamlLlmExecutionResult {
                    payload: coerced.coerced.value,
                    usage: Some(YamlLlmTokenUsage {
                        prompt_tokens: coerced.response.usage.prompt_tokens,
                        completion_tokens: coerced.response.usage.completion_tokens,
                        total_tokens: coerced.response.usage.total_tokens,
                        reasoning_tokens: coerced.response.usage.reasoning_tokens,
                    }),
                    ttft_ms: None,
                    tool_calls: Vec::new(),
                })
            }
        }
    }
}
