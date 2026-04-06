use std::path::Path;

use serde_json::Value;
use simple_agents_core::SimpleAgentsClient;

use super::{
    YamlWorkflow, YamlWorkflowCustomWorkerExecutor, YamlWorkflowEventSink,
    YamlWorkflowExecutionFlags, YamlWorkflowLlmExecutor, YamlWorkflowRunError,
    YamlWorkflowRunOptions, YamlWorkflowRunOutput,
};

async fn run_with_executor<'a>(
    source: WorkflowApiSource<'a>,
    input: WorkflowApiInput<'a>,
    executor: &'a dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    options: Option<&'a YamlWorkflowRunOptions>,
    execution_flags: YamlWorkflowExecutionFlags,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let default_options = YamlWorkflowRunOptions::default();
    let run_options = options.unwrap_or(&default_options);
    match source {
        WorkflowApiSource::File(path) => {
            let (_canonical, workflow) = super::load_workflow_yaml_file(path)?;
            super::run_workflow_yaml_with_custom_worker_and_events_and_options(
                &workflow,
                input.value(),
                executor,
                custom_worker,
                event_sink,
                run_options,
                execution_flags,
            )
            .await
        }
        WorkflowApiSource::Inline(workflow) => {
            super::run_workflow_yaml_with_custom_worker_and_events_and_options(
                workflow,
                input.value(),
                executor,
                custom_worker,
                event_sink,
                run_options,
                execution_flags,
            )
            .await
        }
    }
}

async fn run_with_client<'a>(
    source: WorkflowApiSource<'a>,
    input: WorkflowApiInput<'a>,
    client: &'a SimpleAgentsClient,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    options: Option<&'a YamlWorkflowRunOptions>,
    execution_flags: YamlWorkflowExecutionFlags,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let default_options = YamlWorkflowRunOptions::default();
    let run_options = options.unwrap_or(&default_options);
    match source {
        WorkflowApiSource::File(path) => {
            let (_canonical, workflow) = super::load_workflow_yaml_file(path)?;
            super::run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
                &workflow,
                input.value(),
                client,
                custom_worker,
                event_sink,
                run_options,
                execution_flags,
            )
            .await
        }
        WorkflowApiSource::Inline(workflow) => {
            super::run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
                workflow,
                input.value(),
                client,
                custom_worker,
                event_sink,
                run_options,
                execution_flags,
            )
            .await
        }
    }
}

#[derive(Clone, Copy)]
enum WorkflowApiSource<'a> {
    File(&'a Path),
    Inline(&'a YamlWorkflow),
}

#[derive(Clone, Copy)]
enum WorkflowApiInput<'a> {
    Input(&'a Value),
}

impl<'a> WorkflowApiInput<'a> {
    fn value(&self) -> &'a Value {
        match self {
            WorkflowApiInput::Input(v) => v,
        }
    }
}

pub async fn run_workflow_yaml_file_typed(
    workflow_path: &Path,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        executor,
        None,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_typed(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        executor,
        None,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_file_typed_with_custom_worker_and_events_and_options(
    workflow_path: &Path,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        executor,
        custom_worker,
        event_sink,
        Some(options),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_typed_with_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        executor,
        custom_worker,
        event_sink,
        Some(options),
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_file(
    workflow_path: &Path,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        executor,
        None,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_file_with_client(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        client,
        None,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        client,
        None,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        client,
        custom_worker,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker_and_events(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        client,
        custom_worker,
        event_sink,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
    execution_flags: YamlWorkflowExecutionFlags,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        client,
        custom_worker,
        event_sink,
        Some(options),
        execution_flags,
    )
    .await
}

pub async fn run_workflow_yaml_with_client_and_custom_worker(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        client,
        custom_worker,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_with_client_and_custom_worker_and_events(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        client,
        custom_worker,
        event_sink,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        executor,
        None,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_with_custom_worker(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        executor,
        custom_worker,
        None,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub async fn run_workflow_yaml_with_custom_worker_and_events(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        executor,
        custom_worker,
        event_sink,
        None,
        YamlWorkflowExecutionFlags::default(),
    )
    .await
}

pub mod workflow_execution {
    use super::super::{
        dispatch_yaml_workflow_execution, load_workflow_yaml_file,
        validate_yaml_workflow_execution, YamlWorkflowExecutionRequest,
        YamlWorkflowExecutionSurface, YamlWorkflowRunError, YamlWorkflowRunOutput,
        YamlWorkflowSource, YamlWorkflowStreamFilterSink,
    };

    pub async fn run(
        request: YamlWorkflowExecutionRequest<'_>,
    ) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
        match request.source {
            YamlWorkflowSource::Inline(workflow) => {
                validate_yaml_workflow_execution(
                    workflow,
                    request.flags,
                    YamlWorkflowExecutionSurface::Run,
                )?;
                dispatch_yaml_workflow_execution(
                    workflow,
                    request.workflow_input,
                    request.executor,
                    request.custom_worker,
                    None,
                    request.options,
                    request.flags,
                )
                .await
            }
            YamlWorkflowSource::File(path) => {
                let (_canonical, workflow) = load_workflow_yaml_file(path)?;
                validate_yaml_workflow_execution(
                    &workflow,
                    request.flags,
                    YamlWorkflowExecutionSurface::Run,
                )?;
                dispatch_yaml_workflow_execution(
                    &workflow,
                    request.workflow_input,
                    request.executor,
                    request.custom_worker,
                    None,
                    request.options,
                    request.flags,
                )
                .await
            }
        }
    }

    pub fn run_async<'a>(
        request: YamlWorkflowExecutionRequest<'a>,
    ) -> impl std::future::Future<Output = Result<YamlWorkflowRunOutput, YamlWorkflowRunError>> + Send + 'a
    {
        run(request)
    }

    pub async fn stream(
        request: YamlWorkflowExecutionRequest<'_>,
        sink: &dyn super::super::YamlWorkflowEventSink,
    ) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
        let filter = YamlWorkflowStreamFilterSink::new(sink, request.flags.workflow_streaming);
        match request.source {
            YamlWorkflowSource::Inline(workflow) => {
                validate_yaml_workflow_execution(
                    workflow,
                    request.flags,
                    YamlWorkflowExecutionSurface::Stream,
                )?;
                dispatch_yaml_workflow_execution(
                    workflow,
                    request.workflow_input,
                    request.executor,
                    request.custom_worker,
                    Some(&filter),
                    request.options,
                    request.flags,
                )
                .await
            }
            YamlWorkflowSource::File(path) => {
                let (_canonical, workflow) = load_workflow_yaml_file(path)?;
                validate_yaml_workflow_execution(
                    &workflow,
                    request.flags,
                    YamlWorkflowExecutionSurface::Stream,
                )?;
                dispatch_yaml_workflow_execution(
                    &workflow,
                    request.workflow_input,
                    request.executor,
                    request.custom_worker,
                    Some(&filter),
                    request.options,
                    request.flags,
                )
                .await
            }
        }
    }
}
