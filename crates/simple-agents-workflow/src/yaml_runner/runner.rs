use std::path::Path;

use serde_json::{json, Value};
use simple_agents_core::SimpleAgentsClient;

use super::{
    load_workflow_yaml_file,
    run_workflow_yaml_with_client_and_custom_worker_and_events_and_options,
    run_workflow_yaml_with_custom_worker_and_events_and_options, YamlWorkflow,
    YamlWorkflowCustomWorkerExecutor, YamlWorkflowEventSink, YamlWorkflowLlmExecutor,
    YamlWorkflowRunError, YamlWorkflowRunOptions, YamlWorkflowRunOutput,
};

#[derive(Clone, Copy)]
enum WorkflowRunnerExecutor<'a> {
    Llm(&'a dyn YamlWorkflowLlmExecutor),
    Client(&'a SimpleAgentsClient),
}

#[derive(Clone, Copy)]
enum WorkflowRunnerSource<'a> {
    File(&'a Path),
    Inline(&'a YamlWorkflow),
}

/// Builder-style runner for YAML workflow execution.
///
/// This is the preferred additive API for configuring workflow runs while keeping
/// legacy `run_*` helpers as compatibility adapters.
pub struct WorkflowRunner<'a> {
    source: WorkflowRunnerSource<'a>,
    workflow_input: Option<&'a Value>,
    email_text: Option<&'a str>,
    executor: Option<WorkflowRunnerExecutor<'a>>,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    options: Option<&'a YamlWorkflowRunOptions>,
}

impl<'a> WorkflowRunner<'a> {
    pub fn from_file(workflow_path: &'a Path) -> Self {
        Self {
            source: WorkflowRunnerSource::File(workflow_path),
            workflow_input: None,
            email_text: None,
            executor: None,
            custom_worker: None,
            event_sink: None,
            options: None,
        }
    }

    pub fn from_workflow(workflow: &'a YamlWorkflow) -> Self {
        Self {
            source: WorkflowRunnerSource::Inline(workflow),
            workflow_input: None,
            email_text: None,
            executor: None,
            custom_worker: None,
            event_sink: None,
            options: None,
        }
    }

    pub fn with_input(mut self, workflow_input: &'a Value) -> Self {
        self.workflow_input = Some(workflow_input);
        self
    }

    pub fn with_email_text(mut self, email_text: &'a str) -> Self {
        self.email_text = Some(email_text);
        self
    }

    pub fn with_executor(mut self, executor: &'a dyn YamlWorkflowLlmExecutor) -> Self {
        self.executor = Some(WorkflowRunnerExecutor::Llm(executor));
        self
    }

    pub fn with_client(mut self, client: &'a SimpleAgentsClient) -> Self {
        self.executor = Some(WorkflowRunnerExecutor::Client(client));
        self
    }

    pub fn with_custom_worker(
        mut self,
        custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
    ) -> Self {
        self.custom_worker = custom_worker;
        self
    }

    pub fn with_event_sink(mut self, event_sink: Option<&'a dyn YamlWorkflowEventSink>) -> Self {
        self.event_sink = event_sink;
        self
    }

    pub fn with_options(mut self, options: &'a YamlWorkflowRunOptions) -> Self {
        self.options = Some(options);
        self
    }

    pub async fn run(self) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
        let fallback_input;
        let workflow_input = if let Some(workflow_input) = self.workflow_input {
            workflow_input
        } else if let Some(email_text) = self.email_text {
            fallback_input = json!({ "email_text": email_text });
            &fallback_input
        } else {
            return Err(YamlWorkflowRunError::InvalidInput {
                message: "workflow input is required; call with_input(...) or with_email_text(...)"
                    .to_string(),
            });
        };

        let default_options;
        let options = if let Some(options) = self.options {
            options
        } else {
            default_options = YamlWorkflowRunOptions::default();
            &default_options
        };

        let executor = self
            .executor
            .ok_or_else(|| YamlWorkflowRunError::InvalidInput {
                message:
                    "workflow executor is required; call with_executor(...) or with_client(...)"
                        .to_string(),
            })?;

        match (self.source, executor) {
            (WorkflowRunnerSource::File(path), WorkflowRunnerExecutor::Llm(executor)) => {
                let (_, workflow) = load_workflow_yaml_file(path)?;
                run_workflow_yaml_with_custom_worker_and_events_and_options(
                    &workflow,
                    workflow_input,
                    executor,
                    self.custom_worker,
                    self.event_sink,
                    options,
                )
                .await
            }
            (WorkflowRunnerSource::File(path), WorkflowRunnerExecutor::Client(client)) => {
                let (_, workflow) = load_workflow_yaml_file(path)?;
                run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
                    &workflow,
                    workflow_input,
                    client,
                    self.custom_worker,
                    self.event_sink,
                    options,
                )
                .await
            }
            (WorkflowRunnerSource::Inline(workflow), WorkflowRunnerExecutor::Llm(executor)) => {
                run_workflow_yaml_with_custom_worker_and_events_and_options(
                    workflow,
                    workflow_input,
                    executor,
                    self.custom_worker,
                    self.event_sink,
                    options,
                )
                .await
            }
            (WorkflowRunnerSource::Inline(workflow), WorkflowRunnerExecutor::Client(client)) => {
                run_workflow_yaml_with_client_and_custom_worker_and_events_and_options(
                    workflow,
                    workflow_input,
                    client,
                    self.custom_worker,
                    self.event_sink,
                    options,
                )
                .await
            }
        }
    }
}
