pub mod workflow_execution {
    use super::super::{
        dispatch_yaml_workflow_execution, load_workflow_yaml_file,
        validate_yaml_workflow_execution, YamlWorkflowEventSink,
        YamlWorkflowExecutionDispatchRequest, YamlWorkflowExecutionRequest,
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
                dispatch_yaml_workflow_execution(YamlWorkflowExecutionDispatchRequest {
                    workflow,
                    workflow_input: request.workflow_input,
                    executor: request.executor,
                    custom_worker: request.custom_worker,
                    resume: request.resume,
                    human_response: request.human_response,
                    event_sink: None,
                    options: request.options,
                    flags: request.flags,
                })
                .await
            }
            YamlWorkflowSource::File(path) => {
                let (_canonical, workflow) = load_workflow_yaml_file(path)?;
                validate_yaml_workflow_execution(
                    &workflow,
                    request.flags,
                    YamlWorkflowExecutionSurface::Run,
                )?;
                dispatch_yaml_workflow_execution(YamlWorkflowExecutionDispatchRequest {
                    workflow: &workflow,
                    workflow_input: request.workflow_input,
                    executor: request.executor,
                    custom_worker: request.custom_worker,
                    resume: request.resume,
                    human_response: request.human_response,
                    event_sink: None,
                    options: request.options,
                    flags: request.flags,
                })
                .await
            }
        }
    }

    pub async fn stream(
        request: YamlWorkflowExecutionRequest<'_>,
        sink: &dyn YamlWorkflowEventSink,
    ) -> Result<YamlWorkflowRunOutput, YamlWorkflowRunError> {
        let filter = YamlWorkflowStreamFilterSink::new(sink, request.flags.workflow_streaming);
        match request.source {
            YamlWorkflowSource::Inline(workflow) => {
                validate_yaml_workflow_execution(
                    workflow,
                    request.flags,
                    YamlWorkflowExecutionSurface::Stream,
                )?;
                dispatch_yaml_workflow_execution(YamlWorkflowExecutionDispatchRequest {
                    workflow,
                    workflow_input: request.workflow_input,
                    executor: request.executor,
                    custom_worker: request.custom_worker,
                    resume: request.resume,
                    human_response: request.human_response,
                    event_sink: Some(&filter),
                    options: request.options,
                    flags: request.flags,
                })
                .await
            }
            YamlWorkflowSource::File(path) => {
                let (_canonical, workflow) = load_workflow_yaml_file(path)?;
                validate_yaml_workflow_execution(
                    &workflow,
                    request.flags,
                    YamlWorkflowExecutionSurface::Stream,
                )?;
                dispatch_yaml_workflow_execution(YamlWorkflowExecutionDispatchRequest {
                    workflow: &workflow,
                    workflow_input: request.workflow_input,
                    executor: request.executor,
                    custom_worker: request.custom_worker,
                    resume: request.resume,
                    human_response: request.human_response,
                    event_sink: Some(&filter),
                    options: request.options,
                    flags: request.flags,
                })
                .await
            }
        }
    }
}
