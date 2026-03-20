use std::path::Path;

use serde_json::{json, Value};
use simple_agents_core::SimpleAgentsClient;

use super::{
    WorkflowRunner, YamlWorkflow, YamlWorkflowCustomWorkerExecutor, YamlWorkflowEventSink,
    YamlWorkflowLlmExecutor, YamlWorkflowRunError, YamlWorkflowRunOptions, YamlWorkflowRunOutput,
};

pub async fn run_workflow_yaml_file(
    workflow_path: &Path,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_input(workflow_input)
        .with_executor(executor)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_file(
    workflow_path: &Path,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_email_text(email_text)
        .with_executor(executor)
        .run()
        .await
}

pub async fn run_workflow_yaml_file_with_client(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_input(workflow_input)
        .with_client(client)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_file_with_client(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_email_text(email_text)
        .with_client(client)
        .run()
        .await
}

pub async fn run_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_workflow(workflow)
        .with_input(workflow_input)
        .with_client(client)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_with_client(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_client(workflow, &workflow_input, client).await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_input(workflow_input)
        .with_client(client)
        .with_custom_worker(custom_worker)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_file_with_client_and_custom_worker(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_email_text(email_text)
        .with_client(client)
        .with_custom_worker(custom_worker)
        .run()
        .await
}

pub async fn run_workflow_yaml_file_with_client_and_custom_worker_and_events(
    workflow_path: &Path,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_input(workflow_input)
        .with_client(client)
        .with_custom_worker(custom_worker)
        .with_event_sink(event_sink)
        .run()
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
    WorkflowRunner::from_file(workflow_path)
        .with_input(workflow_input)
        .with_client(client)
        .with_custom_worker(custom_worker)
        .with_event_sink(event_sink)
        .with_options(options)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_file_with_client_and_custom_worker_and_events(
    workflow_path: &Path,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_file(workflow_path)
        .with_email_text(email_text)
        .with_client(client)
        .with_custom_worker(custom_worker)
        .with_event_sink(event_sink)
        .run()
        .await
}

pub async fn run_workflow_yaml_with_client_and_custom_worker(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_workflow(workflow)
        .with_input(workflow_input)
        .with_client(client)
        .with_custom_worker(custom_worker)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_with_client_and_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    client: &SimpleAgentsClient,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_client_and_custom_worker(
        workflow,
        &workflow_input,
        client,
        custom_worker,
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
    super::run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
        workflow,
        workflow_input,
        client,
        custom_worker,
        event_sink,
        &YamlWorkflowRunOptions::default(),
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
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_client_and_custom_worker_and_events(
        workflow,
        &workflow_input,
        client,
        custom_worker,
        event_sink,
    )
    .await
}

pub async fn run_workflow_yaml(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_workflow(workflow)
        .with_input(workflow_input)
        .with_executor(executor)
        .run()
        .await
}

pub async fn run_email_workflow_yaml(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_workflow(workflow)
        .with_email_text(email_text)
        .with_executor(executor)
        .run()
        .await
}

pub async fn run_workflow_yaml_with_custom_worker(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_workflow(workflow)
        .with_input(workflow_input)
        .with_executor(executor)
        .with_custom_worker(custom_worker)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_with_custom_worker(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_workflow(workflow)
        .with_email_text(email_text)
        .with_executor(executor)
        .with_custom_worker(custom_worker)
        .run()
        .await
}

pub async fn run_workflow_yaml_with_custom_worker_and_events(
    workflow: &YamlWorkflow,
    workflow_input: &Value,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    WorkflowRunner::from_workflow(workflow)
        .with_input(workflow_input)
        .with_executor(executor)
        .with_custom_worker(custom_worker)
        .with_event_sink(event_sink)
        .run()
        .await
}

pub async fn run_email_workflow_yaml_with_custom_worker_and_events(
    workflow: &YamlWorkflow,
    email_text: &str,
    executor: &dyn YamlWorkflowLlmExecutor,
    custom_worker: Option<&dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
    let workflow_input = json!({ "email_text": email_text });
    run_workflow_yaml_with_custom_worker_and_events(
        workflow,
        &workflow_input,
        executor,
        custom_worker,
        event_sink,
    )
    .await
}
