use napi::bindgen_prelude::{Error, Result};
use serde_json::Value as JsonValue;
use simple_agents_workflow::YamlWorkflowRunOptions;

pub(crate) fn parse_workflow_options(
    workflow_options: Option<JsonValue>,
) -> Result<YamlWorkflowRunOptions> {
    workflow_options
        .map(|value| {
            serde_json::from_value::<YamlWorkflowRunOptions>(value)
                .map_err(|error| Error::from_reason(format!("invalid workflowOptions: {error}")))
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

pub(crate) fn validate_workflow_request(
    workflow_path: &str,
    workflow_input: &JsonValue,
) -> Result<()> {
    if workflow_path.trim().is_empty() {
        return Err(Error::from_reason(
            "workflow_path cannot be empty".to_string(),
        ));
    }
    if !workflow_input.is_object() {
        return Err(Error::from_reason(
            "workflowInput must be a JSON object".to_string(),
        ));
    }
    Ok(())
}
