//! Workflow IR and validation primitives for SimpleAgents.
//!
//! This crate currently provides:
//! - A minimal canonical workflow IR (`start`, `llm`, `tool`, `condition`, `end`)
//! - Deterministic normalization helpers
//! - Structural validation with actionable diagnostics
//! - Runtime execution for the minimal node set
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

pub mod ir;
pub mod recorder;
pub mod replay;
pub mod runtime;
pub mod trace;
pub mod validation;

pub use ir::{Node, NodeKind, WorkflowDefinition};
pub use recorder::{TraceRecordError, TraceRecorder};
pub use replay::{replay_trace, ReplayError, ReplayReport, ReplayViolation, ReplayViolationCode};
pub use runtime::{
    CancellationSignal, LlmExecutionError, LlmExecutionInput, LlmExecutionOutput, LlmExecutor,
    NodeExecution, NodeExecutionData, NodeExecutionPolicy, ScopeAccessError, ToolExecutionError,
    ToolExecutionInput, ToolExecutor, WorkflowEvent, WorkflowEventKind, WorkflowReplayMode,
    WorkflowRunResult, WorkflowRuntime, WorkflowRuntimeError, WorkflowRuntimeOptions,
};
pub use trace::{
    TraceEvent, TraceEventKind, TraceSequence, TraceTerminalStatus, WorkflowTrace,
    WorkflowTraceMetadata,
};
pub use validation::{
    validate_and_normalize, Diagnostic, DiagnosticCode, Severity, ValidationError, ValidationErrors,
};
