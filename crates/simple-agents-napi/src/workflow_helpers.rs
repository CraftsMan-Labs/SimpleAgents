use napi::bindgen_prelude::{Either, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use simple_agents_workflow::yaml_runner::{YamlWorkflowExecutionFlags, YamlWorkflowRunOptions};

use crate::{parse_message, ContentPartInput, MessageInput};
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{Result as SaResult, SimpleAgentsError};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkflowRequestOptions {
    #[serde(flatten)]
    pub(crate) run_options: YamlWorkflowRunOptions,
    #[serde(default)]
    pub(crate) include_events: bool,
}

pub(crate) fn parse_workflow_request_options(
    workflow_options: Option<JsonValue>,
) -> Result<WorkflowRequestOptions> {
    workflow_options
        .map(|value| {
            serde_json::from_value::<WorkflowRequestOptions>(value)
                .map_err(|error| Error::from_reason(format!("invalid workflowOptions: {error}")))
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct WorkflowExecutionFlagsPatchNapi {
    #[serde(alias = "healing")]
    pub(crate) healing: Option<bool>,
    #[serde(alias = "workflow_streaming", alias = "workflowStreaming")]
    pub(crate) workflow_streaming: Option<bool>,
    #[serde(alias = "node_llm_streaming", alias = "nodeLlmStreaming")]
    pub(crate) node_llm_streaming: Option<bool>,
    #[serde(alias = "split_stream_deltas", alias = "splitStreamDeltas")]
    pub(crate) split_stream_deltas: Option<bool>,
    #[serde(alias = "debug_stream_parse", alias = "debugStreamParse")]
    pub(crate) debug_stream_parse: Option<bool>,
}

pub(crate) fn parse_workflow_execution_flags_patch(
    execution: Option<JsonValue>,
) -> Result<WorkflowExecutionFlagsPatchNapi> {
    execution
        .map(|value| {
            serde_json::from_value::<WorkflowExecutionFlagsPatchNapi>(value).map_err(|error| {
                Error::from_reason(format!("invalid workflowExecution flags: {error}"))
            })
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

pub(crate) fn apply_workflow_execution_flags_patch(
    mut base: YamlWorkflowExecutionFlags,
    patch: &WorkflowExecutionFlagsPatchNapi,
) -> YamlWorkflowExecutionFlags {
    if let Some(v) = patch.healing {
        base.healing = v;
    }
    if let Some(v) = patch.workflow_streaming {
        base.workflow_streaming = v;
    }
    if let Some(v) = patch.node_llm_streaming {
        base.node_llm_streaming = v;
    }
    if let Some(v) = patch.split_stream_deltas {
        base.split_stream_deltas = v;
    }
    if let Some(v) = patch.debug_stream_parse {
        base.debug_stream_parse = v;
    }
    base
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

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MessageContentInputWire {
    Text(String),
    Parts(Vec<ContentPartInputWire>),
}

#[derive(Debug, Deserialize)]
struct ContentPartInputWire {
    #[serde(rename = "type")]
    r#type: String,
    text: Option<String>,
    #[serde(rename = "mediaType", alias = "media_type")]
    media_type: Option<String>,
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageInputWire {
    role: String,
    content: MessageContentInputWire,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "toolCallId")]
    tool_call_id: Option<String>,
}

fn needs_napi_messages_normalization(messages: &JsonValue) -> bool {
    let Some(list) = messages.as_array() else {
        return false;
    };
    for message in list {
        let Some(content) = message.get("content") else {
            continue;
        };
        let Some(parts) = content.as_array() else {
            continue;
        };
        for part in parts {
            let Some(obj) = part.as_object() else {
                continue;
            };
            if obj.contains_key("mediaType") || obj.contains_key("media_type") {
                return true;
            }
            if let Some(kind) = obj.get("type").and_then(JsonValue::as_str) {
                if kind == "image" || kind == "audio" {
                    return true;
                }
            }
        }
    }
    false
}

/// Normalizes `workflowInput.messages` when callers pass NAPI `MessageInput` part shapes
/// (`type: image|audio|video` with `mediaType`/`data`) into canonical workflow wire messages.
pub(crate) fn normalize_workflow_input_messages(workflow_input: &JsonValue) -> Result<JsonValue> {
    let JsonValue::Object(input_obj) = workflow_input else {
        return Ok(workflow_input.clone());
    };
    let Some(messages_value) = input_obj.get("messages") else {
        return Ok(workflow_input.clone());
    };
    if !needs_napi_messages_normalization(messages_value) {
        return Ok(workflow_input.clone());
    }

    let wire_messages: Vec<MessageInputWire> = serde_json::from_value(messages_value.clone())
        .map_err(|error| {
            Error::from_reason(format!(
                "workflowInput.messages must be a MessageInput[]: {error}"
            ))
        })?;

    let typed_messages: Vec<MessageInput> = wire_messages
        .into_iter()
        .map(|message| MessageInput {
            role: message.role,
            content: match message.content {
                MessageContentInputWire::Text(text) => Either::A(text),
                MessageContentInputWire::Parts(parts) => Either::B(
                    parts
                        .into_iter()
                        .map(|part| ContentPartInput {
                            r#type: part.r#type,
                            text: part.text,
                            media_type: part.media_type,
                            data: part.data,
                        })
                        .collect(),
                ),
            },
            name: message.name,
            tool_call_id: message.tool_call_id,
            tool_calls: None,
        })
        .collect();

    let parsed: Vec<Message> = typed_messages
        .into_iter()
        .map(parse_message)
        .collect::<SaResult<Vec<_>>>()
        .map_err(napi_err)?;

    let normalized_messages = parsed
        .iter()
        .map(|message| {
            serde_json::to_value(message).map_err(|error| {
                Error::from_reason(format!("failed to serialize message: {error}"))
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut normalized = input_obj.clone();
    normalized.insert(
        "messages".to_string(),
        JsonValue::Array(normalized_messages),
    );
    Ok(JsonValue::Object(normalized))
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
