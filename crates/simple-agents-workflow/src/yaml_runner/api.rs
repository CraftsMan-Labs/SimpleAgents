use std::path::Path;

use serde_json::Value;
use simple_agents_core::SimpleAgentsClient;

use super::{
    WorkflowRunner, YamlWorkflow, YamlWorkflowCustomWorkerExecutor, YamlWorkflowEventSink,
    YamlWorkflowLlmExecutor, YamlWorkflowRunError, YamlWorkflowRunOptions, YamlWorkflowRunOutput,
    YamlWorkflowRunTypedOutput,
};

#[derive(Clone, Copy)]
enum WorkflowApiSource<'a> {
    File(&'a Path),
    Inline(&'a YamlWorkflow),
}

#[derive(Clone, Copy)]
enum WorkflowApiInput<'a> {
    Input(&'a Value),
    EmailText(&'a str),
}

fn runner_for_source<'a>(source: WorkflowApiSource<'a>) -> WorkflowRunner<'a> {
    match source {
        WorkflowApiSource::File(path) => WorkflowRunner::from_file(path),
        WorkflowApiSource::Inline(workflow) => WorkflowRunner::from_workflow(workflow),
    }
}

fn runner_with_input<'a>(
    runner: WorkflowRunner<'a>,
    input: WorkflowApiInput<'a>,
) -> WorkflowRunner<'a> {
    match input {
        WorkflowApiInput::Input(workflow_input) => runner.with_input(workflow_input),
        WorkflowApiInput::EmailText(email_text) => runner.with_email_text(email_text),
    }
}

fn runner_with_optional_options<'a>(
    runner: WorkflowRunner<'a>,
    options: Option<&'a YamlWorkflowRunOptions>,
) -> WorkflowRunner<'a> {
    if let Some(run_options) = options {
        runner.with_options(run_options)
    } else {
        runner
    }
}

async fn run_with_executor<'a>(
    source: WorkflowApiSource<'a>,
    input: WorkflowApiInput<'a>,
    executor: &'a dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    options: Option<&'a YamlWorkflowRunOptions>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let runner = runner_with_input(runner_for_source(source), input)
        .with_executor(executor)
        .with_custom_worker(custom_worker)
        .with_event_sink(event_sink);
    runner_with_optional_options(runner, options).run().await
}

async fn run_with_client<'a>(
    source: WorkflowApiSource<'a>,
    input: WorkflowApiInput<'a>,
    client: &'a SimpleAgentsClient,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    options: Option<&'a YamlWorkflowRunOptions>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let runner = runner_with_input(runner_for_source(source), input)
        .with_client(client)
        .with_custom_worker(custom_worker)
        .with_event_sink(event_sink);
    runner_with_optional_options(runner, options).run().await
}

async fn run_typed_with_executor<'a>(
    source: WorkflowApiSource<'a>,
    input: WorkflowApiInput<'a>,
    executor: &'a dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    options: Option<&'a YamlWorkflowRunOptions>,
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    let runner = runner_with_input(runner_for_source(source), input)
        .with_executor(executor)
        .with_custom_worker(custom_worker)
        .with_event_sink(event_sink);
    runner_with_optional_options(runner, options)
        .run_typed()
        .await
}

pub async fn run_workflow_yaml_file_typed(
    workflow_path: &Path,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        executor,
        None,
        None,
        None,
    )
    .await
}

pub async fn run_email_workflow_yaml_file_typed(
    workflow_path: &Path,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::EmailText(email_text),
        executor,
        None,
        None,
        None,
    )
    .await
}

pub async fn run_workflow_yaml_typed(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        executor,
        None,
        None,
        None,
    )
    .await
}

pub async fn run_email_workflow_yaml_typed(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        executor,
        None,
        None,
        None,
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
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        executor,
        custom_worker,
        event_sink,
        Some(options),
    )
    .await
}

pub async fn run_email_workflow_yaml_file_typed_with_custom_worker_and_events_and_options(
    workflow_path: &Path,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::EmailText(email_text),
        executor,
        custom_worker,
        event_sink,
        Some(options),
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
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::Input(workflow_input),
        executor,
        custom_worker,
        event_sink,
        Some(options),
    )
    .await
}

pub async fn run_email_workflow_yaml_typed_with_custom_worker_and_events_and_options(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
    options: &YamlWorkflowRunOptions,
) -> Result<YamlWorkflowRunTypedOutput, YamlWorkflowRunError> {
    run_typed_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        executor,
        custom_worker,
        event_sink,
        Some(options),
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
    )
    .await
}

pub async fn run_email_workflow_yaml_file(
    workflow_path: &Path,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::EmailText(email_text),
        executor,
        None,
        None,
        None,
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
    )
    .await
}

pub async fn run_email_workflow_yaml_file_with_client(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::EmailText(email_text),
        client,
        None,
        None,
        None,
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
    )
    .await
}

pub async fn run_email_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        client,
        None,
        None,
        None,
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
    )
    .await
}

pub async fn run_email_workflow_yaml_file_with_client_and_custom_worker(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::EmailText(email_text),
        client,
        custom_worker,
        None,
        None,
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
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::Input(workflow_input),
        client,
        custom_worker,
        event_sink,
        Some(options),
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
    run_with_client(
        WorkflowApiSource::File(workflow_path),
        WorkflowApiInput::EmailText(email_text),
        client,
        custom_worker,
        event_sink,
        None,
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
    )
    .await
}

pub async fn run_email_workflow_yaml_with_client_and_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_client(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        client,
        custom_worker,
        None,
        None,
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
    run_with_client(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        client,
        custom_worker,
        event_sink,
        None,
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
    )
    .await
}

pub async fn run_email_workflow_yaml(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        executor,
        None,
        None,
        None,
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
    )
    .await
}

pub async fn run_email_workflow_yaml_with_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        executor,
        custom_worker,
        None,
        None,
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
    run_with_executor(
        WorkflowApiSource::Inline(workflow),
        WorkflowApiInput::EmailText(email_text),
        executor,
        custom_worker,
        event_sink,
        None,
    )
    .await
}
