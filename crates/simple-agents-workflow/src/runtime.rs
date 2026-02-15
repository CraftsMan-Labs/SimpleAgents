use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Map, Number, Value};
use simple_agent_type::message::Message;
use simple_agent_type::request::CompletionRequest;
use simple_agents_core::{CompletionOptions, CompletionOutcome, SimpleAgentsClient};
use thiserror::Error;
use tokio::time::timeout;

use crate::ir::{Node, NodeKind, WorkflowDefinition};
use crate::recorder::{TraceRecordError, TraceRecorder};
use crate::replay::{replay_trace, ReplayError, ReplayReport};
use crate::trace::{TraceTerminalStatus, WorkflowTrace, WorkflowTraceMetadata};
use crate::validation::{validate_and_normalize, ValidationErrors};

/// Runtime configuration for workflow execution.
#[derive(Debug, Clone)]
pub struct WorkflowRuntimeOptions {
    /// Maximum number of node steps before aborting execution.
    pub max_steps: usize,
    /// Validate and normalize workflow before every run.
    pub validate_before_run: bool,
    /// Retry and timeout policy for LLM nodes.
    pub llm_node_policy: NodeExecutionPolicy,
    /// Retry and timeout policy for tool nodes.
    pub tool_node_policy: NodeExecutionPolicy,
    /// Enable deterministic trace recording for runtime events.
    pub enable_trace_recording: bool,
    /// Optional replay validation mode for recorded traces.
    pub replay_mode: WorkflowReplayMode,
}

/// Runtime replay behavior for deterministic runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowReplayMode {
    /// Trace replay validation is disabled.
    Disabled,
    /// Validate the recorded trace via `replay_trace` before success return.
    ValidateRecordedTrace,
}

/// Retry and timeout policy for runtime-owned node execution.
#[derive(Debug, Clone)]
pub struct NodeExecutionPolicy {
    /// Per-attempt timeout. `None` disables timeout enforcement.
    pub timeout: Option<Duration>,
    /// Number of retries after the first failed attempt.
    pub max_retries: usize,
}

impl Default for WorkflowRuntimeOptions {
    fn default() -> Self {
        Self {
            max_steps: 256,
            validate_before_run: true,
            llm_node_policy: NodeExecutionPolicy::default(),
            tool_node_policy: NodeExecutionPolicy::default(),
            enable_trace_recording: true,
            replay_mode: WorkflowReplayMode::Disabled,
        }
    }
}

impl Default for NodeExecutionPolicy {
    fn default() -> Self {
        Self {
            timeout: None,
            max_retries: 0,
        }
    }
}

/// Cooperative cancellation interface for workflow runs.
pub trait CancellationSignal: Send + Sync {
    /// Returns true if execution should stop.
    fn is_cancelled(&self) -> bool;
}

impl CancellationSignal for AtomicBool {
    fn is_cancelled(&self) -> bool {
        self.load(Ordering::Relaxed)
    }
}

impl CancellationSignal for bool {
    fn is_cancelled(&self) -> bool {
        *self
    }
}

/// Input payload passed to the LLM executor adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmExecutionInput {
    /// Current workflow node id.
    pub node_id: String,
    /// Requested model.
    pub model: String,
    /// Prompt configured on the workflow node.
    pub prompt: String,
    /// Deterministic scoped context for this node execution.
    pub scoped_input: Value,
}

/// LLM output returned to the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmExecutionOutput {
    /// Assistant text returned by the model.
    pub content: String,
}

/// Typed LLM adapter errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LlmExecutionError {
    /// CompletionRequest build/validation failure.
    #[error("invalid completion request: {0}")]
    InvalidRequest(String),
    /// Core client call failed.
    #[error("llm client execution failed: {0}")]
    Client(String),
    /// Unexpected completion mode returned by client.
    #[error("unexpected completion outcome: {0}")]
    UnexpectedOutcome(&'static str),
    /// Response did not include assistant content.
    #[error("llm response had no content")]
    EmptyResponse,
}

/// Async runtime adapter for LLM calls.
#[async_trait]
pub trait LlmExecutor: Send + Sync {
    /// Executes one workflow LLM node.
    async fn execute(
        &self,
        input: LlmExecutionInput,
    ) -> Result<LlmExecutionOutput, LlmExecutionError>;
}

#[async_trait]
impl LlmExecutor for SimpleAgentsClient {
    async fn execute(
        &self,
        input: LlmExecutionInput,
    ) -> Result<LlmExecutionOutput, LlmExecutionError> {
        let user_prompt = build_prompt_with_scope(&input.prompt, &input.scoped_input);
        let request = CompletionRequest::builder()
            .model(input.model)
            .message(Message::user(user_prompt))
            .build()
            .map_err(|error| LlmExecutionError::InvalidRequest(error.to_string()))?;

        let outcome = self
            .complete(&request, CompletionOptions::default())
            .await
            .map_err(|error| LlmExecutionError::Client(error.to_string()))?;

        let response = match outcome {
            CompletionOutcome::Response(response) => response,
            CompletionOutcome::Stream(_) => {
                return Err(LlmExecutionError::UnexpectedOutcome("stream"));
            }
            CompletionOutcome::HealedJson(_) => {
                return Err(LlmExecutionError::UnexpectedOutcome("healed_json"));
            }
            CompletionOutcome::CoercedSchema(_) => {
                return Err(LlmExecutionError::UnexpectedOutcome("coerced_schema"));
            }
        };

        let content = response
            .content()
            .ok_or(LlmExecutionError::EmptyResponse)?
            .to_string();

        Ok(LlmExecutionOutput { content })
    }
}

/// Input payload for host-provided tool execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecutionInput {
    /// Current workflow node id.
    pub node_id: String,
    /// Tool name declared by the workflow.
    pub tool: String,
    /// Static node input payload.
    pub input: Value,
    /// Deterministic scoped context for this node execution.
    pub scoped_input: Value,
}

/// Typed tool execution errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ToolExecutionError {
    /// Tool implementation is unavailable in the host runtime.
    #[error("tool handler not found: {tool}")]
    NotFound {
        /// Missing tool name.
        tool: String,
    },
    /// Tool returned an execution failure.
    #[error("tool execution failed: {0}")]
    Failed(String),
}

/// Async host tool executor surface.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Executes one workflow tool node.
    async fn execute_tool(&self, input: ToolExecutionInput) -> Result<Value, ToolExecutionError>;
}

/// Node-level execution result.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeExecution {
    /// Zero-based step index.
    pub step: usize,
    /// Executed node id.
    pub node_id: String,
    /// Structured execution payload.
    pub data: NodeExecutionData,
}

/// Structured result data emitted for each node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeExecutionData {
    /// Start node transition.
    Start {
        /// Next node id.
        next: String,
    },
    /// LLM node output.
    Llm {
        /// Model used.
        model: String,
        /// Assistant output text.
        output: String,
        /// Next node id.
        next: String,
    },
    /// Tool node output.
    Tool {
        /// Tool name.
        tool: String,
        /// Tool output JSON payload.
        output: Value,
        /// Next node id.
        next: String,
    },
    /// Condition node decision.
    Condition {
        /// Original expression.
        expression: String,
        /// Evaluated bool.
        evaluated: bool,
        /// Chosen next node id.
        next: String,
    },
    /// End node reached.
    End,
}

/// Runtime event stream payload for trace consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowEvent {
    /// Zero-based step index.
    pub step: usize,
    /// Node id associated with this event.
    pub node_id: String,
    /// Event payload.
    pub kind: WorkflowEventKind,
}

/// Event kinds emitted by the runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowEventKind {
    /// Node execution started.
    NodeStarted,
    /// Node execution completed successfully.
    NodeCompleted {
        /// Node execution payload.
        data: NodeExecutionData,
    },
    /// Node execution failed.
    NodeFailed {
        /// Error message.
        message: String,
    },
}

/// Final workflow execution report.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunResult {
    /// Workflow name.
    pub workflow_name: String,
    /// Final terminal node id.
    pub terminal_node_id: String,
    /// Ordered node execution records.
    pub node_executions: Vec<NodeExecution>,
    /// Ordered runtime events.
    pub events: Vec<WorkflowEvent>,
    /// Node output map keyed by node id.
    pub node_outputs: BTreeMap<String, Value>,
    /// Optional deterministic trace captured during this run.
    pub trace: Option<WorkflowTrace>,
    /// Replay validation report when replay mode is enabled.
    pub replay_report: Option<ReplayReport>,
}

/// Scope access capabilities used by runtime nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeCapability {
    LlmRead,
    ToolRead,
    ConditionRead,
    LlmWrite,
    ToolWrite,
    ConditionWrite,
}

impl ScopeCapability {
    fn as_str(self) -> &'static str {
        match self {
            Self::LlmRead => "llm_read",
            Self::ToolRead => "tool_read",
            Self::ConditionRead => "condition_read",
            Self::LlmWrite => "llm_write",
            Self::ToolWrite => "tool_write",
            Self::ConditionWrite => "condition_write",
        }
    }
}

/// Typed read/write boundary failures for scoped runtime state.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScopeAccessError {
    /// Node attempted to read scope with an invalid capability.
    #[error("scope read denied for capability '{capability}'")]
    ReadDenied {
        /// Capability used for the read.
        capability: &'static str,
    },
    /// Node attempted to write scope with an invalid capability.
    #[error("scope write denied for capability '{capability}'")]
    WriteDenied {
        /// Capability used for the write.
        capability: &'static str,
    },
}

/// Runtime failures.
#[derive(Debug, Error)]
pub enum WorkflowRuntimeError {
    /// Workflow failed structural validation.
    #[error(transparent)]
    Validation(#[from] ValidationErrors),
    /// Workflow has no start node.
    #[error("workflow has no start node")]
    MissingStartNode,
    /// Current node id not found in node index.
    #[error("node not found: {node_id}")]
    NodeNotFound {
        /// Missing node id.
        node_id: String,
    },
    /// Loop protection guard triggered.
    #[error("workflow exceeded max step limit ({max_steps})")]
    StepLimitExceeded {
        /// Configured max steps.
        max_steps: usize,
    },
    /// Execution cancelled by caller.
    #[error("workflow execution cancelled")]
    Cancelled,
    /// LLM node failed.
    #[error("llm node '{node_id}' failed: {source}")]
    Llm {
        /// Failing node id.
        node_id: String,
        /// Source error.
        source: LlmExecutionError,
    },
    /// Tool node failed.
    #[error("tool node '{node_id}' failed: {source}")]
    Tool {
        /// Failing node id.
        node_id: String,
        /// Source error.
        source: ToolExecutionError,
    },
    /// LLM node exhausted all retry attempts.
    #[error("llm node '{node_id}' exhausted {attempts} attempt(s): {last_error}")]
    LlmRetryExhausted {
        /// Failing node id.
        node_id: String,
        /// Number of attempts made.
        attempts: usize,
        /// Last attempt error.
        last_error: LlmExecutionError,
    },
    /// Tool node exhausted all retry attempts.
    #[error("tool node '{node_id}' exhausted {attempts} attempt(s): {last_error}")]
    ToolRetryExhausted {
        /// Failing node id.
        node_id: String,
        /// Number of attempts made.
        attempts: usize,
        /// Last attempt error.
        last_error: ToolExecutionError,
    },
    /// LLM node timed out across all attempts.
    #[error(
        "llm node '{node_id}' timed out after {attempts} attempt(s) (timeout: {timeout_ms} ms)"
    )]
    LlmTimeout {
        /// Failing node id.
        node_id: String,
        /// Per-attempt timeout in milliseconds.
        timeout_ms: u128,
        /// Number of attempts made.
        attempts: usize,
    },
    /// Tool node timed out across all attempts.
    #[error(
        "tool node '{node_id}' timed out after {attempts} attempt(s) (timeout: {timeout_ms} ms)"
    )]
    ToolTimeout {
        /// Failing node id.
        node_id: String,
        /// Per-attempt timeout in milliseconds.
        timeout_ms: u128,
        /// Number of attempts made.
        attempts: usize,
    },
    /// Tool node reached without an executor.
    #[error("tool node '{node_id}' requires a tool executor")]
    MissingToolExecutor {
        /// Failing node id.
        node_id: String,
    },
    /// LLM/tool node missing its `next` edge.
    #[error("node '{node_id}' is missing required next edge")]
    MissingNextEdge {
        /// Failing node id.
        node_id: String,
    },
    /// Condition expression could not be evaluated.
    #[error("condition node '{node_id}' has invalid expression '{expression}': {reason}")]
    InvalidCondition {
        /// Failing node id.
        node_id: String,
        /// Condition expression.
        expression: String,
        /// Detailed reason.
        reason: String,
    },
    /// Non-terminal node did not return a next transition.
    #[error("node '{node_id}' did not provide a next transition")]
    MissingNextTransition {
        /// Failing node id.
        node_id: String,
    },
    /// Scoped state boundary check failed.
    #[error("scope access failed on node '{node_id}': {source}")]
    ScopeAccess {
        /// Failing node id.
        node_id: String,
        /// Source access error.
        source: ScopeAccessError,
    },
    /// Trace recorder operation failed.
    #[error(transparent)]
    TraceRecording(#[from] TraceRecordError),
    /// Replay trace validation failed.
    #[error(transparent)]
    Replay(#[from] ReplayError),
    /// Replay mode requested without trace recording.
    #[error("replay validation requires trace recording to be enabled")]
    ReplayRequiresTraceRecording,
}

/// Deterministic minimal runtime for workflow execution.
pub struct WorkflowRuntime<'a> {
    definition: WorkflowDefinition,
    llm_executor: &'a dyn LlmExecutor,
    tool_executor: Option<&'a dyn ToolExecutor>,
    options: WorkflowRuntimeOptions,
}

impl<'a> WorkflowRuntime<'a> {
    /// Creates a runtime with host-provided LLM and tool adapters.
    pub fn new(
        definition: WorkflowDefinition,
        llm_executor: &'a dyn LlmExecutor,
        tool_executor: Option<&'a dyn ToolExecutor>,
        options: WorkflowRuntimeOptions,
    ) -> Self {
        Self {
            definition,
            llm_executor,
            tool_executor,
            options,
        }
    }

    /// Executes a workflow deterministically from `start` to `end`.
    pub async fn execute(
        &self,
        input: Value,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<WorkflowRunResult, WorkflowRuntimeError> {
        let workflow = if self.options.validate_before_run {
            validate_and_normalize(&self.definition)?
        } else {
            self.definition.normalized()
        };

        let node_index = build_node_index(&workflow);
        let start_id = find_start_node_id(&workflow)?;

        if matches!(
            self.options.replay_mode,
            WorkflowReplayMode::ValidateRecordedTrace
        ) && !self.options.enable_trace_recording
        {
            return Err(WorkflowRuntimeError::ReplayRequiresTraceRecording);
        }

        let trace_recorder = self.options.enable_trace_recording.then(|| {
            TraceRecorder::new(WorkflowTraceMetadata {
                trace_id: format!("{}-{}-trace", workflow.name, workflow.version),
                workflow_name: workflow.name.clone(),
                workflow_version: workflow.version.clone(),
                started_at_unix_ms: 0,
                finished_at_unix_ms: None,
            })
        });
        let mut trace_clock = 0u64;

        let mut scope = RuntimeScope::new(input);
        let mut events = Vec::new();
        let mut node_executions = Vec::new();
        let mut current_id = start_id;

        for step in 0..self.options.max_steps {
            check_cancelled(cancellation)?;

            let node = node_index.get(current_id.as_str()).ok_or_else(|| {
                WorkflowRuntimeError::NodeNotFound {
                    node_id: current_id.clone(),
                }
            })?;

            events.push(WorkflowEvent {
                step,
                node_id: current_id.clone(),
                kind: WorkflowEventKind::NodeStarted,
            });

            if let Some(recorder) = &trace_recorder {
                recorder.record_node_enter(next_trace_timestamp(&mut trace_clock), &current_id)?;
            }

            let execution_result = self
                .execute_node(node, step, &mut scope, cancellation)
                .await;
            let execution = match execution_result {
                Ok(execution) => execution,
                Err(error) => {
                    if let Some(recorder) = &trace_recorder {
                        recorder.record_node_error(
                            next_trace_timestamp(&mut trace_clock),
                            &current_id,
                            error.to_string(),
                        )?;
                        recorder.record_terminal(
                            next_trace_timestamp(&mut trace_clock),
                            TraceTerminalStatus::Failed,
                        )?;
                        let _ = recorder.finalize(next_trace_timestamp(&mut trace_clock))?;
                    }

                    events.push(WorkflowEvent {
                        step,
                        node_id: current_id,
                        kind: WorkflowEventKind::NodeFailed {
                            message: error.to_string(),
                        },
                    });
                    return Err(error);
                }
            };

            events.push(WorkflowEvent {
                step,
                node_id: execution.node_id.clone(),
                kind: WorkflowEventKind::NodeCompleted {
                    data: execution.data.clone(),
                },
            });

            if let Some(recorder) = &trace_recorder {
                recorder
                    .record_node_exit(next_trace_timestamp(&mut trace_clock), &execution.node_id)?;
            }

            let is_terminal = matches!(execution.data, NodeExecutionData::End);
            let next_node = next_node_id(&execution.data);
            let executed_node_id = execution.node_id.clone();

            node_executions.push(execution);

            if is_terminal {
                let (trace, replay_report) = if let Some(recorder) = &trace_recorder {
                    recorder.record_terminal(
                        next_trace_timestamp(&mut trace_clock),
                        TraceTerminalStatus::Completed,
                    )?;
                    let finalized_trace =
                        recorder.finalize(next_trace_timestamp(&mut trace_clock))?;
                    let replay_report = match self.options.replay_mode {
                        WorkflowReplayMode::Disabled => None,
                        WorkflowReplayMode::ValidateRecordedTrace => {
                            Some(replay_trace(&finalized_trace)?)
                        }
                    };
                    (Some(finalized_trace), replay_report)
                } else {
                    (None, None)
                };

                return Ok(WorkflowRunResult {
                    workflow_name: workflow.name,
                    terminal_node_id: executed_node_id,
                    node_executions,
                    events,
                    node_outputs: scope.node_outputs,
                    trace,
                    replay_report,
                });
            }

            current_id = next_node.ok_or_else(|| WorkflowRuntimeError::MissingNextTransition {
                node_id: executed_node_id,
            })?;
        }

        Err(WorkflowRuntimeError::StepLimitExceeded {
            max_steps: self.options.max_steps,
        })
    }

    async fn execute_node(
        &self,
        node: &Node,
        step: usize,
        scope: &mut RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        match &node.kind {
            NodeKind::Start { next } => Ok(NodeExecution {
                step,
                node_id: node.id.clone(),
                data: NodeExecutionData::Start { next: next.clone() },
            }),
            NodeKind::Llm {
                model,
                prompt,
                next,
            } => {
                let next_node =
                    next.clone()
                        .ok_or_else(|| WorkflowRuntimeError::MissingNextEdge {
                            node_id: node.id.clone(),
                        })?;

                let output = self
                    .execute_llm_with_policy(node, model, prompt, scope, cancellation)
                    .await?;

                scope
                    .record_llm_output(&node.id, output.content.clone(), ScopeCapability::LlmWrite)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Llm {
                        model: model.clone(),
                        output: output.content,
                        next: next_node,
                    },
                })
            }
            NodeKind::Tool { tool, input, next } => {
                let next_node =
                    next.clone()
                        .ok_or_else(|| WorkflowRuntimeError::MissingNextEdge {
                            node_id: node.id.clone(),
                        })?;

                let executor = self.tool_executor.ok_or_else(|| {
                    WorkflowRuntimeError::MissingToolExecutor {
                        node_id: node.id.clone(),
                    }
                })?;

                let tool_output = self
                    .execute_tool_with_policy(node, tool, input, executor, scope, cancellation)
                    .await?;

                scope
                    .record_tool_output(&node.id, tool_output.clone(), ScopeCapability::ToolWrite)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Tool {
                        tool: tool.clone(),
                        output: tool_output,
                        next: next_node,
                    },
                })
            }
            NodeKind::Condition {
                expression,
                on_true,
                on_false,
            } => {
                check_cancelled(cancellation)?;
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let evaluated =
                    evaluate_condition(expression, &scoped_input).map_err(|reason| {
                        WorkflowRuntimeError::InvalidCondition {
                            node_id: node.id.clone(),
                            expression: expression.clone(),
                            reason,
                        }
                    })?;
                let next = if evaluated {
                    on_true.clone()
                } else {
                    on_false.clone()
                };

                scope
                    .record_condition_output(&node.id, evaluated, ScopeCapability::ConditionWrite)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Condition {
                        expression: expression.clone(),
                        evaluated,
                        next,
                    },
                })
            }
            NodeKind::End => Ok(NodeExecution {
                step,
                node_id: node.id.clone(),
                data: NodeExecutionData::End,
            }),
        }
    }

    async fn execute_llm_with_policy(
        &self,
        node: &Node,
        model: &str,
        prompt: &str,
        scope: &RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<LlmExecutionOutput, WorkflowRuntimeError> {
        let max_attempts = self.options.llm_node_policy.max_retries.saturating_add(1);

        for attempt in 1..=max_attempts {
            check_cancelled(cancellation)?;

            let scoped_input = scope
                .scoped_input(ScopeCapability::LlmRead)
                .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                    node_id: node.id.clone(),
                    source,
                })?;

            let execution = self.llm_executor.execute(LlmExecutionInput {
                node_id: node.id.clone(),
                model: model.to_string(),
                prompt: prompt.to_string(),
                scoped_input,
            });

            let outcome = if let Some(timeout_duration) = self.options.llm_node_policy.timeout {
                match timeout(timeout_duration, execution).await {
                    Ok(result) => result,
                    Err(_) => {
                        if attempt == max_attempts {
                            return Err(WorkflowRuntimeError::LlmTimeout {
                                node_id: node.id.clone(),
                                timeout_ms: timeout_duration.as_millis(),
                                attempts: attempt,
                            });
                        }
                        check_cancelled(cancellation)?;
                        continue;
                    }
                }
            } else {
                execution.await
            };

            match outcome {
                Ok(output) => return Ok(output),
                Err(last_error) => {
                    if attempt == max_attempts {
                        return Err(WorkflowRuntimeError::LlmRetryExhausted {
                            node_id: node.id.clone(),
                            attempts: attempt,
                            last_error,
                        });
                    }
                    check_cancelled(cancellation)?;
                }
            }
        }

        unreachable!("llm attempts loop always returns")
    }

    async fn execute_tool_with_policy(
        &self,
        node: &Node,
        tool: &str,
        input: &Value,
        executor: &dyn ToolExecutor,
        scope: &RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<Value, WorkflowRuntimeError> {
        let max_attempts = self.options.tool_node_policy.max_retries.saturating_add(1);

        for attempt in 1..=max_attempts {
            check_cancelled(cancellation)?;

            let scoped_input = scope
                .scoped_input(ScopeCapability::ToolRead)
                .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                    node_id: node.id.clone(),
                    source,
                })?;

            let execution = executor.execute_tool(ToolExecutionInput {
                node_id: node.id.clone(),
                tool: tool.to_string(),
                input: input.clone(),
                scoped_input,
            });

            let outcome = if let Some(timeout_duration) = self.options.tool_node_policy.timeout {
                match timeout(timeout_duration, execution).await {
                    Ok(result) => result,
                    Err(_) => {
                        if attempt == max_attempts {
                            return Err(WorkflowRuntimeError::ToolTimeout {
                                node_id: node.id.clone(),
                                timeout_ms: timeout_duration.as_millis(),
                                attempts: attempt,
                            });
                        }
                        check_cancelled(cancellation)?;
                        continue;
                    }
                }
            } else {
                execution.await
            };

            match outcome {
                Ok(output) => return Ok(output),
                Err(last_error) => {
                    if attempt == max_attempts {
                        return Err(WorkflowRuntimeError::ToolRetryExhausted {
                            node_id: node.id.clone(),
                            attempts: attempt,
                            last_error,
                        });
                    }
                    check_cancelled(cancellation)?;
                }
            }
        }

        unreachable!("tool attempts loop always returns")
    }
}

#[derive(Debug)]
struct RuntimeScope {
    workflow_input: Value,
    node_outputs: BTreeMap<String, Value>,
    last_llm_output: Option<String>,
    last_tool_output: Option<Value>,
}

impl RuntimeScope {
    fn new(workflow_input: Value) -> Self {
        Self {
            workflow_input,
            node_outputs: BTreeMap::new(),
            last_llm_output: None,
            last_tool_output: None,
        }
    }

    fn scoped_input(&self, capability: ScopeCapability) -> Result<Value, ScopeAccessError> {
        if !matches!(
            capability,
            ScopeCapability::LlmRead | ScopeCapability::ToolRead | ScopeCapability::ConditionRead
        ) {
            return Err(ScopeAccessError::ReadDenied {
                capability: capability.as_str(),
            });
        }

        let mut object = Map::new();
        object.insert("input".to_string(), self.workflow_input.clone());
        object.insert(
            "last_llm_output".to_string(),
            self.last_llm_output
                .as_ref()
                .map_or(Value::Null, |value| Value::String(value.clone())),
        );
        object.insert(
            "last_tool_output".to_string(),
            self.last_tool_output.clone().unwrap_or(Value::Null),
        );
        object.insert(
            "node_outputs".to_string(),
            Value::Object(
                self.node_outputs
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ),
        );
        Ok(Value::Object(object))
    }

    fn record_llm_output(
        &mut self,
        node_id: &str,
        output: String,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if capability != ScopeCapability::LlmWrite {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.last_llm_output = Some(output.clone());
        self.node_outputs
            .insert(node_id.to_string(), Value::String(output));
        Ok(())
    }

    fn record_tool_output(
        &mut self,
        node_id: &str,
        output: Value,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if capability != ScopeCapability::ToolWrite {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.last_tool_output = Some(output.clone());
        self.node_outputs.insert(node_id.to_string(), output);
        Ok(())
    }

    fn record_condition_output(
        &mut self,
        node_id: &str,
        evaluated: bool,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if capability != ScopeCapability::ConditionWrite {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.node_outputs
            .insert(node_id.to_string(), Value::Bool(evaluated));
        Ok(())
    }
}

fn check_cancelled(
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<(), WorkflowRuntimeError> {
    if cancellation.is_some_and(CancellationSignal::is_cancelled) {
        Err(WorkflowRuntimeError::Cancelled)
    } else {
        Ok(())
    }
}

fn next_trace_timestamp(clock: &mut u64) -> u64 {
    let timestamp = *clock;
    *clock = clock.saturating_add(1);
    timestamp
}

fn build_prompt_with_scope(prompt: &str, scoped_input: &Value) -> String {
    format!("{}\n\nScoped context:\n{}", prompt, scoped_input)
}

fn build_node_index(workflow: &WorkflowDefinition) -> HashMap<&str, &Node> {
    let mut index = HashMap::with_capacity(workflow.nodes.len());
    for node in &workflow.nodes {
        index.insert(node.id.as_str(), node);
    }
    index
}

fn find_start_node_id(workflow: &WorkflowDefinition) -> Result<String, WorkflowRuntimeError> {
    workflow
        .nodes
        .iter()
        .find_map(|node| match node.kind {
            NodeKind::Start { .. } => Some(node.id.clone()),
            _ => None,
        })
        .ok_or(WorkflowRuntimeError::MissingStartNode)
}

fn next_node_id(data: &NodeExecutionData) -> Option<String> {
    match data {
        NodeExecutionData::Start { next }
        | NodeExecutionData::Llm { next, .. }
        | NodeExecutionData::Tool { next, .. }
        | NodeExecutionData::Condition { next, .. } => Some(next.clone()),
        NodeExecutionData::End => None,
    }
}

fn evaluate_condition(expression: &str, scoped_input: &Value) -> Result<bool, String> {
    let expr = expression.trim();
    if expr.is_empty() {
        return Err("empty expression".to_string());
    }

    if let Some(inner) = expr.strip_prefix('!') {
        return Ok(!evaluate_condition(inner, scoped_input)?);
    }

    if let Some((left, right)) = expr.split_once("==") {
        let left_value = resolve_operand(left.trim(), scoped_input)?;
        let right_value = resolve_operand(right.trim(), scoped_input)?;
        return Ok(left_value == right_value);
    }

    if let Some((left, right)) = expr.split_once("!=") {
        let left_value = resolve_operand(left.trim(), scoped_input)?;
        let right_value = resolve_operand(right.trim(), scoped_input)?;
        return Ok(left_value != right_value);
    }

    let value = resolve_operand(expr, scoped_input)?;
    Ok(is_truthy(&value))
}

fn resolve_operand(token: &str, scoped_input: &Value) -> Result<Value, String> {
    let trimmed = token.trim();

    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return Ok(Value::Null);
    }

    if let Some(stripped) = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Ok(Value::String(stripped.to_string()));
    }
    if let Some(stripped) = trimmed
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
    {
        return Ok(Value::String(stripped.to_string()));
    }

    if let Ok(number) = trimmed.parse::<i64>() {
        return Ok(Value::Number(Number::from(number)));
    }
    if let Ok(number) = trimmed.parse::<f64>() {
        if let Some(num) = Number::from_f64(number) {
            return Ok(Value::Number(num));
        }
    }

    let path = trimmed.strip_prefix("$.").unwrap_or(trimmed);
    resolve_path(scoped_input, path).cloned().ok_or_else(|| {
        format!(
            "path '{}' not found in scoped input",
            if path.is_empty() { "$" } else { path }
        )
    })
}

fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(root);
    }

    path.split('.')
        .filter(|segment| !segment.is_empty())
        .try_fold(root, |current, segment| current.get(segment))
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Null => false,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use serde_json::json;
    use tokio::time::sleep;

    use super::*;
    use crate::ir::{Node, NodeKind, WorkflowDefinition};

    struct MockLlmExecutor {
        output: String,
    }

    #[async_trait]
    impl LlmExecutor for MockLlmExecutor {
        async fn execute(
            &self,
            _input: LlmExecutionInput,
        ) -> Result<LlmExecutionOutput, LlmExecutionError> {
            Ok(LlmExecutionOutput {
                content: self.output.clone(),
            })
        }
    }

    struct MockToolExecutor {
        output: Value,
        fail: bool,
    }

    #[async_trait]
    impl ToolExecutor for MockToolExecutor {
        async fn execute_tool(
            &self,
            input: ToolExecutionInput,
        ) -> Result<Value, ToolExecutionError> {
            if self.fail {
                return Err(ToolExecutionError::Failed(format!(
                    "tool '{}' failed intentionally",
                    input.tool
                )));
            }
            Ok(self.output.clone())
        }
    }

    struct SequencedLlmExecutor {
        responses: Mutex<Vec<Result<LlmExecutionOutput, LlmExecutionError>>>,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl LlmExecutor for SequencedLlmExecutor {
        async fn execute(
            &self,
            _input: LlmExecutionInput,
        ) -> Result<LlmExecutionOutput, LlmExecutionError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.responses
                .lock()
                .expect("sequenced llm lock poisoned")
                .remove(0)
        }
    }

    struct SlowToolExecutor {
        delay: Duration,
    }

    #[async_trait]
    impl ToolExecutor for SlowToolExecutor {
        async fn execute_tool(
            &self,
            _input: ToolExecutionInput,
        ) -> Result<Value, ToolExecutionError> {
            sleep(self.delay).await;
            Ok(json!({"status": "slow-ok"}))
        }
    }

    struct CancellingLlmExecutor {
        cancel_flag: Arc<AtomicBool>,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl LlmExecutor for CancellingLlmExecutor {
        async fn execute(
            &self,
            _input: LlmExecutionInput,
        ) -> Result<LlmExecutionOutput, LlmExecutionError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.cancel_flag.store(true, Ordering::Relaxed);
            Err(LlmExecutionError::Client("transient failure".to_string()))
        }
    }

    fn linear_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "linear".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "llm".to_string(),
                    },
                },
                Node {
                    id: "llm".to_string(),
                    kind: NodeKind::Llm {
                        model: "gpt-4".to_string(),
                        prompt: "Summarize".to_string(),
                        next: Some("tool".to_string()),
                    },
                },
                Node {
                    id: "tool".to_string(),
                    kind: NodeKind::Tool {
                        tool: "extract".to_string(),
                        input: json!({"k": "v"}),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    fn llm_only_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "llm-only".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "llm".to_string(),
                    },
                },
                Node {
                    id: "llm".to_string(),
                    kind: NodeKind::Llm {
                        model: "gpt-4".to_string(),
                        prompt: "Summarize".to_string(),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    #[tokio::test]
    async fn executes_happy_path_linear_flow() {
        let llm = MockLlmExecutor {
            output: "ok".to_string(),
        };
        let tools = MockToolExecutor {
            output: json!({"status": "done"}),
            fail: false,
        };
        let runtime = WorkflowRuntime::new(
            linear_workflow(),
            &llm,
            Some(&tools),
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(json!({"request_id": "r1"}), None)
            .await
            .expect("linear workflow should succeed");

        assert_eq!(result.workflow_name, "linear");
        assert_eq!(result.terminal_node_id, "end");
        assert_eq!(result.node_executions.len(), 4);
        assert_eq!(
            result.node_outputs.get("llm"),
            Some(&Value::String("ok".to_string()))
        );
        assert_eq!(
            result.node_outputs.get("tool"),
            Some(&json!({"status": "done"}))
        );
        assert_eq!(result.events.len(), 8);
        assert!(result.trace.is_some());
        assert_eq!(result.replay_report, None);
    }

    #[tokio::test]
    async fn executes_conditional_branching() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "conditional".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "condition".to_string(),
                    },
                },
                Node {
                    id: "condition".to_string(),
                    kind: NodeKind::Condition {
                        expression: "input.approved".to_string(),
                        on_true: "end_true".to_string(),
                        on_false: "end_false".to_string(),
                    },
                },
                Node {
                    id: "end_true".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "end_false".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let runtime = WorkflowRuntime::new(workflow, &llm, None, WorkflowRuntimeOptions::default());

        let result = runtime
            .execute(json!({"approved": true}), None)
            .await
            .expect("conditional workflow should succeed");

        assert_eq!(result.terminal_node_id, "end_true");
    }

    #[tokio::test]
    async fn fails_when_tool_executor_is_missing() {
        let llm = MockLlmExecutor {
            output: "ok".to_string(),
        };
        let runtime = WorkflowRuntime::new(
            linear_workflow(),
            &llm,
            None,
            WorkflowRuntimeOptions::default(),
        );

        let error = runtime
            .execute(json!({}), None)
            .await
            .expect_err("workflow should fail without tool executor");

        assert!(matches!(
            error,
            WorkflowRuntimeError::MissingToolExecutor { node_id } if node_id == "tool"
        ));
    }

    #[tokio::test]
    async fn fails_on_tool_execution_error() {
        let llm = MockLlmExecutor {
            output: "ok".to_string(),
        };
        let tools = MockToolExecutor {
            output: json!({"status": "unused"}),
            fail: true,
        };
        let runtime = WorkflowRuntime::new(
            linear_workflow(),
            &llm,
            Some(&tools),
            WorkflowRuntimeOptions::default(),
        );

        let error = runtime
            .execute(json!({}), None)
            .await
            .expect_err("workflow should fail on tool error");

        assert!(matches!(
            error,
            WorkflowRuntimeError::ToolRetryExhausted { node_id, attempts: 1, .. }
                if node_id == "tool"
        ));
    }

    #[tokio::test]
    async fn retries_llm_after_transient_failure() {
        let llm = SequencedLlmExecutor {
            responses: Mutex::new(vec![
                Err(LlmExecutionError::Client("temporary".to_string())),
                Ok(LlmExecutionOutput {
                    content: "recovered".to_string(),
                }),
            ]),
            calls: AtomicUsize::new(0),
        };
        let runtime = WorkflowRuntime::new(
            llm_only_workflow(),
            &llm,
            None,
            WorkflowRuntimeOptions {
                llm_node_policy: NodeExecutionPolicy {
                    timeout: None,
                    max_retries: 1,
                },
                ..WorkflowRuntimeOptions::default()
            },
        );

        let result = runtime
            .execute(json!({"request_id": "r2"}), None)
            .await
            .expect("llm retry should recover");

        assert_eq!(result.terminal_node_id, "end");
        assert_eq!(result.node_outputs.get("llm"), Some(&json!("recovered")));
        assert_eq!(llm.calls.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn times_out_tool_execution_per_policy() {
        let llm = MockLlmExecutor {
            output: "ok".to_string(),
        };
        let tool = SlowToolExecutor {
            delay: Duration::from_millis(50),
        };
        let runtime = WorkflowRuntime::new(
            linear_workflow(),
            &llm,
            Some(&tool),
            WorkflowRuntimeOptions {
                tool_node_policy: NodeExecutionPolicy {
                    timeout: Some(Duration::from_millis(5)),
                    max_retries: 0,
                },
                ..WorkflowRuntimeOptions::default()
            },
        );

        let error = runtime
            .execute(json!({}), None)
            .await
            .expect_err("tool execution should time out");

        assert!(matches!(
            error,
            WorkflowRuntimeError::ToolTimeout {
                node_id,
                timeout_ms: 5,
                attempts: 1,
            } if node_id == "tool"
        ));
    }

    #[tokio::test]
    async fn cancels_between_retry_attempts() {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let llm = CancellingLlmExecutor {
            cancel_flag: Arc::clone(&cancel_flag),
            calls: AtomicUsize::new(0),
        };
        let runtime = WorkflowRuntime::new(
            llm_only_workflow(),
            &llm,
            None,
            WorkflowRuntimeOptions {
                llm_node_policy: NodeExecutionPolicy {
                    timeout: None,
                    max_retries: 3,
                },
                ..WorkflowRuntimeOptions::default()
            },
        );

        let error = runtime
            .execute(json!({}), Some(cancel_flag.as_ref()))
            .await
            .expect_err("workflow should stop when cancellation is observed");

        assert!(matches!(error, WorkflowRuntimeError::Cancelled));
        assert_eq!(llm.calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn enforces_step_limit_guard() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "loop".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "condition".to_string(),
                    },
                },
                Node {
                    id: "condition".to_string(),
                    kind: NodeKind::Condition {
                        expression: "true".to_string(),
                        on_true: "condition".to_string(),
                        on_false: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let runtime = WorkflowRuntime::new(
            workflow,
            &llm,
            None,
            WorkflowRuntimeOptions {
                max_steps: 3,
                ..WorkflowRuntimeOptions::default()
            },
        );

        let error = runtime
            .execute(json!({}), None)
            .await
            .expect_err("workflow should fail on step limit");

        assert!(matches!(
            error,
            WorkflowRuntimeError::StepLimitExceeded { max_steps: 3 }
        ));
    }

    #[tokio::test]
    async fn validates_recorded_trace_in_replay_mode() {
        let llm = MockLlmExecutor {
            output: "ok".to_string(),
        };
        let tools = MockToolExecutor {
            output: json!({"status": "done"}),
            fail: false,
        };
        let runtime = WorkflowRuntime::new(
            linear_workflow(),
            &llm,
            Some(&tools),
            WorkflowRuntimeOptions {
                replay_mode: WorkflowReplayMode::ValidateRecordedTrace,
                ..WorkflowRuntimeOptions::default()
            },
        );

        let result = runtime
            .execute(json!({"request_id": "r1"}), None)
            .await
            .expect("replay validation should pass");

        assert!(result.trace.is_some());
        assert_eq!(
            result.replay_report.as_ref().map(|r| r.total_events),
            Some(9)
        );
    }

    #[test]
    fn scope_capabilities_enforce_read_write_boundaries() {
        let mut scope = RuntimeScope::new(json!({"k": "v"}));

        let read_error = scope
            .scoped_input(ScopeCapability::LlmWrite)
            .expect_err("write capability should not read scope");
        assert!(matches!(read_error, ScopeAccessError::ReadDenied { .. }));

        let write_error = scope
            .record_tool_output("tool", json!({"ok": true}), ScopeCapability::LlmWrite)
            .expect_err("llm write capability should not write tool output");
        assert!(matches!(write_error, ScopeAccessError::WriteDenied { .. }));
    }
}
