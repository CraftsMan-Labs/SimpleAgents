use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use simple_agent_type::message::Message;
use simple_agent_type::request::CompletionRequest;
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, SimpleAgentsClient,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlStepTiming {
    pub node_id: String,
    pub node_kind: String,
    pub elapsed_ms: u128,
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
    pub total_elapsed_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YamlWorkflowEvent {
    pub event_type: String,
    pub node_id: Option<String>,
    pub node_kind: Option<String>,
    pub streamable: Option<bool>,
    pub message: Option<String>,
    pub delta: Option<String>,
    pub elapsed_ms: Option<u128>,
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
    #[error("llm schema mapping not defined for node '{node_id}'")]
    MissingLlmSchema { node_id: String },
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
}

pub trait YamlWorkflowEventSink: Send + Sync {
    fn emit(&self, event: &YamlWorkflowEvent);
}

pub struct NoopYamlWorkflowEventSink;

impl YamlWorkflowEventSink for NoopYamlWorkflowEventSink {
    fn emit(&self, _event: &YamlWorkflowEvent) {}
}

#[derive(Debug, Clone)]
pub struct YamlLlmExecutionRequest {
    pub node_id: String,
    pub model: String,
    pub prompt: String,
    pub schema: Value,
    pub stream: bool,
    pub heal: bool,
}

#[async_trait]
pub trait YamlWorkflowLlmExecutor: Send + Sync {
    async fn complete_structured(
        &self,
        request: YamlLlmExecutionRequest,
        event_sink: Option<&dyn YamlWorkflowEventSink>,
    ) -> Result<Value, String>;
}

#[async_trait]
pub trait YamlWorkflowCustomWorkerExecutor: Send + Sync {
    async fn execute(
        &self,
        handler: &str,
        payload: &Value,
        email_text: &str,
        context: &Value,
    ) -> Result<Value, String>;
}

pub async fn run_email_workflow_yaml_file(
    workflow_path: &Path,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_email_workflow_yaml(&workflow, email_text, executor).await
}

pub async fn run_email_workflow_yaml_file_with_client(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_email_workflow_yaml_with_client(&workflow, email_text, client).await
}

pub async fn run_email_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_email_workflow_yaml_with_client_and_custom_worker(workflow, email_text, client, None).await
}

pub async fn run_email_workflow_yaml_file_with_client_and_custom_worker(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_email_workflow_yaml_with_client_and_custom_worker(
        &workflow,
        email_text,
        client,
        custom_worker,
    )
    .await
}

pub async fn run_email_workflow_yaml_file_with_client_and_custom_worker_and_events(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let contents =
        std::fs::read_to_string(workflow_path).map_err(|source| YamlWorkflowRunError::Read {
            path: workflow_path.display().to_string(),
            source,
        })?;

    let workflow: YamlWorkflow =
        serde_yaml::from_str(&contents).map_err(|source| YamlWorkflowRunError::Parse {
            path: workflow_path.display().to_string(),
            source,
        })?;

    run_email_workflow_yaml_with_client_and_custom_worker_and_events(
        &workflow,
        email_text,
        client,
        custom_worker,
        event_sink,
    )
    .await
}

pub async fn run_email_workflow_yaml_with_client_and_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_email_workflow_yaml_with_client_and_custom_worker_and_events(
        workflow,
        email_text,
        client,
        custom_worker,
        None,
    )
    .await
}

pub async fn run_email_workflow_yaml_with_client_and_custom_worker_and_events(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    struct BorrowedClientExecutor<'a> {
        client: &'a SimpleAgentsClient,
    }

    #[async_trait]
    impl<'a> YamlWorkflowLlmExecutor for BorrowedClientExecutor<'a> {
        async fn complete_structured(
            &self,
            request: YamlLlmExecutionRequest,
            event_sink: Option<&dyn YamlWorkflowEventSink>,
        ) -> Result<Value, String> {
            let mut effective_stream = request.stream;
            if request.heal && request.stream {
                effective_stream = false;
                if let Some(sink) = event_sink {
                    sink.emit(&YamlWorkflowEvent {
                        event_type: "node_streaming_unavailable".to_string(),
                        node_id: Some(request.node_id.clone()),
                        node_kind: Some("llm_call".to_string()),
                        streamable: Some(false),
                        message: Some(
                            "stream disabled because heal=true requires non-stream completion"
                                .to_string(),
                        ),
                        delta: None,
                        elapsed_ms: None,
                    });
                }
            }

            let mut builder = CompletionRequest::builder()
                .model(&request.model)
                .messages(vec![
                    Message::system("You execute workflow classification steps."),
                    Message::user(&request.prompt),
                ]);

            if effective_stream {
                builder = builder.stream(true);
            }

            let completion_request = builder
                .build()
                .map_err(|error| format!("failed to build completion request: {error}"))?;

            let completion_options = if request.heal {
                CompletionOptions {
                    mode: CompletionMode::HealedJson,
                }
            } else {
                CompletionOptions::default()
            };

            let outcome = self
                .client
                .complete(&completion_request, completion_options)
                .await
                .map_err(|error| error.to_string())?;

            match outcome {
                CompletionOutcome::Stream(mut stream) => {
                    let mut aggregated = String::new();
                    while let Some(chunk_result) = stream.next().await {
                        let chunk = chunk_result.map_err(|error| error.to_string())?;
                        if let Some(choice) = chunk.choices.first() {
                            if let Some(delta) = choice.delta.content.clone() {
                                aggregated.push_str(delta.as_str());
                                if let Some(sink) = event_sink {
                                    sink.emit(&YamlWorkflowEvent {
                                        event_type: "node_stream_delta".to_string(),
                                        node_id: Some(request.node_id.clone()),
                                        node_kind: Some("llm_call".to_string()),
                                        streamable: Some(true),
                                        message: None,
                                        delta: Some(delta),
                                        elapsed_ms: None,
                                    });
                                }
                            }
                        }
                    }

                    serde_json::from_str(aggregated.as_str()).map_err(|error| {
                        format!(
                            "failed to parse streamed structured completion JSON: {error}; body={aggregated}"
                        )
                    })
                }
                CompletionOutcome::Response(response) => {
                    let content = response
                        .content()
                        .ok_or_else(|| "completion returned empty content".to_string())?;
                    serde_json::from_str(content).map_err(|error| {
                        format!("failed to parse structured completion JSON: {error}")
                    })
                }
                CompletionOutcome::HealedJson(healed) => {
                    if let Some(sink) = event_sink {
                        sink.emit(&YamlWorkflowEvent {
                            event_type: "node_healed".to_string(),
                            node_id: Some(request.node_id.clone()),
                            node_kind: Some("llm_call".to_string()),
                            streamable: Some(false),
                            message: Some(format!(
                                "healed structured response confidence={}",
                                healed.parsed.confidence
                            )),
                            delta: None,
                            elapsed_ms: None,
                        });
                    }
                    Ok(healed.parsed.value)
                }
                CompletionOutcome::CoercedSchema(coerced) => Ok(coerced.coerced.value),
            }
        }
    }

    let executor = BorrowedClientExecutor { client };
    run_email_workflow_yaml_with_custom_worker_and_events(
        workflow,
        email_text,
        &executor,
        custom_worker,
        event_sink,
    )
    .await
}

pub async fn run_email_workflow_yaml(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_email_workflow_yaml_with_custom_worker_and_events(
        workflow, email_text, executor, None, None,
    )
    .await
}

pub async fn run_email_workflow_yaml_with_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_email_workflow_yaml_with_custom_worker_and_events(
        workflow,
        email_text,
        executor,
        custom_worker,
        None,
    )
    .await
}

pub async fn run_email_workflow_yaml_with_custom_worker_and_events(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
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
    let started = Instant::now();

    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "workflow_started".to_string(),
            node_id: None,
            node_kind: None,
            streamable: None,
            message: Some(format!("workflow_id={}", workflow.id)),
            delta: None,
            elapsed_ms: Some(0),
        });
    }

    loop {
        let node =
            *node_map
                .get(current.as_str())
                .ok_or_else(|| YamlWorkflowRunError::MissingNode {
                    node_id: current.clone(),
                })?;

        trace.push(node.id.clone());
        let step_started = Instant::now();

        let node_streamable = node
            .node_type
            .llm_call
            .as_ref()
            .map(|llm| llm.stream.unwrap_or(false) && !llm.heal.unwrap_or(false));

        if let Some(sink) = event_sink {
            sink.emit(&YamlWorkflowEvent {
                event_type: "node_started".to_string(),
                node_id: Some(node.id.clone()),
                node_kind: Some(node.kind_name().to_string()),
                streamable: node_streamable,
                message: if node_streamable == Some(false) {
                    Some("Node is not streamable; status events only".to_string())
                } else {
                    None
                },
                delta: None,
                elapsed_ms: Some(started.elapsed().as_millis()),
            });
        }

        let next = if let Some(llm) = &node.node_type.llm_call {
            let prompt_template = node
                .config
                .as_ref()
                .and_then(|cfg| cfg.prompt.as_deref())
                .unwrap_or_default();
            let context = json!({
                "input": { "email_text": email_text },
                "nodes": outputs,
                "globals": Value::Object(globals.clone())
            });
            let prompt = interpolate_template(prompt_template, &context);
            let schema = schema_for_node(node.id.as_str()).ok_or_else(|| {
                YamlWorkflowRunError::MissingLlmSchema {
                    node_id: node.id.clone(),
                }
            })?;

            let request = YamlLlmExecutionRequest {
                node_id: node.id.clone(),
                model: llm.model.clone(),
                prompt,
                schema,
                stream: llm.stream.unwrap_or(false),
                heal: llm.heal.unwrap_or(false),
            };

            let payload = executor
                .complete_structured(request, event_sink)
                .await
                .map_err(|message| YamlWorkflowRunError::Llm {
                    node_id: node.id.clone(),
                    message,
                })?;

            if !payload.is_object() {
                return Err(YamlWorkflowRunError::LlmPayloadNotObject {
                    node_id: node.id.clone(),
                });
            }

            outputs.insert(node.id.clone(), json!({ "output": payload }));
            apply_set_globals(node, &outputs, email_text, &mut globals);
            edge_map
                .get(node.id.as_str())
                .map(|value| value.to_string())
        } else if let Some(switch) = &node.node_type.switch {
            let context = json!({
                "input": { "email_text": email_text },
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
                "input": { "email_text": email_text },
                "nodes": outputs,
                "globals": Value::Object(globals.clone())
            });

            let worker_output = if let Some(custom_worker_executor) = custom_worker {
                custom_worker_executor
                    .execute(custom.handler.as_str(), &payload, email_text, &context)
                    .await
                    .map_err(|message| YamlWorkflowRunError::CustomWorker {
                        node_id: node.id.clone(),
                        message,
                    })?
            } else {
                if custom.handler != "GetRagData" {
                    return Err(YamlWorkflowRunError::UnsupportedCustomHandler {
                        handler: custom.handler.clone(),
                    });
                }

                let topic = payload
                    .get("topic")
                    .and_then(Value::as_str)
                    .unwrap_or("clarification");
                mock_rag(topic)
            };

            outputs.insert(node.id.clone(), json!({ "output": worker_output }));
            apply_set_globals(node, &outputs, email_text, &mut globals);
            edge_map.get(node.id.as_str()).map(|value| value.to_string())
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
            elapsed_ms,
        });

        if let Some(sink) = event_sink {
            sink.emit(&YamlWorkflowEvent {
                event_type: "node_completed".to_string(),
                node_id: Some(node.id.clone()),
                node_kind: Some(node.kind_name().to_string()),
                streamable: node_streamable,
                message: None,
                delta: None,
                elapsed_ms: Some(elapsed_ms),
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

    let output = YamlWorkflowRunOutput {
        workflow_id: workflow.id.clone(),
        entry_node: workflow.entry_node.clone(),
        email_text: email_text.to_string(),
        trace,
        outputs,
        terminal_node,
        terminal_output,
        step_timings,
        total_elapsed_ms: started.elapsed().as_millis(),
    };

    if let Some(sink) = event_sink {
        sink.emit(&YamlWorkflowEvent {
            event_type: "workflow_completed".to_string(),
            node_id: None,
            node_kind: None,
            streamable: None,
            message: Some(format!("terminal_node={}", output.terminal_node)),
            delta: None,
            elapsed_ms: Some(output.total_elapsed_ms),
        });
    }

    Ok(output)
}

fn evaluate_switch_condition(
    condition: &str,
    context: &Value,
) -> Result<bool, YamlWorkflowRunError> {
    let (left, right) =
        condition
            .split_once("==")
            .ok_or_else(|| YamlWorkflowRunError::UnsupportedCondition {
                condition: condition.to_string(),
            })?;

    let left_path = left.trim().trim_start_matches("$.");
    let right_literal = right.trim().trim_matches('"').trim_matches('\'');
    let left_value = resolve_path(context, left_path);
    Ok(left_value
        .and_then(Value::as_str)
        .map(|value| value == right_literal)
        .unwrap_or(false))
}

pub fn verify_yaml_workflow(workflow: &YamlWorkflow) -> Vec<YamlWorkflowDiagnostic> {
    let mut diagnostics = Vec::new();
    let known_ids: HashMap<&str, &YamlNode> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    if !known_ids.contains_key(workflow.entry_node.as_str()) {
        diagnostics.push(YamlWorkflowDiagnostic {
            node_id: None,
            code: "missing_entry".to_string(),
            severity: YamlWorkflowDiagnosticSeverity::Error,
            message: format!("entry node '{}' does not exist", workflow.entry_node),
        });
    }

    for edge in &workflow.edges {
        if !known_ids.contains_key(edge.from.as_str()) {
            diagnostics.push(YamlWorkflowDiagnostic {
                node_id: Some(edge.from.clone()),
                code: "unknown_edge_from".to_string(),
                severity: YamlWorkflowDiagnosticSeverity::Error,
                message: format!("edge.from '{}' does not exist", edge.from),
            });
        }
        if !known_ids.contains_key(edge.to.as_str()) {
            diagnostics.push(YamlWorkflowDiagnostic {
                node_id: Some(edge.to.clone()),
                code: "unknown_edge_to".to_string(),
                severity: YamlWorkflowDiagnosticSeverity::Error,
                message: format!("edge.to '{}' does not exist", edge.to),
            });
        }
    }

    for node in &workflow.nodes {
        if let Some(llm) = &node.node_type.llm_call {
            if llm.model.trim().is_empty() {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "empty_model".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Error,
                    message: "llm_call.model must not be empty".to_string(),
                });
            }
            if llm.stream.unwrap_or(false) && llm.heal.unwrap_or(false) {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "stream_heal_conflict".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Warning,
                    message:
                        "llm_call.stream=true with heal=true is not streamable; runtime will disable streaming"
                            .to_string(),
                });
            }
        }

        if let Some(switch) = &node.node_type.switch {
            for branch in &switch.branches {
                if !known_ids.contains_key(branch.target.as_str()) {
                    diagnostics.push(YamlWorkflowDiagnostic {
                        node_id: Some(node.id.clone()),
                        code: "unknown_switch_target".to_string(),
                        severity: YamlWorkflowDiagnosticSeverity::Error,
                        message: format!("switch branch target '{}' does not exist", branch.target),
                    });
                }
            }
            if !known_ids.contains_key(switch.default.as_str()) {
                diagnostics.push(YamlWorkflowDiagnostic {
                    node_id: Some(node.id.clone()),
                    code: "unknown_switch_default".to_string(),
                    severity: YamlWorkflowDiagnosticSeverity::Error,
                    message: format!("switch default target '{}' does not exist", switch.default),
                });
            }
        }
    }

    diagnostics
}

fn resolve_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .try_fold(value, |current, segment| current.get(segment))
}

fn interpolate_template(template: &str, context: &Value) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    loop {
        let Some(start) = rest.find("{{") else {
            out.push_str(rest);
            break;
        };

        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            out.push_str(&rest[start..]);
            break;
        };

        let expr = after_start[..end].trim();
        let replacement = resolve_path(context, expr)
            .map(value_to_template_string)
            .unwrap_or_default();
        out.push_str(replacement.as_str());

        rest = &after_start[end + 2..];
    }

    out
}

fn value_to_template_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn apply_set_globals(
    node: &YamlNode,
    outputs: &BTreeMap<String, Value>,
    email_text: &str,
    globals: &mut serde_json::Map<String, Value>,
) {
    let Some(config) = node.config.as_ref() else {
        return;
    };
    let Some(set_globals) = config.set_globals.as_ref() else {
        return;
    };

    let context = json!({
        "input": { "email_text": email_text },
        "nodes": outputs,
        "globals": Value::Object(globals.clone())
    });

    for (key, expr) in set_globals {
        let value = resolve_path(&context, expr.as_str())
            .cloned()
            .unwrap_or(Value::Null);
        globals.insert(key.clone(), value);
    }
}

fn schema_for_node(node_id: &str) -> Option<Value> {
    if node_id == "classify_top_level" {
        return Some(json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "enum": [
                        "probation",
                        "termination",
                        "leave_request",
                        "supply_chain_request",
                        "clarification"
                    ]
                },
                "reason": { "type": "string" }
            },
            "required": ["category", "reason"],
            "additionalProperties": false
        }));
    }

    if node_id == "classify_supply_chain_subtype" {
        return Some(json!({
            "type": "object",
            "properties": {
                "subtype": {
                    "type": "string",
                    "enum": ["order_assessment", "order_replacement", "clarification"]
                },
                "reason": { "type": "string" }
            },
            "required": ["subtype", "reason"],
            "additionalProperties": false
        }));
    }

    if node_id == "classify_termination_subtype" {
        return Some(json!({
            "type": "object",
            "properties": {
                "subtype": {
                    "type": "string",
                    "enum": ["first_time_offense", "repeated_offense", "clarification"]
                },
                "reason": { "type": "string" }
            },
            "required": ["subtype", "reason"],
            "additionalProperties": false
        }));
    }

    if node_id == "generate_email_draft" {
        return Some(json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string" },
                "body": { "type": "string" }
            },
            "required": ["subject", "body"],
            "additionalProperties": false
        }));
    }

    None
}

fn mock_rag(topic: &str) -> Value {
    let (kb_source, playbook) = match topic {
        "probation" => (
            "hr_policy/probation.md",
            "Collect manager review, performance evidence, and probation timeline.",
        ),
        "leave_request" => (
            "hr_policy/leave.md",
            "Validate leave balance, manager approval, and blackout dates.",
        ),
        "supply_chain_order_assessment" => (
            "supply_chain/order_assessment.md",
            "Review order specs, inventory risk, and vendor lead-time guidance.",
        ),
        "supply_chain_order_replacement" => (
            "supply_chain/order_replacement.md",
            "Collect order id, damage proof, and replacement SLA policy.",
        ),
        "termination_first_time_offense" => (
            "hr_policy/termination_first_offense.md",
            "Validate first-incident criteria and route to HRBP review.",
        ),
        "termination_repeated_offense" => (
            "hr_policy/termination_repeated_offense.md",
            "Collect prior warnings and escalation approvals before final action.",
        ),
        _ => (
            "shared/request_clarification.md",
            "Request clarifying details before routing.",
        ),
    };

    json!({
        "kb_source": kb_source,
        "playbook": playbook,
    })
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
        } else {
            "unknown"
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNodeType {
    pub llm_call: Option<YamlLlmCall>,
    pub switch: Option<YamlSwitch>,
    pub custom_worker: Option<YamlCustomWorker>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlLlmCall {
    pub model: String,
    pub stream: Option<bool>,
    pub heal: Option<bool>,
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
pub struct YamlCustomWorker {
    pub handler: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlNodeConfig {
    pub prompt: Option<String>,
    pub payload: Option<Value>,
    pub set_globals: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YamlEdge {
    pub from: String,
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor;

    #[async_trait]
    impl YamlWorkflowLlmExecutor for MockExecutor {
        async fn complete_structured(
            &self,
            request: YamlLlmExecutionRequest,
            _event_sink: Option<&dyn YamlWorkflowEventSink>,
        ) -> Result<Value, String> {
            let prompt = request.prompt;
            if prompt.contains("exactly one category") {
                return Ok(json!({"category":"termination","reason":"mock"}));
            }
            if prompt.contains("Determine termination subtype") {
                return Ok(json!({"subtype":"repeated_offense","reason":"mock"}));
            }
            if prompt.contains("Determine supply chain subtype") {
                return Ok(json!({"subtype":"order_replacement","reason":"mock"}));
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
        let output = run_email_workflow_yaml(&workflow, "test", &MockExecutor)
            .await
            .expect("yaml workflow should execute");

        assert_eq!(output.workflow_id, "email-intake-classification");
        assert_eq!(output.terminal_node, "rag_termination_repeated_offense");
        assert!(!output.step_timings.is_empty());
        assert_eq!(output.step_timings.len(), output.trace.len());
        assert!(output
            .outputs
            .contains_key("rag_termination_repeated_offense"));
    }
}
