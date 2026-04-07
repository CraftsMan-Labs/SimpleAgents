use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Instant;

use crate::observability::tracing::{
    flush_workflow_tracer, workflow_tracer, SpanKind, TraceContext,
};
use serde_json::{json, Value};
use simple_agent_type::message::Message;
use simple_agent_type::tool::{ToolChoice, ToolChoiceMode, ToolChoiceTool, ToolDefinition};

mod api;
mod client_executor;
pub(crate) mod context;
mod contracts;
mod events;
pub(crate) mod execute;
mod globals;
mod llm_tools;
pub(crate) mod loader;
mod nerdstats;
mod node_execution;
mod output;
mod recovery;
mod spans;
mod stream_filters;
mod subworkflow;
mod telemetry;
pub(crate) mod types;
mod validation;
pub use api::workflow_execution;
use client_executor::BorrowedClientExecutor;
use context::{
    collect_template_bindings, evaluate_switch_condition, interpolate_json, interpolate_template,
    parse_messages_from_context, resolve_path,
};
use contracts::{event_sink_is_cancelled, workflow_event_sink_cancelled_message};
pub use contracts::{
    is_workflow_stream_delta_event, NoopYamlWorkflowEventSink, WorkflowMessage,
    WorkflowMessageRole, YamlCustomWorker, YamlEdge, YamlGlobalUpdate, YamlLlmCall,
    YamlLlmExecutionRequest, YamlNode, YamlNodeConfig, YamlNodeType, YamlOpenAiToolDeclaration,
    YamlOpenAiToolFunction, YamlResolvedTool, YamlSimplifiedToolDeclaration, YamlSwitch,
    YamlSwitchBranch, YamlTemplateBinding, YamlToIrError, YamlToolChoiceConfig,
    YamlToolDeclaration, YamlToolFormat, YamlWorkflow, YamlWorkflowCustomWorkerExecutor,
    YamlWorkflowDiagnostic, YamlWorkflowDiagnosticSeverity, YamlWorkflowEvent,
    YamlWorkflowEventSink, YamlWorkflowLlmExecutor, YamlWorkflowRunError,
    YamlWorkflowStreamFilterSink, YamlWorkflowTokenKind,
};
pub use events::{
    CallbackSink, DefaultEventPrinter, NodeType, NoopSink, TokenKind, WorkflowEvent,
    WorkflowEventSink,
};
use globals::{apply_set_globals, apply_update_globals};
use llm_tools::{llm_output_schema_for_node, normalize_llm_tools, normalize_tool_choice};
pub(crate) use loader::load_workflow_yaml_file;
pub(crate) use nerdstats::workflow_nerdstats;
pub use output::{RunMetadata, StepTiming, TokenTotals, WorkflowRunOutput};
pub use recovery::{PartialWorkflowOutput, WorkflowCheckpoint};
pub(crate) use telemetry::*;
use types::{
    completion_tokens_per_second, resolve_requested_model, validate_sample_rate, YamlTokenTotals,
};
// Execution-plumbing types are public API (used by callers constructing requests).
pub use types::{
    validate_yaml_workflow_execution, YamlLlmExecutionResult, YamlLlmNodeMetrics,
    YamlLlmTokenUsage, YamlStepTiming, YamlToolCallTrace, YamlToolTraceMode,
    YamlWorkflowExecutionFlags, YamlWorkflowExecutionRequest, YamlWorkflowExecutionSurface,
    YamlWorkflowExecutorBinding, YamlWorkflowPayloadMode, YamlWorkflowRunOptions,
    YamlWorkflowSource, YamlWorkflowTelemetryConfig, YamlWorkflowTraceContextInput,
    YamlWorkflowTraceOptions, YamlWorkflowTraceTenantContext,
};
// Internal output type — kept pub(crate) so execute.rs / nerdstats.rs etc. can use it;
// external callers should use `WorkflowRunOutput` from `output.rs` instead.
pub(crate) use types::YamlWorkflowRunOutput;
pub use validation::verify_yaml_workflow;

/// When `custom_worker` is `None`, ensures the workflow file does not declare `custom_worker`
/// nodes that require a runtime executor (same rules as workflow execution).
pub fn validate_custom_worker_executor_for_file(
    workflow_path: &Path,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<(), YamlWorkflowRunError> {
    if custom_worker.is_some() {
        return Ok(());
    }
    let (_canonical, workflow) = load_workflow_yaml_file(workflow_path)?;
    execute::validate_custom_worker_handler_files(&workflow, None)
}

#[cfg(test)]
mod tests;

pub(super) async fn dispatch_yaml_workflow_execution<'a>(
    workflow: &'a YamlWorkflow,
    workflow_input: &'a Value,
    executor: YamlWorkflowExecutorBinding<'a>,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    options: &'a YamlWorkflowRunOptions,
    flags: YamlWorkflowExecutionFlags,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    match executor {
        YamlWorkflowExecutorBinding::Llm(ex) => {
            execute::run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
                workflow,
                workflow_input,
                ex,
                custom_worker,
                event_sink,
                options,
                flags,
            )
            .await
        }
        YamlWorkflowExecutorBinding::Client(client) => {
            let client_executor = BorrowedClientExecutor {
                client,
                custom_worker,
                run_options: options.clone(),
            };
            execute::run_workflow_yaml_with_custom_worker_and_events_and_options_impl(
                workflow,
                workflow_input,
                &client_executor,
                custom_worker,
                event_sink,
                options,
                flags,
            )
            .await
        }
    }
}
