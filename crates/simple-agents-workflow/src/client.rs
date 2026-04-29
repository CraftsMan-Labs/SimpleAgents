//! [`WorkflowClient`] — wraps [`SimpleAgentsClient`] and adds workflow `run`, `stream`, `resume`.
//!
//! Because `simple-agents-core` cannot depend on `simple-agents-workflow` (that would create a
//! circular dependency), the workflow methods live here instead of on `SimpleAgentsClient`.
//!
//! # Usage
//!
//! ```no_run
//! use simple_agents_workflow::{WorkflowClient, WorkflowRunOptions, DefaultEventPrinter};
//! use simple_agents_core::SimpleAgentsClient;
//! use simple_agent_type::prelude::*;
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct MyProvider;
//!
//! #[async_trait]
//! impl Provider for MyProvider {
//!     fn name(&self) -> &str { "mock" }
//!     fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
//!         Ok(ProviderRequest::new("http://example.com"))
//!     }
//!     async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
//!         Ok(ProviderResponse::new(200, serde_json::json!({})))
//!     }
//!     fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
//!         unimplemented!()
//!     }
//! }
//!
//! # async fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! let client = WorkflowClient::from_client(
//!     SimpleAgentsClient::new(Arc::new(MyProvider)),
//! );
//!
//! let messages = vec![Message::user("Hello")];
//!
//! // Run a workflow
//! let output = client.run("workflow.yaml", messages.clone(), WorkflowRunOptions::default()).await?;
//!
//! // Stream a workflow
//! let output = client.stream("workflow.yaml", messages, &DefaultEventPrinter, WorkflowRunOptions::default()).await?;
//! # Ok(())
//! # }
//! ```

use serde_json::Value;
use simple_agent_type::message::Message;
use simple_agents_core::{CompletionOptions, CompletionOutcome, SimpleAgentsClient};

use crate::yaml_runner::{
    workflow_execution, RunMetadata, StepTiming, WorkflowCheckpoint, WorkflowEventSink,
    WorkflowRunOutput,
    YamlWorkflowEventSink, YamlWorkflowExecutionFlags, YamlWorkflowExecutionRequest,
    YamlWorkflowExecutorBinding, YamlWorkflowRunOptions, YamlWorkflowSource,
};

use simple_agent_type::prelude::SimpleAgentsError;

/// Error type for workflow operations.
#[derive(Debug)]
pub enum WorkflowError {
    /// Workflow-level run error.
    Workflow(String),
    /// Core LLM error.
    Core(SimpleAgentsError),
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::Workflow(msg) => write!(f, "workflow error: {msg}"),
            WorkflowError::Core(err) => write!(f, "core error: {err}"),
        }
    }
}

impl std::error::Error for WorkflowError {}

impl From<SimpleAgentsError> for WorkflowError {
    fn from(e: SimpleAgentsError) -> Self {
        WorkflowError::Core(e)
    }
}

/// Execution options for a workflow run.
///
/// Tools are passed via `tool_executor` — an optional callback that handles tool calls
/// declared in the workflow YAML. This mirrors the OpenAI SDK pattern where tools are
/// just another optional parameter on the same call.
#[derive(Clone, Default)]
pub struct RunOptions {
    /// Override the `YamlWorkflowRunOptions` (telemetry, trace, model override).
    pub workflow_options: YamlWorkflowRunOptions,
    /// Override execution flags (streaming, healing, etc.).
    pub execution_flags: YamlWorkflowExecutionFlags,
}

/// A client that wraps [`SimpleAgentsClient`] and exposes workflow operations.
///
/// The workflow methods (`run`, `stream`, `resume`) live here rather than on
/// `SimpleAgentsClient` because `simple-agents-core` cannot depend on
/// `simple-agents-workflow` without creating a circular crate dependency.
pub struct WorkflowClient {
    inner: SimpleAgentsClient,
}

impl WorkflowClient {
    /// Create a `WorkflowClient` from an already-built `SimpleAgentsClient`.
    pub fn from_client(client: SimpleAgentsClient) -> Self {
        Self { inner: client }
    }

    /// Expose the underlying `SimpleAgentsClient` for direct LLM calls.
    pub fn client(&self) -> &SimpleAgentsClient {
        &self.inner
    }

    /// Run a YAML workflow file and return the full output.
    ///
    /// Tools are configured in the workflow YAML itself; runtime tool executors
    /// (Python `custom_worker` / Node handler callbacks) are currently wired
    /// through the binding layer.
    pub async fn run(
        &self,
        workflow_path: &str,
        messages: Vec<Message>,
        options: RunOptions,
    ) -> Result<WorkflowRunOutput, WorkflowError> {
        let input = messages_to_value(messages);
        let request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(std::path::Path::new(workflow_path)),
            workflow_input: &input,
            executor: YamlWorkflowExecutorBinding::Client(&self.inner),
            custom_worker: None,
            options: &options.workflow_options,
            flags: options.execution_flags,
        };

        let output = workflow_execution::run(request)
            .await
            .map_err(|e| WorkflowError::Workflow(e.to_string()))?;

        // Map YamlWorkflowRunOutput to the clean WorkflowRunOutput
        Ok(yaml_output_to_workflow_output(output))
    }

    /// Stream a YAML workflow file, emitting events to `sink`.
    ///
    /// Pass `&DefaultEventPrinter` to get tokens printed to stdout by default.
    pub async fn stream(
        &self,
        workflow_path: &str,
        messages: Vec<Message>,
        sink: &dyn WorkflowEventSink,
        options: RunOptions,
    ) -> Result<WorkflowRunOutput, WorkflowError> {
        let input = messages_to_value(messages);
        let mut flags = options.execution_flags;
        flags.workflow_streaming = true;

        let request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(std::path::Path::new(workflow_path)),
            workflow_input: &input,
            executor: YamlWorkflowExecutorBinding::Client(&self.inner),
            custom_worker: None,
            options: &options.workflow_options,
            flags,
        };

        // Bridge the typed WorkflowEventSink to YamlWorkflowEventSink
        let bridge = EventSinkBridge { inner: sink };
        let output = workflow_execution::stream(request, &bridge)
            .await
            .map_err(|e| WorkflowError::Workflow(e.to_string()))?;

        Ok(yaml_output_to_workflow_output(output))
    }

    /// Resume a workflow from a saved checkpoint.
    pub async fn resume(
        &self,
        checkpoint: WorkflowCheckpoint,
        options: RunOptions,
    ) -> Result<WorkflowRunOutput, WorkflowError> {
        // Reconstruct a messages-based input from the checkpoint
        let messages = checkpoint.original_messages.clone();
        self.run(&checkpoint.workflow_path, messages, options).await
    }

    /// Direct LLM completion (delegates to the inner client).
    pub async fn complete(
        &self,
        request: &simple_agent_type::request::CompletionRequest,
        options: CompletionOptions,
    ) -> Result<CompletionOutcome, SimpleAgentsError> {
        self.inner.complete(request, options).await
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert a list of messages into the `{"messages": [...]}` Value shape the
/// YAML executor expects as workflow input.
fn messages_to_value(messages: Vec<Message>) -> Value {
    let msgs: Vec<Value> = messages
        .into_iter()
        .map(|m| serde_json::to_value(&m).unwrap_or(Value::Null))
        .collect();
    serde_json::json!({ "messages": msgs })
}

/// Bridge from the new typed [`WorkflowEventSink`] to the old
/// [`YamlWorkflowEventSink`] that the executor uses internally.
struct EventSinkBridge<'a> {
    inner: &'a dyn WorkflowEventSink,
}

impl YamlWorkflowEventSink for EventSinkBridge<'_> {
    fn emit(&self, event: &crate::yaml_runner::YamlWorkflowEvent) {
        // Map YamlWorkflowEvent → WorkflowEvent
        use crate::yaml_runner::{TokenKind, WorkflowEvent};
        let mapped = match event.event_type.as_str() {
            "workflow_started" => Some(WorkflowEvent::WorkflowStarted {
                workflow_id: event.node_id.clone().unwrap_or_default(),
            }),
            "node_started" => Some(WorkflowEvent::NodeStarted {
                node_id: event.node_id.clone().unwrap_or_default(),
                node_type: node_type_from_kind(event.node_kind.as_deref()),
            }),
            "node_completed" => Some(WorkflowEvent::NodeCompleted {
                node_id: event.node_id.clone().unwrap_or_default(),
                output: event
                    .snapshot
                    .clone()
                    .or_else(|| event.message.clone().map(Value::String))
                    .unwrap_or(Value::Null),
            }),
            "node_tool_call_requested" => tool_name_from_event(event).map(|tool_name| {
                WorkflowEvent::ToolCallRequested {
                    node_id: event.node_id.clone().unwrap_or_default(),
                    tool_name,
                    arguments: event
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.get("arguments"))
                        .cloned()
                        .unwrap_or(Value::Null),
                }
            }),
            "node_tool_call_completed" => tool_name_from_event(event).map(|tool_name| {
                WorkflowEvent::ToolCallCompleted {
                    node_id: event.node_id.clone().unwrap_or_default(),
                    tool_name,
                    output: event
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.get("output"))
                        .cloned()
                        .unwrap_or(Value::Null),
                }
            }),
            "node_tool_call_failed" => Some(WorkflowEvent::NodeFailed {
                node_id: event.node_id.clone().unwrap_or_default(),
                error: event.message.clone().unwrap_or_else(|| {
                    "tool call failed without an error message".to_string()
                }),
            }),
            "node_stream_delta" | "node_stream_output_delta" => {
                Some(WorkflowEvent::LlmTokenDelta {
                    node_id: event.node_id.clone().unwrap_or_default(),
                    token: event.delta.clone().unwrap_or_default(),
                    token_kind: TokenKind::Output,
                })
            }
            "node_stream_thinking_delta" => Some(WorkflowEvent::LlmTokenDelta {
                node_id: event.node_id.clone().unwrap_or_default(),
                token: event.delta.clone().unwrap_or_default(),
                token_kind: TokenKind::Reasoning,
            }),
            "workflow_completed" => Some(WorkflowEvent::WorkflowCompleted {
                output: event.snapshot.clone().unwrap_or(Value::Null),
                metadata: event.metadata.clone(),
            }),
            _ => None,
        };
        if let Some(ev) = mapped {
            self.inner.emit(&ev);
        }
    }
}

fn node_type_from_kind(kind: Option<&str>) -> crate::yaml_runner::NodeType {
    use crate::yaml_runner::NodeType;

    match kind {
        Some("llm_call") => NodeType::LlmCall,
        Some("switch") => NodeType::Switch,
        Some("custom_worker") => NodeType::CustomWorker,
        Some("end") => NodeType::End,
        _ => NodeType::Unknown,
    }
}

fn tool_name_from_event(event: &crate::yaml_runner::YamlWorkflowEvent) -> Option<String> {
    event
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("tool_name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Map the internal `YamlWorkflowRunOutput` to the cleaner public `WorkflowRunOutput`.
fn yaml_output_to_workflow_output(
    output: crate::yaml_runner::YamlWorkflowRunOutput,
) -> WorkflowRunOutput {
    let metadata = Some(RunMetadata {
        total_elapsed_ms: output.total_elapsed_ms,
        ttft_ms: output.ttft_ms,
        total_input_tokens: output.total_input_tokens,
        total_output_tokens: output.total_output_tokens,
        total_tokens: output.total_tokens,
        total_reasoning_tokens: output.total_reasoning_tokens,
        tokens_per_second: output.tokens_per_second,
        step_details: output
            .step_timings
            .iter()
            .map(|step| StepTiming {
                node_id: step.node_id.clone(),
                node_type: step.node_kind.clone(),
                model: step.model_name.clone(),
                elapsed_ms: step.elapsed_ms,
                input_tokens: step.prompt_tokens.map(u64::from),
                output_tokens: step.completion_tokens.map(u64::from),
                total_tokens: step.total_tokens.map(u64::from),
                reasoning_tokens: step.reasoning_tokens.map(u64::from),
                ttft_ms: None,
            })
            .collect(),
        trace_id: output.trace_id.clone(),
    });

    WorkflowRunOutput {
        workflow_id: output.workflow_id,
        entry_node: output.entry_node,
        trace: output.trace,
        outputs: output.outputs,
        terminal_node: output.terminal_node,
        terminal_output: output.terminal_output,
        metadata,
        events: None,
    }
}
