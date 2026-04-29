pub mod workflow_execution {
    use super::super::{
        dispatch_yaml_workflow_execution, load_workflow_yaml_file,
        validate_yaml_workflow_execution, YamlWorkflowEventSink, YamlWorkflowExecutionRequest,
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
                    request.resume,
                    request.human_response,
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
                    request.resume,
                    request.human_response,
                    None,
                    request.options,
                    request.flags,
                )
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
                dispatch_yaml_workflow_execution(
                    workflow,
                    request.workflow_input,
                    request.executor,
                    request.custom_worker,
                    request.resume,
                    request.human_response,
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
                    request.resume,
                    request.human_response,
                    Some(&filter),
                    request.options,
                    request.flags,
                )
                .await
            }
        }
    }
}
