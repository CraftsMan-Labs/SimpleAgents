use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use simple_agent_type::message::Message;
use simple_agent_type::request::CompletionRequest;
use simple_agents_core::{CompletionOptions, CompletionOutcome, SimpleAgentsClient};
use thiserror::Error;
use tokio::time::timeout;

use crate::checkpoint::WorkflowCheckpoint;
use crate::expressions;
use crate::ir::{MergePolicy, Node, NodeKind, ReduceOperation, WorkflowDefinition};
use crate::recorder::{TraceRecordError, TraceRecorder};
use crate::replay::{replay_trace, ReplayError, ReplayReport};
use crate::scheduler::DagScheduler;
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
    /// Global scheduler bound for node-level concurrent fan-out.
    pub scheduler_max_in_flight: usize,
    /// Named subgraph registry used by `subgraph` nodes.
    pub subgraph_registry: BTreeMap<String, WorkflowDefinition>,
    /// Runtime guardrails for expression and fan-out resource usage.
    pub security_limits: RuntimeSecurityLimits,
}

/// Runtime-enforced security and resource limits.
#[derive(Debug, Clone)]
pub struct RuntimeSecurityLimits {
    /// Maximum serialized bytes allowed for one expression evaluation scope.
    pub max_expression_scope_bytes: usize,
    /// Maximum array size accepted by `map` nodes.
    pub max_map_items: usize,
    /// Maximum branch count accepted by `parallel` nodes.
    pub max_parallel_branches: usize,
    /// Maximum array size accepted by `filter` nodes.
    pub max_filter_items: usize,
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
#[derive(Debug, Clone, Default)]
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
            scheduler_max_in_flight: 8,
            subgraph_registry: BTreeMap::new(),
            security_limits: RuntimeSecurityLimits::default(),
        }
    }
}

impl Default for RuntimeSecurityLimits {
    fn default() -> Self {
        Self {
            max_expression_scope_bytes: 128 * 1024,
            max_map_items: 4096,
            max_parallel_branches: 128,
            max_filter_items: 8192,
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
    /// Debounce gate decision.
    Debounce {
        /// Resolved debounce key.
        key: String,
        /// Whether the event was suppressed.
        suppressed: bool,
        /// Chosen next node id.
        next: String,
    },
    /// Throttle gate decision.
    Throttle {
        /// Resolved throttle key.
        key: String,
        /// Whether the event was throttled.
        throttled: bool,
        /// Chosen next node id.
        next: String,
    },
    /// Explicit retry/compensation execution result.
    RetryCompensate {
        /// Primary tool name.
        tool: String,
        /// Total primary attempts executed.
        attempts: usize,
        /// Whether compensation path was used.
        compensated: bool,
        /// Node output payload.
        output: Value,
        /// Chosen next node id.
        next: String,
    },
    /// Human decision result.
    HumanInTheLoop {
        /// True when approved.
        approved: bool,
        /// Optional human payload routed through the node.
        response: Value,
        /// Chosen next node id.
        next: String,
    },
    /// Cache read result.
    CacheRead {
        /// Cache key.
        key: String,
        /// True when key was found.
        hit: bool,
        /// Returned value (null on miss).
        value: Value,
        /// Chosen next node id.
        next: String,
    },
    /// Cache write result.
    CacheWrite {
        /// Cache key.
        key: String,
        /// Stored value.
        value: Value,
        /// Next node id.
        next: String,
    },
    /// Event trigger evaluation result.
    EventTrigger {
        /// Expected event name.
        event: String,
        /// True when input event matched.
        matched: bool,
        /// Chosen next node id.
        next: String,
    },
    /// Router/selector decision.
    Router {
        /// Selected route node id.
        selected: String,
        /// Chosen next node id.
        next: String,
    },
    /// Transform node output.
    Transform {
        /// Original transform expression.
        expression: String,
        /// Evaluated output payload.
        output: Value,
        /// Next node id.
        next: String,
    },
    /// Loop node decision.
    Loop {
        /// Original loop condition.
        condition: String,
        /// Evaluated bool.
        evaluated: bool,
        /// Current iteration when entering body. Zero when exiting loop.
        iteration: u32,
        /// Chosen next node id.
        next: String,
    },
    /// Subgraph execution result.
    Subgraph {
        /// Subgraph registry key.
        graph: String,
        /// Subgraph terminal node id.
        terminal_node_id: String,
        /// Aggregated subgraph node outputs.
        output: Value,
        /// Next node id.
        next: String,
    },
    /// Batch extraction result.
    Batch {
        /// Source path used for extraction.
        items_path: String,
        /// Number of extracted items.
        item_count: usize,
        /// Next node id.
        next: String,
    },
    /// Filter evaluation result.
    Filter {
        /// Source path used for extraction.
        items_path: String,
        /// Filter predicate expression.
        expression: String,
        /// Number of retained items.
        kept: usize,
        /// Next node id.
        next: String,
    },
    /// Parallel node aggregate result.
    Parallel {
        /// Branch node ids executed by this node.
        branches: Vec<String>,
        /// Branch outputs keyed by branch node id.
        outputs: BTreeMap<String, Value>,
        /// Next node id.
        next: String,
    },
    /// Merge node aggregate output.
    Merge {
        /// Merge policy used.
        policy: MergePolicy,
        /// Source node ids consumed.
        sources: Vec<String>,
        /// Merged value.
        output: Value,
        /// Next node id.
        next: String,
    },
    /// Map node output.
    Map {
        /// Number of input items mapped.
        item_count: usize,
        /// Collected mapped values in source order.
        output: Value,
        /// Next node id.
        next: String,
    },
    /// Reduce node output.
    Reduce {
        /// Reduce operation executed.
        operation: ReduceOperation,
        /// Reduced result.
        output: Value,
        /// Next node id.
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
    /// Retry diagnostics captured during node execution.
    pub retry_events: Vec<WorkflowRetryEvent>,
    /// Node output map keyed by node id.
    pub node_outputs: BTreeMap<String, Value>,
    /// Optional deterministic trace captured during this run.
    pub trace: Option<WorkflowTrace>,
    /// Replay validation report when replay mode is enabled.
    pub replay_report: Option<ReplayReport>,
}

/// Captures one retry decision and the reason it occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRetryEvent {
    /// Zero-based step index for the active node.
    pub step: usize,
    /// Node id that will be retried.
    pub node_id: String,
    /// Operation category (`llm` or `tool`).
    pub operation: String,
    /// Attempt that failed and triggered the retry.
    pub failed_attempt: usize,
    /// Human-readable retry reason.
    pub reason: String,
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
    MapRead,
    MapWrite,
    ReduceWrite,
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
            Self::MapRead => "map_read",
            Self::MapWrite => "map_write",
            Self::ReduceWrite => "reduce_write",
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
    /// Loop condition expression could not be evaluated.
    #[error("loop node '{node_id}' has invalid condition '{expression}': {reason}")]
    InvalidLoopCondition {
        /// Failing node id.
        node_id: String,
        /// Loop condition expression.
        expression: String,
        /// Detailed reason.
        reason: String,
    },
    /// Loop exceeded configured max iterations.
    #[error("loop node '{node_id}' exceeded max iterations ({max_iterations})")]
    LoopIterationLimitExceeded {
        /// Failing node id.
        node_id: String,
        /// Configured max iterations.
        max_iterations: u32,
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
    /// Parallel branch references unsupported node kind.
    #[error("parallel node '{node_id}' cannot execute branch '{branch_id}': {reason}")]
    ParallelBranchUnsupported {
        /// Parallel node id.
        node_id: String,
        /// Branch node id.
        branch_id: String,
        /// Reason for rejection.
        reason: String,
    },
    /// Merge node cannot resolve one or more sources.
    #[error("merge node '{node_id}' missing source output from '{source_id}'")]
    MissingMergeSource {
        /// Merge node id.
        node_id: String,
        /// Missing source id.
        source_id: String,
    },
    /// Merge quorum policy could not be satisfied.
    #[error("merge node '{node_id}' quorum not met: required {required}, resolved {resolved}")]
    MergeQuorumNotMet {
        /// Merge node id.
        node_id: String,
        /// Required number of sources.
        required: usize,
        /// Number of resolved sources.
        resolved: usize,
    },
    /// Map node items path did not resolve to an array.
    #[error("map node '{node_id}' items_path '{items_path}' did not resolve to an array")]
    MapItemsNotArray {
        /// Map node id.
        node_id: String,
        /// Configured path.
        items_path: String,
    },
    /// Reduce node source value is not reducible.
    #[error("reduce node '{node_id}' source '{source_node}' is not reducible: {reason}")]
    InvalidReduceInput {
        /// Reduce node id.
        node_id: String,
        /// Source node id.
        source_node: String,
        /// Detailed reason.
        reason: String,
    },
    /// Subgraph registry key not found.
    #[error("subgraph node '{node_id}' references unknown graph '{graph}'")]
    SubgraphNotFound { node_id: String, graph: String },
    /// Batch source path did not resolve to an array.
    #[error("batch node '{node_id}' items_path '{items_path}' did not resolve to an array")]
    BatchItemsNotArray { node_id: String, items_path: String },
    /// Filter source path did not resolve to an array.
    #[error("filter node '{node_id}' items_path '{items_path}' did not resolve to an array")]
    FilterItemsNotArray { node_id: String, items_path: String },
    /// Filter expression failed while evaluating one item.
    #[error("filter node '{node_id}' expression '{expression}' failed: {reason}")]
    InvalidFilterExpression {
        node_id: String,
        expression: String,
        reason: String,
    },
    /// Serialized expression scope exceeded configured runtime limit.
    #[error(
        "expression scope too large on node '{node_id}': {actual_bytes} bytes exceeds limit {limit_bytes}"
    )]
    ExpressionScopeLimitExceeded {
        node_id: String,
        actual_bytes: usize,
        limit_bytes: usize,
    },
    /// Parallel branch fan-out exceeded configured runtime limit.
    #[error(
        "parallel node '{node_id}' has {actual_branches} branches, exceeding limit {max_branches}"
    )]
    ParallelBranchLimitExceeded {
        node_id: String,
        actual_branches: usize,
        max_branches: usize,
    },
    /// Map item count exceeded configured runtime limit.
    #[error("map node '{node_id}' has {actual_items} items, exceeding limit {max_items}")]
    MapItemLimitExceeded {
        node_id: String,
        actual_items: usize,
        max_items: usize,
    },
    /// Filter item count exceeded configured runtime limit.
    #[error("filter node '{node_id}' has {actual_items} items, exceeding limit {max_items}")]
    FilterItemLimitExceeded {
        node_id: String,
        actual_items: usize,
        max_items: usize,
    },
    /// A node-required path could not be resolved.
    #[error("node '{node_id}' could not resolve required path '{path}'")]
    MissingPath { node_id: String, path: String },
    /// A cache key path did not resolve to a string.
    #[error("node '{node_id}' path '{path}' did not resolve to a string cache key")]
    CacheKeyNotString { node_id: String, path: String },
    /// Human decision value is unsupported.
    #[error("human node '{node_id}' has unsupported decision value at '{path}': {value}")]
    InvalidHumanDecision {
        node_id: String,
        path: String,
        value: String,
    },
    /// Event value did not resolve to a string.
    #[error("event trigger node '{node_id}' path '{path}' did not resolve to an event string")]
    InvalidEventValue { node_id: String, path: String },
    /// Router expression evaluation failed.
    #[error("router node '{node_id}' route expression '{expression}' failed: {reason}")]
    InvalidRouterExpression {
        node_id: String,
        expression: String,
        reason: String,
    },
    /// Transform expression evaluation failed.
    #[error("transform node '{node_id}' expression '{expression}' failed: {reason}")]
    InvalidTransformExpression {
        node_id: String,
        expression: String,
        reason: String,
    },
    /// Explicit retry/compensate node exhausted primary and compensation failed.
    #[error(
        "retry_compensate node '{node_id}' failed after {attempts} primary attempt(s) and compensation error: {compensation_error}"
    )]
    RetryCompensateFailed {
        node_id: String,
        attempts: usize,
        compensation_error: ToolExecutionError,
    },
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

        let start_id = find_start_node_id(&workflow)?;
        self.execute_from_node(
            workflow,
            RuntimeScope::new(input),
            start_id,
            0,
            cancellation,
        )
        .await
    }

    /// Resumes execution from a checkpoint created after a node failure.
    pub async fn execute_resume_from_failure(
        &self,
        checkpoint: &WorkflowCheckpoint,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<WorkflowRunResult, WorkflowRuntimeError> {
        let workflow = if self.options.validate_before_run {
            validate_and_normalize(&self.definition)?
        } else {
            self.definition.normalized()
        };

        let scope_input = checkpoint
            .scope_snapshot
            .get("input")
            .cloned()
            .unwrap_or_else(|| checkpoint.scope_snapshot.clone());

        self.execute_from_node(
            workflow,
            RuntimeScope::new(scope_input),
            checkpoint.next_node_id.clone(),
            checkpoint.step,
            cancellation,
        )
        .await
    }

    async fn execute_from_node(
        &self,
        workflow: WorkflowDefinition,
        mut scope: RuntimeScope,
        start_node_id: String,
        starting_step: usize,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<WorkflowRunResult, WorkflowRuntimeError> {
        let node_index = build_node_index(&workflow);

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
        let mut events = Vec::new();
        let mut retry_events = Vec::new();
        let mut node_executions = Vec::new();
        let mut current_id = start_node_id;

        for step in starting_step..self.options.max_steps {
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
                .execute_node(
                    node,
                    &node_index,
                    step,
                    &mut scope,
                    cancellation,
                    &mut retry_events,
                )
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
                    retry_events,
                    node_outputs: scope.node_outputs,
                    trace,
                    replay_report,
                });
            }

            current_id = next_node.ok_or(WorkflowRuntimeError::MissingNextTransition {
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
        node_index: &HashMap<&str, &Node>,
        step: usize,
        scope: &mut RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
        retry_events: &mut Vec<WorkflowRetryEvent>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        match &node.kind {
            NodeKind::Start { next } => Ok(self.execute_start_node(step, node, next)),
            NodeKind::Llm {
                model,
                prompt,
                next,
            } => {
                self.execute_llm_node(
                    step,
                    node,
                    LlmNodeSpec {
                        model,
                        prompt,
                        next,
                    },
                    scope,
                    cancellation,
                    retry_events,
                )
                .await
            }
            NodeKind::Tool { tool, input, next } => {
                self.execute_tool_node(
                    step,
                    node,
                    ToolNodeSpec { tool, input, next },
                    scope,
                    cancellation,
                    retry_events,
                )
                .await
            }
            NodeKind::Condition {
                expression,
                on_true,
                on_false,
            } => self.execute_condition_node(
                step,
                node,
                ConditionNodeSpec {
                    expression,
                    on_true,
                    on_false,
                },
                scope,
                cancellation,
            ),
            NodeKind::Debounce {
                key_path,
                window_steps,
                next,
                on_suppressed,
            } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let suppressed = scope.debounce(&node.id, &key, step, *window_steps);
                let chosen_next = if suppressed {
                    on_suppressed.clone().unwrap_or_else(|| next.clone())
                } else {
                    next.clone()
                };

                scope
                    .record_node_output(
                        &node.id,
                        json!({"key": key.clone(), "suppressed": suppressed}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Debounce {
                        key,
                        suppressed,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::Throttle {
                key_path,
                window_steps,
                next,
                on_throttled,
            } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let throttled = scope.throttle(&node.id, &key, step, *window_steps);
                let chosen_next = if throttled {
                    on_throttled.clone().unwrap_or_else(|| next.clone())
                } else {
                    next.clone()
                };

                scope
                    .record_node_output(
                        &node.id,
                        json!({"key": key.clone(), "throttled": throttled}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Throttle {
                        key,
                        throttled,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::RetryCompensate {
                tool,
                input,
                max_retries,
                compensate_tool,
                compensate_input,
                next,
                on_compensated,
            } => {
                let executor = self.tool_executor.ok_or_else(|| {
                    WorkflowRuntimeError::MissingToolExecutor {
                        node_id: node.id.clone(),
                    }
                })?;
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ToolRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let total_attempts = max_retries.saturating_add(1);
                let mut last_error = None;
                let mut output = Value::Null;
                let mut compensated = false;

                for attempt in 1..=total_attempts {
                    check_cancelled(cancellation)?;
                    match executor
                        .execute_tool(ToolExecutionInput {
                            node_id: node.id.clone(),
                            tool: tool.clone(),
                            input: input.clone(),
                            scoped_input: scoped_input.clone(),
                        })
                        .await
                    {
                        Ok(value) => {
                            output = json!({"status": "ok", "attempt": attempt, "value": value});
                            break;
                        }
                        Err(error) => {
                            last_error = Some(error.clone());
                            if attempt < total_attempts {
                                retry_events.push(WorkflowRetryEvent {
                                    step,
                                    node_id: node.id.clone(),
                                    operation: "retry_compensate".to_string(),
                                    failed_attempt: attempt,
                                    reason: error.to_string(),
                                });
                            }
                        }
                    }
                }

                if output.is_null() {
                    compensated = true;
                    let compensation = executor
                        .execute_tool(ToolExecutionInput {
                            node_id: node.id.clone(),
                            tool: compensate_tool.clone(),
                            input: compensate_input.clone(),
                            scoped_input,
                        })
                        .await
                        .map_err(|compensation_error| {
                            WorkflowRuntimeError::RetryCompensateFailed {
                                node_id: node.id.clone(),
                                attempts: total_attempts,
                                compensation_error,
                            }
                        })?;
                    output = json!({
                        "status": "compensated",
                        "attempts": total_attempts,
                        "last_error": last_error.map(|error| error.to_string()).unwrap_or_default(),
                        "compensation": compensation
                    });
                }

                let chosen_next = if compensated {
                    on_compensated.clone().unwrap_or_else(|| next.clone())
                } else {
                    next.clone()
                };

                scope
                    .record_node_output(&node.id, output.clone(), ScopeCapability::MapWrite)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::RetryCompensate {
                        tool: tool.clone(),
                        attempts: total_attempts,
                        compensated,
                        output,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::HumanInTheLoop {
                decision_path,
                response_path,
                on_approve,
                on_reject,
            } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let decision_value =
                    resolve_path(&scoped_input, decision_path).ok_or_else(|| {
                        WorkflowRuntimeError::MissingPath {
                            node_id: node.id.clone(),
                            path: decision_path.clone(),
                        }
                    })?;
                let approved = evaluate_human_decision(decision_value).ok_or_else(|| {
                    WorkflowRuntimeError::InvalidHumanDecision {
                        node_id: node.id.clone(),
                        path: decision_path.clone(),
                        value: decision_value.to_string(),
                    }
                })?;
                let response = if let Some(path) = response_path {
                    resolve_path(&scoped_input, path).cloned().ok_or_else(|| {
                        WorkflowRuntimeError::MissingPath {
                            node_id: node.id.clone(),
                            path: path.clone(),
                        }
                    })?
                } else {
                    Value::Null
                };
                let chosen_next = if approved {
                    on_approve.clone()
                } else {
                    on_reject.clone()
                };

                scope
                    .record_node_output(
                        &node.id,
                        json!({"approved": approved, "response": response.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::HumanInTheLoop {
                        approved,
                        response,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::CacheWrite {
                key_path,
                value_path,
                next,
            } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let value = resolve_path(&scoped_input, value_path)
                    .cloned()
                    .ok_or_else(|| WorkflowRuntimeError::MissingPath {
                        node_id: node.id.clone(),
                        path: value_path.clone(),
                    })?;

                scope.put_cache(&key, value.clone());
                scope
                    .record_node_output(
                        &node.id,
                        json!({"key": key.clone(), "value": value.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::CacheWrite {
                        key,
                        value,
                        next: next.clone(),
                    },
                })
            }
            NodeKind::CacheRead {
                key_path,
                next,
                on_miss,
            } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let key = resolve_string_path(&scoped_input, key_path).ok_or_else(|| {
                    WorkflowRuntimeError::CacheKeyNotString {
                        node_id: node.id.clone(),
                        path: key_path.clone(),
                    }
                })?;
                let value = scope.cache_value(&key).cloned().unwrap_or(Value::Null);
                let hit = !value.is_null();
                let chosen_next = if hit {
                    next.clone()
                } else {
                    on_miss.clone().unwrap_or_else(|| next.clone())
                };

                scope
                    .record_node_output(
                        &node.id,
                        json!({"key": key.clone(), "hit": hit, "value": value.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::CacheRead {
                        key,
                        hit,
                        value,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::EventTrigger {
                event,
                event_path,
                next,
                on_mismatch,
            } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let actual = resolve_path(&scoped_input, event_path)
                    .and_then(Value::as_str)
                    .ok_or_else(|| WorkflowRuntimeError::InvalidEventValue {
                        node_id: node.id.clone(),
                        path: event_path.clone(),
                    })?;
                let matched = actual == event;
                let chosen_next = if matched {
                    next.clone()
                } else {
                    on_mismatch.clone().unwrap_or_else(|| next.clone())
                };

                scope
                    .record_node_output(
                        &node.id,
                        json!({"event": event.clone(), "matched": matched, "actual": actual}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::EventTrigger {
                        event: event.clone(),
                        matched,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::Router { routes, default } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                enforce_expression_scope_budget(
                    &node.id,
                    &scoped_input,
                    self.options.security_limits.max_expression_scope_bytes,
                )?;
                let mut selected = default.clone();
                for route in routes {
                    let matched = expressions::evaluate_bool(&route.when, &scoped_input).map_err(
                        |reason| WorkflowRuntimeError::InvalidRouterExpression {
                            node_id: node.id.clone(),
                            expression: route.when.clone(),
                            reason: reason.to_string(),
                        },
                    )?;
                    if matched {
                        selected = route.next.clone();
                        break;
                    }
                }

                scope
                    .record_node_output(
                        &node.id,
                        json!({"selected": selected.clone()}),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Router {
                        selected: selected.clone(),
                        next: selected,
                    },
                })
            }
            NodeKind::Transform { expression, next } => {
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                let output =
                    evaluate_transform_expression(expression, &scoped_input).map_err(|reason| {
                        WorkflowRuntimeError::InvalidTransformExpression {
                            node_id: node.id.clone(),
                            expression: expression.clone(),
                            reason,
                        }
                    })?;

                scope
                    .record_node_output(&node.id, output.clone(), ScopeCapability::MapWrite)
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Transform {
                        expression: expression.clone(),
                        output,
                        next: next.clone(),
                    },
                })
            }
            NodeKind::Loop {
                condition,
                body,
                next,
                max_iterations,
            } => {
                check_cancelled(cancellation)?;
                let scoped_input =
                    scope
                        .scoped_input(ScopeCapability::ConditionRead)
                        .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                            node_id: node.id.clone(),
                            source,
                        })?;
                enforce_expression_scope_budget(
                    &node.id,
                    &scoped_input,
                    self.options.security_limits.max_expression_scope_bytes,
                )?;
                let evaluated =
                    expressions::evaluate_bool(condition, &scoped_input).map_err(|reason| {
                        WorkflowRuntimeError::InvalidLoopCondition {
                            node_id: node.id.clone(),
                            expression: condition.clone(),
                            reason: reason.to_string(),
                        }
                    })?;

                let (iteration, chosen_next) = if evaluated {
                    let iteration = scope.loop_iteration(&node.id).saturating_add(1);
                    if let Some(limit) = max_iterations {
                        if iteration > *limit {
                            return Err(WorkflowRuntimeError::LoopIterationLimitExceeded {
                                node_id: node.id.clone(),
                                max_iterations: *limit,
                            });
                        }
                    }
                    scope.set_loop_iteration(&node.id, iteration);
                    (iteration, body.clone())
                } else {
                    scope.clear_loop_iteration(&node.id);
                    (0, next.clone())
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
                    data: NodeExecutionData::Loop {
                        condition: condition.clone(),
                        evaluated,
                        iteration,
                        next: chosen_next,
                    },
                })
            }
            NodeKind::Parallel {
                branches,
                next,
                max_in_flight,
            } => {
                self.execute_parallel_node(
                    step,
                    node,
                    ParallelNodeSpec {
                        node_index,
                        branches,
                        next,
                        max_in_flight: *max_in_flight,
                    },
                    scope,
                    cancellation,
                    retry_events,
                )
                .await
            }
            NodeKind::Merge {
                sources,
                policy,
                quorum,
                next,
            } => self.execute_merge_node(
                step,
                node,
                MergeNodeSpec {
                    sources,
                    policy,
                    quorum: *quorum,
                    next,
                },
                scope,
            ),
            NodeKind::Map {
                tool,
                items_path,
                next,
                max_in_flight,
            } => {
                self.execute_map_node(
                    step,
                    node,
                    MapNodeSpec {
                        tool,
                        items_path,
                        next,
                        max_in_flight: *max_in_flight,
                    },
                    scope,
                    cancellation,
                    retry_events,
                )
                .await
            }
            NodeKind::Reduce {
                source,
                operation,
                next,
            } => self.execute_reduce_node(step, node, source, operation, next, scope),
            NodeKind::Subgraph { graph, next } => {
                check_cancelled(cancellation)?;
                let next_node =
                    next.clone()
                        .ok_or_else(|| WorkflowRuntimeError::MissingNextEdge {
                            node_id: node.id.clone(),
                        })?;
                let subgraph = self.options.subgraph_registry.get(graph).ok_or_else(|| {
                    WorkflowRuntimeError::SubgraphNotFound {
                        node_id: node.id.clone(),
                        graph: graph.clone(),
                    }
                })?;

                let subgraph_runtime = WorkflowRuntime::new(
                    subgraph.clone(),
                    self.llm_executor,
                    self.tool_executor,
                    WorkflowRuntimeOptions {
                        replay_mode: WorkflowReplayMode::Disabled,
                        enable_trace_recording: false,
                        ..self.options.clone()
                    },
                );
                let subgraph_result = Box::pin(
                    subgraph_runtime.execute(
                        scope
                            .scoped_input(ScopeCapability::ConditionRead)
                            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                                node_id: node.id.clone(),
                                source,
                            })?,
                        cancellation,
                    ),
                )
                .await?;

                let subgraph_output = json!({
                    "terminal_node_id": subgraph_result.terminal_node_id,
                    "node_outputs": subgraph_result.node_outputs,
                });
                scope
                    .record_node_output(
                        &node.id,
                        subgraph_output.clone(),
                        ScopeCapability::MapWrite,
                    )
                    .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                        node_id: node.id.clone(),
                        source,
                    })?;

                Ok(NodeExecution {
                    step,
                    node_id: node.id.clone(),
                    data: NodeExecutionData::Subgraph {
                        graph: graph.clone(),
                        terminal_node_id: subgraph_result.terminal_node_id,
                        output: subgraph_output,
                        next: next_node,
                    },
                })
            }
            NodeKind::Batch { items_path, next } => {
                self.execute_batch_node(step, node, items_path, next, scope)
            }
            NodeKind::Filter {
                items_path,
                expression,
                next,
            } => self.execute_filter_node(step, node, items_path, expression, next, scope),
            NodeKind::End => Ok(NodeExecution {
                step,
                node_id: node.id.clone(),
                data: NodeExecutionData::End,
            }),
        }
    }

    fn execute_start_node(&self, step: usize, node: &Node, next: &str) -> NodeExecution {
        NodeExecution {
            step,
            node_id: node.id.clone(),
            data: NodeExecutionData::Start {
                next: next.to_string(),
            },
        }
    }

    async fn execute_llm_node(
        &self,
        step: usize,
        node: &Node,
        spec: LlmNodeSpec<'_>,
        scope: &mut RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
        retry_events: &mut Vec<WorkflowRetryEvent>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let next_node = spec
            .next
            .clone()
            .ok_or_else(|| WorkflowRuntimeError::MissingNextEdge {
                node_id: node.id.clone(),
            })?;

        let (output, llm_retries) = self
            .execute_llm_with_policy(step, node, spec.model, spec.prompt, scope, cancellation)
            .await?;
        retry_events.extend(llm_retries);

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
                model: spec.model.to_string(),
                output: output.content,
                next: next_node,
            },
        })
    }

    async fn execute_tool_node(
        &self,
        step: usize,
        node: &Node,
        spec: ToolNodeSpec<'_>,
        scope: &mut RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
        retry_events: &mut Vec<WorkflowRetryEvent>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let next_node = spec
            .next
            .clone()
            .ok_or_else(|| WorkflowRuntimeError::MissingNextEdge {
                node_id: node.id.clone(),
            })?;

        let executor =
            self.tool_executor
                .ok_or_else(|| WorkflowRuntimeError::MissingToolExecutor {
                    node_id: node.id.clone(),
                })?;

        let scoped_input = scope
            .scoped_input(ScopeCapability::ToolRead)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        let (tool_output, tool_retries) = self
            .execute_tool_with_policy_for_scope(ToolPolicyRequest {
                step,
                node,
                tool: spec.tool,
                input: spec.input,
                executor,
                scoped_input,
                cancellation,
            })
            .await?;
        retry_events.extend(tool_retries);

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
                tool: spec.tool.to_string(),
                output: tool_output,
                next: next_node,
            },
        })
    }

    fn execute_condition_node(
        &self,
        step: usize,
        node: &Node,
        spec: ConditionNodeSpec<'_>,
        scope: &mut RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        check_cancelled(cancellation)?;
        let scoped_input =
            scope
                .scoped_input(ScopeCapability::ConditionRead)
                .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                    node_id: node.id.clone(),
                    source,
                })?;
        enforce_expression_scope_budget(
            &node.id,
            &scoped_input,
            self.options.security_limits.max_expression_scope_bytes,
        )?;
        let evaluated =
            expressions::evaluate_bool(spec.expression, &scoped_input).map_err(|reason| {
                WorkflowRuntimeError::InvalidCondition {
                    node_id: node.id.clone(),
                    expression: spec.expression.to_string(),
                    reason: reason.to_string(),
                }
            })?;
        let next = if evaluated {
            spec.on_true.to_string()
        } else {
            spec.on_false.to_string()
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
                expression: spec.expression.to_string(),
                evaluated,
                next,
            },
        })
    }

    async fn execute_parallel_node(
        &self,
        step: usize,
        node: &Node,
        spec: ParallelNodeSpec<'_>,
        scope: &mut RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
        retry_events: &mut Vec<WorkflowRetryEvent>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        check_cancelled(cancellation)?;
        if spec.branches.len() > self.options.security_limits.max_parallel_branches {
            return Err(WorkflowRuntimeError::ParallelBranchLimitExceeded {
                node_id: node.id.clone(),
                actual_branches: spec.branches.len(),
                max_branches: self.options.security_limits.max_parallel_branches,
            });
        }
        let base_scope = scope
            .scoped_input(ScopeCapability::MapRead)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;
        let scheduler = DagScheduler::new(
            spec.max_in_flight
                .unwrap_or(self.options.scheduler_max_in_flight),
        );
        let parallel_node_id = node.id.clone();

        let branch_outputs: Vec<(String, Value, Vec<WorkflowRetryEvent>)> = scheduler
            .run_bounded(spec.branches.iter().cloned(), |branch_id| {
                let parallel_node_id = parallel_node_id.clone();
                let base_scope = base_scope.clone();
                async move {
                    let branch_node = spec.node_index.get(branch_id.as_str()).ok_or_else(|| {
                        WorkflowRuntimeError::NodeNotFound {
                            node_id: branch_id.clone(),
                        }
                    })?;
                    self.execute_parallel_branch(
                        step,
                        &parallel_node_id,
                        branch_node,
                        base_scope,
                        cancellation,
                    )
                    .await
                }
            })
            .await?;

        let mut outputs: BTreeMap<String, Value> = BTreeMap::new();
        for (branch_id, output, branch_retry_events) in branch_outputs {
            retry_events.extend(branch_retry_events);
            scope
                .record_node_output(&branch_id, output.clone(), ScopeCapability::MapWrite)
                .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                    node_id: node.id.clone(),
                    source,
                })?;
            outputs.insert(branch_id, output);
        }

        scope
            .record_node_output(
                &node.id,
                Value::Object(
                    outputs
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                ),
                ScopeCapability::MapWrite,
            )
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        Ok(NodeExecution {
            step,
            node_id: node.id.clone(),
            data: NodeExecutionData::Parallel {
                branches: spec.branches.to_vec(),
                outputs,
                next: spec.next.to_string(),
            },
        })
    }

    fn execute_merge_node(
        &self,
        step: usize,
        node: &Node,
        spec: MergeNodeSpec<'_>,
        scope: &mut RuntimeScope,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let mut resolved = Vec::with_capacity(spec.sources.len());
        for source in spec.sources {
            let Some(value) = scope.node_output(source).cloned() else {
                return Err(WorkflowRuntimeError::MissingMergeSource {
                    node_id: node.id.clone(),
                    source_id: source.clone(),
                });
            };
            resolved.push((source.clone(), value));
        }

        let output = match spec.policy {
            MergePolicy::First => resolved
                .first()
                .map(|(_, value)| value.clone())
                .unwrap_or(Value::Null),
            MergePolicy::All => Value::Array(
                resolved
                    .iter()
                    .map(|(_, value)| value.clone())
                    .collect::<Vec<_>>(),
            ),
            MergePolicy::Quorum => {
                let required = spec.quorum.unwrap_or_default();
                let resolved_count = resolved.len();
                if resolved_count < required {
                    return Err(WorkflowRuntimeError::MergeQuorumNotMet {
                        node_id: node.id.clone(),
                        required,
                        resolved: resolved_count,
                    });
                }
                Value::Array(
                    resolved
                        .iter()
                        .take(required)
                        .map(|(_, value)| value.clone())
                        .collect::<Vec<_>>(),
                )
            }
        };

        scope
            .record_node_output(&node.id, output.clone(), ScopeCapability::ReduceWrite)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        Ok(NodeExecution {
            step,
            node_id: node.id.clone(),
            data: NodeExecutionData::Merge {
                policy: spec.policy.clone(),
                sources: spec.sources.to_vec(),
                output,
                next: spec.next.to_string(),
            },
        })
    }

    async fn execute_map_node(
        &self,
        step: usize,
        node: &Node,
        spec: MapNodeSpec<'_>,
        scope: &mut RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
        retry_events: &mut Vec<WorkflowRetryEvent>,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let executor =
            self.tool_executor
                .ok_or_else(|| WorkflowRuntimeError::MissingToolExecutor {
                    node_id: node.id.clone(),
                })?;

        let scoped_input = scope
            .scoped_input(ScopeCapability::MapRead)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;
        let items = resolve_path(&scoped_input, spec.items_path)
            .and_then(Value::as_array)
            .ok_or_else(|| WorkflowRuntimeError::MapItemsNotArray {
                node_id: node.id.clone(),
                items_path: spec.items_path.to_string(),
            })?
            .clone();
        if items.len() > self.options.security_limits.max_map_items {
            return Err(WorkflowRuntimeError::MapItemLimitExceeded {
                node_id: node.id.clone(),
                actual_items: items.len(),
                max_items: self.options.security_limits.max_map_items,
            });
        }

        let scheduler = DagScheduler::new(
            spec.max_in_flight
                .unwrap_or(self.options.scheduler_max_in_flight),
        );
        let map_node = node.clone();
        let mapped: Vec<(Value, Vec<WorkflowRetryEvent>)> = scheduler
            .run_bounded(items.into_iter().enumerate(), |(index, item)| {
                let scoped_input = scoped_input.clone();
                let map_node = map_node.clone();
                async move {
                    let item_scope = map_item_scoped_input(&scoped_input, &item, index);
                    let (output, retries) = self
                        .execute_tool_with_policy_for_scope(ToolPolicyRequest {
                            step,
                            node: &map_node,
                            tool: spec.tool,
                            input: &item,
                            executor,
                            scoped_input: item_scope,
                            cancellation,
                        })
                        .await?;
                    Ok::<(Value, Vec<WorkflowRetryEvent>), WorkflowRuntimeError>((output, retries))
                }
            })
            .await?;

        let mut outputs = Vec::with_capacity(mapped.len());
        for (output, local_retries) in mapped {
            outputs.push(output);
            retry_events.extend(local_retries);
        }

        let output = Value::Array(outputs);
        scope
            .record_node_output(&node.id, output.clone(), ScopeCapability::MapWrite)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        Ok(NodeExecution {
            step,
            node_id: node.id.clone(),
            data: NodeExecutionData::Map {
                item_count: output.as_array().map_or(0, Vec::len),
                output,
                next: spec.next.to_string(),
            },
        })
    }

    fn execute_reduce_node(
        &self,
        step: usize,
        node: &Node,
        source: &str,
        operation: &ReduceOperation,
        next: &str,
        scope: &mut RuntimeScope,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let source_value = scope.node_output(source).cloned().ok_or_else(|| {
            WorkflowRuntimeError::MissingMergeSource {
                node_id: node.id.clone(),
                source_id: source.to_string(),
            }
        })?;

        let reduced = reduce_value(&node.id, source, operation, source_value)?;
        scope
            .record_node_output(&node.id, reduced.clone(), ScopeCapability::ReduceWrite)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        Ok(NodeExecution {
            step,
            node_id: node.id.clone(),
            data: NodeExecutionData::Reduce {
                operation: operation.clone(),
                output: reduced,
                next: next.to_string(),
            },
        })
    }

    fn execute_batch_node(
        &self,
        step: usize,
        node: &Node,
        items_path: &str,
        next: &str,
        scope: &mut RuntimeScope,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let scoped = scope
            .scoped_input(ScopeCapability::MapRead)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;
        let items = resolve_path(&scoped, items_path).ok_or_else(|| {
            WorkflowRuntimeError::BatchItemsNotArray {
                node_id: node.id.clone(),
                items_path: items_path.to_string(),
            }
        })?;
        let array = items
            .as_array()
            .ok_or_else(|| WorkflowRuntimeError::BatchItemsNotArray {
                node_id: node.id.clone(),
                items_path: items_path.to_string(),
            })?;

        let output = Value::Array(array.clone());
        scope
            .record_node_output(&node.id, output.clone(), ScopeCapability::MapWrite)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        Ok(NodeExecution {
            step,
            node_id: node.id.clone(),
            data: NodeExecutionData::Batch {
                items_path: items_path.to_string(),
                item_count: array.len(),
                next: next.to_string(),
            },
        })
    }

    fn execute_filter_node(
        &self,
        step: usize,
        node: &Node,
        items_path: &str,
        expression: &str,
        next: &str,
        scope: &mut RuntimeScope,
    ) -> Result<NodeExecution, WorkflowRuntimeError> {
        let scoped = scope
            .scoped_input(ScopeCapability::MapRead)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;
        let items_value = resolve_path(&scoped, items_path).ok_or_else(|| {
            WorkflowRuntimeError::FilterItemsNotArray {
                node_id: node.id.clone(),
                items_path: items_path.to_string(),
            }
        })?;
        let array =
            items_value
                .as_array()
                .ok_or_else(|| WorkflowRuntimeError::FilterItemsNotArray {
                    node_id: node.id.clone(),
                    items_path: items_path.to_string(),
                })?;
        if array.len() > self.options.security_limits.max_filter_items {
            return Err(WorkflowRuntimeError::FilterItemLimitExceeded {
                node_id: node.id.clone(),
                actual_items: array.len(),
                max_items: self.options.security_limits.max_filter_items,
            });
        }

        let mut kept = Vec::new();
        for (index, item) in array.iter().enumerate() {
            let mut eval_scope = scoped.clone();
            if let Some(object) = eval_scope.as_object_mut() {
                object.insert("item".to_string(), item.clone());
                object.insert("item_index".to_string(), Value::from(index as u64));
            }
            enforce_expression_scope_budget(
                &node.id,
                &eval_scope,
                self.options.security_limits.max_expression_scope_bytes,
            )?;
            let include =
                expressions::evaluate_bool(expression, &eval_scope).map_err(|reason| {
                    WorkflowRuntimeError::InvalidFilterExpression {
                        node_id: node.id.clone(),
                        expression: expression.to_string(),
                        reason: reason.to_string(),
                    }
                })?;
            if include {
                kept.push(item.clone());
            }
        }
        let output = Value::Array(kept.clone());
        scope
            .record_node_output(&node.id, output, ScopeCapability::MapWrite)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        Ok(NodeExecution {
            step,
            node_id: node.id.clone(),
            data: NodeExecutionData::Filter {
                items_path: items_path.to_string(),
                expression: expression.to_string(),
                kept: kept.len(),
                next: next.to_string(),
            },
        })
    }

    async fn execute_llm_with_policy(
        &self,
        step: usize,
        node: &Node,
        model: &str,
        prompt: &str,
        scope: &RuntimeScope,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<(LlmExecutionOutput, Vec<WorkflowRetryEvent>), WorkflowRuntimeError> {
        let scoped_input = scope
            .scoped_input(ScopeCapability::LlmRead)
            .map_err(|source| WorkflowRuntimeError::ScopeAccess {
                node_id: node.id.clone(),
                source,
            })?;

        self.execute_llm_with_policy_for_scope(
            step,
            node,
            model,
            prompt,
            scoped_input,
            cancellation,
        )
        .await
    }

    async fn execute_parallel_branch(
        &self,
        step: usize,
        parallel_node_id: &str,
        branch_node: &Node,
        scoped_input: Value,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<(String, Value, Vec<WorkflowRetryEvent>), WorkflowRuntimeError> {
        match &branch_node.kind {
            NodeKind::Llm {
                model,
                prompt,
                next: _,
            } => {
                let (output, retries) = self
                    .execute_llm_with_policy_for_scope(
                        step,
                        branch_node,
                        model,
                        prompt,
                        scoped_input,
                        cancellation,
                    )
                    .await?;
                Ok((
                    branch_node.id.clone(),
                    Value::String(output.content),
                    retries,
                ))
            }
            NodeKind::Tool { tool, input, .. } => {
                let executor = self.tool_executor.ok_or_else(|| {
                    WorkflowRuntimeError::MissingToolExecutor {
                        node_id: branch_node.id.clone(),
                    }
                })?;
                let (output, retries) = self
                    .execute_tool_with_policy_for_scope(ToolPolicyRequest {
                        step,
                        node: branch_node,
                        tool,
                        input,
                        executor,
                        scoped_input,
                        cancellation,
                    })
                    .await?;
                Ok((branch_node.id.clone(), output, retries))
            }
            _ => Err(WorkflowRuntimeError::ParallelBranchUnsupported {
                node_id: parallel_node_id.to_string(),
                branch_id: branch_node.id.clone(),
                reason: "only llm/tool branches are supported".to_string(),
            }),
        }
    }

    async fn execute_llm_with_policy_for_scope(
        &self,
        step: usize,
        node: &Node,
        model: &str,
        prompt: &str,
        scoped_input: Value,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<(LlmExecutionOutput, Vec<WorkflowRetryEvent>), WorkflowRuntimeError> {
        let max_attempts = self.options.llm_node_policy.max_retries.saturating_add(1);
        let mut retry_events = Vec::new();

        for attempt in 1..=max_attempts {
            check_cancelled(cancellation)?;

            let execution = self.llm_executor.execute(LlmExecutionInput {
                node_id: node.id.clone(),
                model: model.to_string(),
                prompt: prompt.to_string(),
                scoped_input: scoped_input.clone(),
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
                        retry_events.push(WorkflowRetryEvent {
                            step,
                            node_id: node.id.clone(),
                            operation: "llm".to_string(),
                            failed_attempt: attempt,
                            reason: format!(
                                "attempt {} timed out after {} ms",
                                attempt,
                                timeout_duration.as_millis()
                            ),
                        });
                        check_cancelled(cancellation)?;
                        continue;
                    }
                }
            } else {
                execution.await
            };

            match outcome {
                Ok(output) => return Ok((output, retry_events)),
                Err(last_error) => {
                    if attempt == max_attempts {
                        return Err(WorkflowRuntimeError::LlmRetryExhausted {
                            node_id: node.id.clone(),
                            attempts: attempt,
                            last_error,
                        });
                    }
                    retry_events.push(WorkflowRetryEvent {
                        step,
                        node_id: node.id.clone(),
                        operation: "llm".to_string(),
                        failed_attempt: attempt,
                        reason: last_error.to_string(),
                    });
                    check_cancelled(cancellation)?;
                }
            }
        }

        unreachable!("llm attempts loop always returns")
    }

    async fn execute_tool_with_policy_for_scope(
        &self,
        request: ToolPolicyRequest<'_>,
    ) -> Result<(Value, Vec<WorkflowRetryEvent>), WorkflowRuntimeError> {
        let ToolPolicyRequest {
            step,
            node,
            tool,
            input,
            executor,
            scoped_input,
            cancellation,
        } = request;
        let max_attempts = self.options.tool_node_policy.max_retries.saturating_add(1);
        let mut retry_events = Vec::new();

        for attempt in 1..=max_attempts {
            check_cancelled(cancellation)?;

            let execution = executor.execute_tool(ToolExecutionInput {
                node_id: node.id.clone(),
                tool: tool.to_string(),
                input: input.clone(),
                scoped_input: scoped_input.clone(),
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
                        retry_events.push(WorkflowRetryEvent {
                            step,
                            node_id: node.id.clone(),
                            operation: "tool".to_string(),
                            failed_attempt: attempt,
                            reason: format!(
                                "attempt {} timed out after {} ms",
                                attempt,
                                timeout_duration.as_millis()
                            ),
                        });
                        check_cancelled(cancellation)?;
                        continue;
                    }
                }
            } else {
                execution.await
            };

            match outcome {
                Ok(output) => return Ok((output, retry_events)),
                Err(last_error) => {
                    if attempt == max_attempts {
                        return Err(WorkflowRuntimeError::ToolRetryExhausted {
                            node_id: node.id.clone(),
                            attempts: attempt,
                            last_error,
                        });
                    }
                    retry_events.push(WorkflowRetryEvent {
                        step,
                        node_id: node.id.clone(),
                        operation: "tool".to_string(),
                        failed_attempt: attempt,
                        reason: last_error.to_string(),
                    });
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
    loop_iterations: HashMap<String, u32>,
    debounce_last_seen: HashMap<String, usize>,
    throttle_last_pass: HashMap<String, usize>,
    cache_entries: BTreeMap<String, Value>,
    last_llm_output: Option<String>,
    last_tool_output: Option<Value>,
}

struct ToolPolicyRequest<'a> {
    step: usize,
    node: &'a Node,
    tool: &'a str,
    input: &'a Value,
    executor: &'a dyn ToolExecutor,
    scoped_input: Value,
    cancellation: Option<&'a dyn CancellationSignal>,
}

struct LlmNodeSpec<'a> {
    model: &'a str,
    prompt: &'a str,
    next: &'a Option<String>,
}

struct ToolNodeSpec<'a> {
    tool: &'a str,
    input: &'a Value,
    next: &'a Option<String>,
}

struct ConditionNodeSpec<'a> {
    expression: &'a str,
    on_true: &'a str,
    on_false: &'a str,
}

struct ParallelNodeSpec<'a> {
    node_index: &'a HashMap<&'a str, &'a Node>,
    branches: &'a [String],
    next: &'a str,
    max_in_flight: Option<usize>,
}

struct MergeNodeSpec<'a> {
    sources: &'a [String],
    policy: &'a MergePolicy,
    quorum: Option<usize>,
    next: &'a str,
}

struct MapNodeSpec<'a> {
    tool: &'a str,
    items_path: &'a str,
    next: &'a str,
    max_in_flight: Option<usize>,
}

impl RuntimeScope {
    fn new(workflow_input: Value) -> Self {
        Self {
            workflow_input,
            node_outputs: BTreeMap::new(),
            loop_iterations: HashMap::new(),
            debounce_last_seen: HashMap::new(),
            throttle_last_pass: HashMap::new(),
            cache_entries: BTreeMap::new(),
            last_llm_output: None,
            last_tool_output: None,
        }
    }

    fn scoped_input(&self, capability: ScopeCapability) -> Result<Value, ScopeAccessError> {
        if !matches!(
            capability,
            ScopeCapability::LlmRead
                | ScopeCapability::ToolRead
                | ScopeCapability::ConditionRead
                | ScopeCapability::MapRead
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

    fn record_node_output(
        &mut self,
        node_id: &str,
        output: Value,
        capability: ScopeCapability,
    ) -> Result<(), ScopeAccessError> {
        if !matches!(
            capability,
            ScopeCapability::MapWrite | ScopeCapability::ReduceWrite
        ) {
            return Err(ScopeAccessError::WriteDenied {
                capability: capability.as_str(),
            });
        }

        self.node_outputs.insert(node_id.to_string(), output);
        Ok(())
    }

    fn node_output(&self, node_id: &str) -> Option<&Value> {
        self.node_outputs.get(node_id)
    }

    fn loop_iteration(&self, node_id: &str) -> u32 {
        self.loop_iterations.get(node_id).copied().unwrap_or(0)
    }

    fn set_loop_iteration(&mut self, node_id: &str, iteration: u32) {
        self.loop_iterations.insert(node_id.to_string(), iteration);
    }

    fn clear_loop_iteration(&mut self, node_id: &str) {
        self.loop_iterations.remove(node_id);
    }

    fn debounce(&mut self, node_id: &str, key: &str, step: usize, window_steps: u32) -> bool {
        let namespaced = format!("{node_id}:{key}");
        let window = window_steps as usize;
        let suppressed = self
            .debounce_last_seen
            .get(&namespaced)
            .is_some_and(|last| step.saturating_sub(*last) < window);
        self.debounce_last_seen.insert(namespaced, step);
        suppressed
    }

    fn throttle(&mut self, node_id: &str, key: &str, step: usize, window_steps: u32) -> bool {
        let namespaced = format!("{node_id}:{key}");
        let window = window_steps as usize;
        let throttled = self
            .throttle_last_pass
            .get(&namespaced)
            .is_some_and(|last| step.saturating_sub(*last) < window);
        if !throttled {
            self.throttle_last_pass.insert(namespaced, step);
        }
        throttled
    }

    fn put_cache(&mut self, key: &str, value: Value) {
        self.cache_entries.insert(key.to_string(), value);
    }

    fn cache_value(&self, key: &str) -> Option<&Value> {
        self.cache_entries.get(key)
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
        | NodeExecutionData::Condition { next, .. }
        | NodeExecutionData::Debounce { next, .. }
        | NodeExecutionData::Throttle { next, .. }
        | NodeExecutionData::RetryCompensate { next, .. }
        | NodeExecutionData::HumanInTheLoop { next, .. }
        | NodeExecutionData::CacheRead { next, .. }
        | NodeExecutionData::CacheWrite { next, .. }
        | NodeExecutionData::EventTrigger { next, .. }
        | NodeExecutionData::Router { next, .. }
        | NodeExecutionData::Transform { next, .. }
        | NodeExecutionData::Loop { next, .. }
        | NodeExecutionData::Subgraph { next, .. }
        | NodeExecutionData::Batch { next, .. }
        | NodeExecutionData::Filter { next, .. }
        | NodeExecutionData::Parallel { next, .. }
        | NodeExecutionData::Merge { next, .. }
        | NodeExecutionData::Map { next, .. }
        | NodeExecutionData::Reduce { next, .. } => Some(next.clone()),
        NodeExecutionData::End => None,
    }
}

fn resolve_path<'a>(scope: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = scope;
    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        current = current.get(segment)?;
    }
    Some(current)
}

fn resolve_string_path(scope: &Value, path: &str) -> Option<String> {
    resolve_path(scope, path)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn evaluate_human_decision(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "approve" | "approved" | "yes" | "true" => Some(true),
                "reject" | "rejected" | "no" | "false" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

fn evaluate_transform_expression(expression: &str, scope: &Value) -> Result<Value, String> {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        return Err("expression is empty".to_string());
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }

    let path = trimmed.strip_prefix("$.").unwrap_or(trimmed);
    resolve_path(scope, path)
        .cloned()
        .ok_or_else(|| format!("path '{path}' not found in scoped input"))
}

fn map_item_scoped_input(base_scope: &Value, item: &Value, index: usize) -> Value {
    let mut object = match base_scope {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    object.insert("map_item".to_string(), item.clone());
    object.insert("map_index".to_string(), Value::from(index as u64));
    Value::Object(object)
}

fn enforce_expression_scope_budget(
    node_id: &str,
    scoped_input: &Value,
    max_expression_scope_bytes: usize,
) -> Result<(), WorkflowRuntimeError> {
    let size = serde_json::to_vec(scoped_input)
        .map(|bytes| bytes.len())
        .unwrap_or(max_expression_scope_bytes.saturating_add(1));
    if size > max_expression_scope_bytes {
        return Err(WorkflowRuntimeError::ExpressionScopeLimitExceeded {
            node_id: node_id.to_string(),
            actual_bytes: size,
            limit_bytes: max_expression_scope_bytes,
        });
    }
    Ok(())
}

fn reduce_value(
    node_id: &str,
    source: &str,
    operation: &ReduceOperation,
    source_value: Value,
) -> Result<Value, WorkflowRuntimeError> {
    let items =
        source_value
            .as_array()
            .ok_or_else(|| WorkflowRuntimeError::InvalidReduceInput {
                node_id: node_id.to_string(),
                source_node: source.to_string(),
                reason: "expected source output to be an array".to_string(),
            })?;

    match operation {
        ReduceOperation::Count => Ok(Value::from(items.len() as u64)),
        ReduceOperation::Sum => {
            let mut sum = 0.0f64;
            for value in items {
                let number =
                    value
                        .as_f64()
                        .ok_or_else(|| WorkflowRuntimeError::InvalidReduceInput {
                            node_id: node_id.to_string(),
                            source_node: source.to_string(),
                            reason: "sum operation requires numeric array values".to_string(),
                        })?;
                sum += number;
            }
            let number = serde_json::Number::from_f64(sum).ok_or_else(|| {
                WorkflowRuntimeError::InvalidReduceInput {
                    node_id: node_id.to_string(),
                    source_node: source.to_string(),
                    reason: "sum produced non-finite value".to_string(),
                }
            })?;
            Ok(Value::Number(number))
        }
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
    use crate::ir::{MergePolicy, Node, NodeKind, ReduceOperation, WorkflowDefinition};

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

    struct IncrementingToolExecutor {
        value: AtomicUsize,
    }

    #[async_trait]
    impl ToolExecutor for IncrementingToolExecutor {
        async fn execute_tool(
            &self,
            _input: ToolExecutionInput,
        ) -> Result<Value, ToolExecutionError> {
            let next = self.value.fetch_add(1, Ordering::Relaxed) + 1;
            Ok(json!(next))
        }
    }

    struct EchoInputToolExecutor;

    #[async_trait]
    impl ToolExecutor for EchoInputToolExecutor {
        async fn execute_tool(
            &self,
            input: ToolExecutionInput,
        ) -> Result<Value, ToolExecutionError> {
            Ok(input.input)
        }
    }

    struct RetryCompensateToolExecutor {
        attempts: AtomicUsize,
    }

    #[async_trait]
    impl ToolExecutor for RetryCompensateToolExecutor {
        async fn execute_tool(
            &self,
            input: ToolExecutionInput,
        ) -> Result<Value, ToolExecutionError> {
            match input.tool.as_str() {
                "unstable_primary" => {
                    let current = self.attempts.fetch_add(1, Ordering::Relaxed) + 1;
                    if current <= 1 {
                        Err(ToolExecutionError::Failed("primary failed".to_string()))
                    } else {
                        Ok(json!({"primary_attempt": current}))
                    }
                }
                "always_fail" => Err(ToolExecutionError::Failed("always fail".to_string())),
                "compensate" => Ok(json!({"compensated": true})),
                _ => Err(ToolExecutionError::NotFound {
                    tool: input.tool.clone(),
                }),
            }
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

    fn loop_workflow(max_iterations: Option<u32>) -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "loop-workflow".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "loop".to_string(),
                    },
                },
                Node {
                    id: "loop".to_string(),
                    kind: NodeKind::Loop {
                        condition: "last_tool_output != 3".to_string(),
                        body: "counter".to_string(),
                        next: "end".to_string(),
                        max_iterations,
                    },
                },
                Node {
                    id: "counter".to_string(),
                    kind: NodeKind::Tool {
                        tool: "counter".to_string(),
                        input: json!({}),
                        next: Some("loop".to_string()),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    fn parallel_merge_workflow(policy: MergePolicy, quorum: Option<usize>) -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "parallel-merge".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "parallel".to_string(),
                    },
                },
                Node {
                    id: "parallel".to_string(),
                    kind: NodeKind::Parallel {
                        branches: vec!["tool_a".to_string(), "tool_b".to_string()],
                        next: "merge".to_string(),
                        max_in_flight: Some(2),
                    },
                },
                Node {
                    id: "tool_a".to_string(),
                    kind: NodeKind::Tool {
                        tool: "extract".to_string(),
                        input: json!({"value": 1}),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "tool_b".to_string(),
                    kind: NodeKind::Tool {
                        tool: "extract".to_string(),
                        input: json!({"value": 2}),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "merge".to_string(),
                    kind: NodeKind::Merge {
                        sources: vec!["tool_a".to_string(), "tool_b".to_string()],
                        policy,
                        quorum,
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    fn map_reduce_workflow(operation: ReduceOperation) -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "map-reduce".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "map".to_string(),
                    },
                },
                Node {
                    id: "map".to_string(),
                    kind: NodeKind::Map {
                        tool: "counter".to_string(),
                        items_path: "input.values".to_string(),
                        next: "reduce".to_string(),
                        max_in_flight: Some(3),
                    },
                },
                Node {
                    id: "reduce".to_string(),
                    kind: NodeKind::Reduce {
                        source: "map".to_string(),
                        operation,
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    fn debounce_and_throttle_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "debounce-throttle".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "debounce_a".to_string(),
                    },
                },
                Node {
                    id: "debounce_a".to_string(),
                    kind: NodeKind::Debounce {
                        key_path: "input.key".to_string(),
                        window_steps: 3,
                        next: "debounce_a".to_string(),
                        on_suppressed: Some("throttle_a".to_string()),
                    },
                },
                Node {
                    id: "throttle_a".to_string(),
                    kind: NodeKind::Throttle {
                        key_path: "input.key".to_string(),
                        window_steps: 3,
                        next: "throttle_a".to_string(),
                        on_throttled: Some("end_throttled".to_string()),
                    },
                },
                Node {
                    id: "end_throttled".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    fn extended_nodes_workflow() -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "extended-nodes".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "event".to_string(),
                    },
                },
                Node {
                    id: "event".to_string(),
                    kind: NodeKind::EventTrigger {
                        event: "webhook".to_string(),
                        event_path: "input.event_type".to_string(),
                        next: "cache_write".to_string(),
                        on_mismatch: Some("end_mismatch".to_string()),
                    },
                },
                Node {
                    id: "cache_write".to_string(),
                    kind: NodeKind::CacheWrite {
                        key_path: "input.cache_key".to_string(),
                        value_path: "input.payload".to_string(),
                        next: "cache_read".to_string(),
                    },
                },
                Node {
                    id: "cache_read".to_string(),
                    kind: NodeKind::CacheRead {
                        key_path: "input.cache_key".to_string(),
                        next: "router".to_string(),
                        on_miss: Some("end_miss".to_string()),
                    },
                },
                Node {
                    id: "router".to_string(),
                    kind: NodeKind::Router {
                        routes: vec![crate::ir::RouterRoute {
                            when: "input.mode == 'manual'".to_string(),
                            next: "human".to_string(),
                        }],
                        default: "transform".to_string(),
                    },
                },
                Node {
                    id: "human".to_string(),
                    kind: NodeKind::HumanInTheLoop {
                        decision_path: "input.approval".to_string(),
                        response_path: Some("input.review_notes".to_string()),
                        on_approve: "transform".to_string(),
                        on_reject: "end_rejected".to_string(),
                    },
                },
                Node {
                    id: "transform".to_string(),
                    kind: NodeKind::Transform {
                        expression: "node_outputs.cache_read.value".to_string(),
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "end_mismatch".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "end_miss".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "end_rejected".to_string(),
                    kind: NodeKind::End,
                },
            ],
        }
    }

    fn retry_compensate_workflow(primary_tool: &str) -> WorkflowDefinition {
        WorkflowDefinition {
            version: "v0".to_string(),
            name: "retry-compensate".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "retry_comp".to_string(),
                    },
                },
                Node {
                    id: "retry_comp".to_string(),
                    kind: NodeKind::RetryCompensate {
                        tool: primary_tool.to_string(),
                        input: json!({"job": "run"}),
                        max_retries: 1,
                        compensate_tool: "compensate".to_string(),
                        compensate_input: json!({"job": "rollback"}),
                        next: "end".to_string(),
                        on_compensated: Some("end_compensated".to_string()),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "end_compensated".to_string(),
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
        assert!(result.retry_events.is_empty());
        assert!(result.trace.is_some());
        assert_eq!(result.replay_report, None);
    }

    #[tokio::test]
    async fn linear_flow_records_expected_node_execution_payloads() {
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

        assert!(matches!(
            result.node_executions[0].data,
            NodeExecutionData::Start { ref next } if next == "llm"
        ));
        assert!(matches!(
            result.node_executions[1].data,
            NodeExecutionData::Llm {
                ref model,
                ref output,
                ref next
            } if model == "gpt-4" && output == "ok" && next == "tool"
        ));
        assert!(matches!(
            result.node_executions[2].data,
            NodeExecutionData::Tool {
                ref tool,
                ref next,
                ..
            } if tool == "extract" && next == "end"
        ));
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
    async fn executes_conditional_false_branch() {
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "conditional-false".to_string(),
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
            .execute(json!({"approved": false}), None)
            .await
            .expect("conditional workflow should take false branch");

        assert_eq!(result.terminal_node_id, "end_false");
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
        assert_eq!(result.retry_events.len(), 1);
        assert_eq!(result.retry_events[0].operation, "llm");
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

    #[tokio::test]
    async fn executes_loop_until_condition_fails() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let counter = IncrementingToolExecutor {
            value: AtomicUsize::new(0),
        };
        let runtime = WorkflowRuntime::new(
            loop_workflow(Some(8)),
            &llm,
            Some(&counter),
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(json!({}), None)
            .await
            .expect("loop workflow should terminate at end");

        assert_eq!(result.terminal_node_id, "end");
        assert!(result.node_executions.iter().any(|step| matches!(
            step.data,
            NodeExecutionData::Loop {
                evaluated: false,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn fails_when_loop_exceeds_max_iterations() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let counter = MockToolExecutor {
            output: json!(0),
            fail: false,
        };
        let runtime = WorkflowRuntime::new(
            loop_workflow(Some(2)),
            &llm,
            Some(&counter),
            WorkflowRuntimeOptions {
                max_steps: 20,
                ..WorkflowRuntimeOptions::default()
            },
        );

        let error = runtime
            .execute(json!({}), None)
            .await
            .expect_err("loop should fail once max iterations are exceeded");

        assert!(matches!(
            error,
            WorkflowRuntimeError::LoopIterationLimitExceeded {
                node_id,
                max_iterations: 2
            } if node_id == "loop"
        ));
    }

    #[tokio::test]
    async fn executes_parallel_then_merge_all() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let tool = EchoInputToolExecutor;
        let runtime = WorkflowRuntime::new(
            parallel_merge_workflow(MergePolicy::All, None),
            &llm,
            Some(&tool),
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(json!({}), None)
            .await
            .expect("parallel merge workflow should succeed");

        assert_eq!(result.terminal_node_id, "end");
        assert_eq!(
            result.node_outputs.get("tool_a"),
            Some(&json!({"value": 1}))
        );
        assert_eq!(
            result.node_outputs.get("tool_b"),
            Some(&json!({"value": 2}))
        );
        assert_eq!(
            result.node_outputs.get("merge"),
            Some(&json!([{"value": 1}, {"value": 2}]))
        );
    }

    #[tokio::test]
    async fn executes_map_reduce_sum() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let tool = EchoInputToolExecutor;
        let runtime = WorkflowRuntime::new(
            map_reduce_workflow(ReduceOperation::Sum),
            &llm,
            Some(&tool),
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(json!({"values": [1, 2, 3]}), None)
            .await
            .expect("map reduce workflow should succeed");

        assert_eq!(result.node_outputs.get("map"), Some(&json!([1, 2, 3])));
        assert_eq!(result.node_outputs.get("reduce"), Some(&json!(6.0)));
    }

    #[tokio::test]
    async fn fails_map_when_items_path_is_not_array() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let tool = EchoInputToolExecutor;
        let runtime = WorkflowRuntime::new(
            map_reduce_workflow(ReduceOperation::Count),
            &llm,
            Some(&tool),
            WorkflowRuntimeOptions::default(),
        );

        let error = runtime
            .execute(json!({"values": {"not": "array"}}), None)
            .await
            .expect_err("map node should fail on non-array path");

        assert!(matches!(
            error,
            WorkflowRuntimeError::MapItemsNotArray {
                node_id,
                items_path
            } if node_id == "map" && items_path == "input.values"
        ));
    }

    #[tokio::test]
    async fn executes_subgraph_via_registry() {
        let llm = MockLlmExecutor {
            output: "nested-ok".to_string(),
        };
        let subgraph = WorkflowDefinition {
            version: "v0".to_string(),
            name: "child".to_string(),
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
                        prompt: "child".to_string(),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };
        let parent = WorkflowDefinition {
            version: "v0".to_string(),
            name: "parent".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "sub".to_string(),
                    },
                },
                Node {
                    id: "sub".to_string(),
                    kind: NodeKind::Subgraph {
                        graph: "child_graph".to_string(),
                        next: Some("end".to_string()),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let mut registry = BTreeMap::new();
        registry.insert("child_graph".to_string(), subgraph);
        let runtime = WorkflowRuntime::new(
            parent,
            &llm,
            None,
            WorkflowRuntimeOptions {
                subgraph_registry: registry,
                ..WorkflowRuntimeOptions::default()
            },
        );

        let result = runtime
            .execute(json!({"approved": true}), None)
            .await
            .expect("subgraph workflow should execute");

        assert_eq!(result.terminal_node_id, "end");
        assert!(matches!(
            result.node_executions[1].data,
            NodeExecutionData::Subgraph { .. }
        ));
    }

    #[tokio::test]
    async fn executes_batch_and_filter_nodes() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "batch-filter".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "batch".to_string(),
                    },
                },
                Node {
                    id: "batch".to_string(),
                    kind: NodeKind::Batch {
                        items_path: "input.items".to_string(),
                        next: "filter".to_string(),
                    },
                },
                Node {
                    id: "filter".to_string(),
                    kind: NodeKind::Filter {
                        items_path: "node_outputs.batch".to_string(),
                        expression: "item.keep == true".to_string(),
                        next: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let runtime = WorkflowRuntime::new(workflow, &llm, None, WorkflowRuntimeOptions::default());
        let result = runtime
            .execute(
                json!({
                    "items": [
                        {"id": 1, "keep": true},
                        {"id": 2, "keep": false},
                        {"id": 3, "keep": true}
                    ]
                }),
                None,
            )
            .await
            .expect("batch/filter workflow should execute");

        assert_eq!(
            result
                .node_outputs
                .get("batch")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
        assert_eq!(
            result
                .node_outputs
                .get("filter")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[tokio::test]
    async fn executes_debounce_and_throttle_nodes() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let runtime = WorkflowRuntime::new(
            debounce_and_throttle_workflow(),
            &llm,
            None,
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(json!({"key": "k1"}), None)
            .await
            .expect("debounce/throttle workflow should execute");

        assert_eq!(result.terminal_node_id, "end_throttled");
        assert_eq!(
            result.node_outputs.get("debounce_a"),
            Some(&json!({"key": "k1", "suppressed": true}))
        );
        assert_eq!(
            result.node_outputs.get("throttle_a"),
            Some(&json!({"key": "k1", "throttled": true}))
        );
    }

    #[tokio::test]
    async fn executes_retry_compensate_successfully_without_compensation() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let tools = RetryCompensateToolExecutor {
            attempts: AtomicUsize::new(0),
        };
        let runtime = WorkflowRuntime::new(
            retry_compensate_workflow("unstable_primary"),
            &llm,
            Some(&tools),
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(json!({}), None)
            .await
            .expect("retry_compensate should recover by retry");

        assert_eq!(result.terminal_node_id, "end");
        assert_eq!(tools.attempts.load(Ordering::Relaxed), 2);
        assert_eq!(result.retry_events.len(), 1);
    }

    #[tokio::test]
    async fn executes_retry_compensate_with_compensation_route() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let tools = RetryCompensateToolExecutor {
            attempts: AtomicUsize::new(0),
        };
        let runtime = WorkflowRuntime::new(
            retry_compensate_workflow("always_fail"),
            &llm,
            Some(&tools),
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(json!({}), None)
            .await
            .expect("retry_compensate should use compensation");

        assert_eq!(result.terminal_node_id, "end_compensated");
        assert_eq!(result.retry_events.len(), 1);
        assert_eq!(
            result
                .node_outputs
                .get("retry_comp")
                .and_then(|value| value.get("status")),
            Some(&json!("compensated"))
        );
    }

    #[tokio::test]
    async fn executes_event_cache_router_human_transform_nodes() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let runtime = WorkflowRuntime::new(
            extended_nodes_workflow(),
            &llm,
            None,
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(
                json!({
                    "event_type": "webhook",
                    "cache_key": "customer-1",
                    "payload": {"value": 99},
                    "mode": "manual",
                    "approval": "approve",
                    "review_notes": {"editor": "ops"}
                }),
                None,
            )
            .await
            .expect("extended nodes workflow should execute");

        assert_eq!(result.terminal_node_id, "end");
        assert_eq!(
            result.node_outputs.get("cache_read"),
            Some(&json!({"key": "customer-1", "hit": true, "value": {"value": 99}}))
        );
        assert_eq!(
            result.node_outputs.get("router"),
            Some(&json!({"selected": "human"}))
        );
        assert_eq!(
            result.node_outputs.get("transform"),
            Some(&json!({"value": 99}))
        );
    }

    #[tokio::test]
    async fn routes_event_trigger_mismatch_to_fallback() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let runtime = WorkflowRuntime::new(
            extended_nodes_workflow(),
            &llm,
            None,
            WorkflowRuntimeOptions::default(),
        );

        let result = runtime
            .execute(
                json!({
                    "event_type": "cron",
                    "cache_key": "customer-1",
                    "payload": {"value": 99},
                    "mode": "manual",
                    "approval": "approve"
                }),
                None,
            )
            .await
            .expect("event mismatch should route to fallback");

        assert_eq!(result.terminal_node_id, "end_mismatch");
    }

    #[tokio::test]
    async fn cache_read_routes_to_on_miss_when_value_absent() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "cache-read-miss".to_string(),
            nodes: vec![
                Node {
                    id: "start".to_string(),
                    kind: NodeKind::Start {
                        next: "cache_read".to_string(),
                    },
                },
                Node {
                    id: "cache_read".to_string(),
                    kind: NodeKind::CacheRead {
                        key_path: "input.cache_key".to_string(),
                        next: "end_hit".to_string(),
                        on_miss: Some("end_miss".to_string()),
                    },
                },
                Node {
                    id: "end_hit".to_string(),
                    kind: NodeKind::End,
                },
                Node {
                    id: "end_miss".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };
        let runtime = WorkflowRuntime::new(workflow, &llm, None, WorkflowRuntimeOptions::default());

        let result = runtime
            .execute(json!({"cache_key": "new-customer"}), None)
            .await
            .expect("cache read miss should still execute via on_miss route");

        assert_eq!(
            result.node_outputs.get("cache_read"),
            Some(&json!({"key": "new-customer", "hit": false, "value": null}))
        );
        assert_eq!(result.terminal_node_id, "end_miss");
    }

    #[tokio::test]
    async fn rejects_condition_when_expression_scope_exceeds_limit() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let workflow = WorkflowDefinition {
            version: "v0".to_string(),
            name: "scope-limit".to_string(),
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
                        expression: "input.flag == true".to_string(),
                        on_true: "end".to_string(),
                        on_false: "end".to_string(),
                    },
                },
                Node {
                    id: "end".to_string(),
                    kind: NodeKind::End,
                },
            ],
        };

        let runtime = WorkflowRuntime::new(
            workflow,
            &llm,
            None,
            WorkflowRuntimeOptions {
                security_limits: RuntimeSecurityLimits {
                    max_expression_scope_bytes: 32,
                    ..RuntimeSecurityLimits::default()
                },
                ..WorkflowRuntimeOptions::default()
            },
        );

        let error = runtime
            .execute(
                json!({"flag": true, "payload": "this-is-too-large-for-limit"}),
                None,
            )
            .await
            .expect_err("condition should fail when scope budget is exceeded");
        assert!(matches!(
            error,
            WorkflowRuntimeError::ExpressionScopeLimitExceeded { node_id, .. }
                if node_id == "condition"
        ));
    }

    #[tokio::test]
    async fn rejects_map_when_item_count_exceeds_limit() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let tool = EchoInputToolExecutor;
        let runtime = WorkflowRuntime::new(
            map_reduce_workflow(ReduceOperation::Count),
            &llm,
            Some(&tool),
            WorkflowRuntimeOptions {
                security_limits: RuntimeSecurityLimits {
                    max_map_items: 2,
                    ..RuntimeSecurityLimits::default()
                },
                ..WorkflowRuntimeOptions::default()
            },
        );

        let error = runtime
            .execute(json!({"values": [1, 2, 3]}), None)
            .await
            .expect_err("map should fail when item guard is exceeded");
        assert!(matches!(
            error,
            WorkflowRuntimeError::MapItemLimitExceeded {
                node_id,
                actual_items: 3,
                max_items: 2,
            } if node_id == "map"
        ));
    }

    #[tokio::test]
    async fn resumes_from_checkpoint() {
        let llm = MockLlmExecutor {
            output: "unused".to_string(),
        };
        let tool = MockToolExecutor {
            output: json!({"ok": true}),
            fail: false,
        };
        let runtime = WorkflowRuntime::new(
            linear_workflow(),
            &llm,
            Some(&tool),
            WorkflowRuntimeOptions::default(),
        );

        let checkpoint = WorkflowCheckpoint {
            run_id: "run-1".to_string(),
            workflow_name: "linear".to_string(),
            step: 2,
            next_node_id: "tool".to_string(),
            scope_snapshot: json!({"input": {"request_id": "r-resume"}}),
        };

        let resumed = runtime
            .execute_resume_from_failure(&checkpoint, None)
            .await
            .expect("resume should continue from checkpoint node");
        assert_eq!(resumed.terminal_node_id, "end");
        assert_eq!(resumed.node_executions[0].node_id, "tool");
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
