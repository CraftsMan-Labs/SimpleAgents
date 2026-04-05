use napi::bindgen_prelude::{Error, Result};
use serde_json::{Map, Value as JsonValue};
use simple_agents_workflow::YamlWorkflowRunOptions;

use crate::{parse_message, MessageInput};
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{Result as SaResult, SimpleAgentsError};

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

fn napi_err(error: SimpleAgentsError) -> Error {
    Error::from_reason(error.to_string())
}

/// Builds workflow JSON input with a required `messages` array (from typed [`MessageInput`]).
pub(crate) fn build_workflow_input_with_messages_envelope(
    messages: Vec<MessageInput>,
    extra: Option<&JsonValue>,
) -> Result<JsonValue> {
    if messages.is_empty() {
        return Err(Error::from_reason(
            "messages must contain at least one item".to_string(),
        ));
    }
    let parsed: Vec<Message> = messages
        .into_iter()
        .map(parse_message)
        .collect::<SaResult<Vec<_>>>()
        .map_err(napi_err)?;
    let mut arr = Vec::with_capacity(parsed.len());
    for message in &parsed {
        let value = serde_json::to_value(message)
            .map_err(|error| Error::from_reason(format!("failed to serialize message: {error}")))?;
        arr.push(value);
    }

    let mut map = Map::new();
    map.insert("messages".to_string(), JsonValue::Array(arr));

    if let Some(extra) = extra {
        if extra.is_null() {
            return Ok(JsonValue::Object(map));
        }
        let JsonValue::Object(extra_obj) = extra else {
            return Err(Error::from_reason(
                "extraWorkflowInput must be a JSON object when provided".to_string(),
            ));
        };
        for (key, value) in extra_obj {
            if key == "messages" {
                continue;
            }
            map.insert(key.clone(), value.clone());
        }
    }

    Ok(JsonValue::Object(map))
}
