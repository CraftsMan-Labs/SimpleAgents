//! Workflow IR and validation primitives for SimpleAgents.
//!
//! This crate currently provides:
//! - A minimal canonical workflow IR (`start`, `llm`, `tool`, `condition`, `loop`, `end`)
//! - Deterministic normalization helpers
//! - Structural validation with actionable diagnostics
//! - Runtime execution for the minimal node set
//! - In-process worker protocol and bounded worker pool primitives
//! - Worker pool adapter hook for external (for example gRPC) worker clients
//!
//! # Example
//!
//! ```rust
//! use simple_agents_workflow::{Node, NodeKind, WorkflowDefinition, validate_and_normalize};
//!
//! let workflow = WorkflowDefinition {
//!     version: "v0".into(),
//!     name: "hello".into(),
//!     nodes: vec![
//!         Node { id: "start".into(), kind: NodeKind::Start { next: "end".into() } },
//!         Node { id: "end".into(), kind: NodeKind::End },
//!     ],
//! };
//!
//! let normalized = validate_and_normalize(&workflow).unwrap();
//! assert_eq!(normalized.name, "hello");
//! ```

pub mod checkpoint;
pub mod debug;
pub mod expressions;
pub mod ir;
pub mod observability;
pub mod recorder;
pub mod replay;
pub mod runtime;
pub mod scheduler;
pub mod state;
pub mod trace;
pub mod validation;
pub mod visualize;
pub mod worker;
pub mod worker_adapter;
pub mod yaml_runner;

pub use checkpoint::{
    CheckpointError, CheckpointStore, FilesystemCheckpointStore, WorkflowCheckpoint,
};
pub use debug::{
    inspect_replay_trace, node_timeline, retry_reason_summary, NodeTimelineEntry,
    ReplayTraceInspection, RetryReasonSummary,
};
pub use expressions::{
    default_expression_engine, evaluate_bool, ExpressionBackend, ExpressionEngine, ExpressionError,
    ExpressionLimits,
};
pub use ir::{MergePolicy, Node, NodeKind, ReduceOperation, RouterRoute, WorkflowDefinition};
pub use recorder::{TraceRecordError, TraceRecorder};
pub use replay::{
    replay_trace, replay_trace_with_options, ReplayCachePolicy, ReplayError, ReplayOptions,
    ReplayReport, ReplayViolation, ReplayViolationCode,
};
pub use runtime::{
    CancellationSignal, LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor,
    NodeExecution, NodeExecutionData, NodeExecutionPolicy, RuntimeSecurityLimits, ScopeAccessError,
    ToolExecutionError, ToolExecutionInput, ToolExecutor, WorkflowEvent, WorkflowEventKind,
    WorkflowReplayMode, WorkflowRetryEvent, WorkflowRunResult, WorkflowRuntime,
    WorkflowRuntimeError, WorkflowRuntimeOptions,
};
pub use scheduler::DagScheduler;
pub use state::{CapabilityToken, ScopedState};
pub use trace::{
    TraceEvent, TraceEventKind, TraceSequence, TraceTerminalStatus, WorkflowTrace,
    WorkflowTraceMetadata,
};
pub use validation::{
    validate_and_normalize, Diagnostic, DiagnosticCode, Severity, ValidationError, ValidationErrors,
};
pub use visualize::workflow_to_mermaid;
pub use worker::{
    CircuitBreakerHooks, WorkerErrorCode, WorkerHandler, WorkerHealth, WorkerHealthStatus,
    WorkerOperation, WorkerPool, WorkerPoolClient, WorkerPoolError, WorkerPoolOptions,
    WorkerProtocolError, WorkerRequest, WorkerResponse, WorkerResult, WorkerSecurityPolicy,
};
pub use worker_adapter::WorkerPoolToolExecutor;
pub use yaml_runner::{
    run_email_workflow_yaml, run_email_workflow_yaml_file,
    run_email_workflow_yaml_file_with_client,
    run_email_workflow_yaml_file_with_client_and_custom_worker,
    run_email_workflow_yaml_file_with_client_and_custom_worker_and_events,
    run_email_workflow_yaml_with_client, run_email_workflow_yaml_with_client_and_custom_worker,
    run_email_workflow_yaml_with_client_and_custom_worker_and_events,
    run_email_workflow_yaml_with_custom_worker,
    run_email_workflow_yaml_with_custom_worker_and_events, run_workflow_yaml,
    run_workflow_yaml_file, run_workflow_yaml_file_with_client,
    run_workflow_yaml_file_with_client_and_custom_worker,
    run_workflow_yaml_file_with_client_and_custom_worker_and_events,
    run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options,
    run_workflow_yaml_with_client, run_workflow_yaml_with_client_and_custom_worker,
    run_workflow_yaml_with_client_and_custom_worker_and_events,
    run_workflow_yaml_with_client_and_custom_worker_and_events_and_options,
    run_workflow_yaml_with_custom_worker, run_workflow_yaml_with_custom_worker_and_events,
    run_workflow_yaml_with_custom_worker_and_events_and_options, verify_yaml_workflow,
    yaml_workflow_file_to_mermaid, yaml_workflow_to_ir, yaml_workflow_to_mermaid,
    NoopYamlWorkflowEventSink, WorkflowMessage, WorkflowMessageRole, WorkflowRunner,
    YamlLlmExecutionRequest, YamlStepTiming, YamlToIrError, YamlWorkflow,
    YamlWorkflowCustomWorkerExecutor, YamlWorkflowDiagnostic, YamlWorkflowDiagnosticSeverity,
    YamlWorkflowEvent, YamlWorkflowEventSink, YamlWorkflowLlmExecutor, YamlWorkflowPayloadMode,
    YamlWorkflowRunError, YamlWorkflowRunOptions, YamlWorkflowRunOutput,
    YamlWorkflowTelemetryConfig, YamlWorkflowTraceContextInput, YamlWorkflowTraceOptions,
    YamlWorkflowTraceTenantContext,
};
