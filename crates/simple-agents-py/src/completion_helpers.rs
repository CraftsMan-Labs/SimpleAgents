use crate::schema_helpers::schema_to_json_value;
use crate::{HealedJsonResult, ResponseWithMetadata};
use futures_util::Stream;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use simple_agent_type::message::{parse_messages_value, Message};
use simple_agent_type::prelude::{CompletionChunk, CompletionRequest, Result, SimpleAgentsError};
use simple_agent_type::request::{JsonSchemaFormat, ResponseFormat};
use simple_agent_type::response::{CompletionResponse, FinishReason, Usage};
use simple_agent_type::tool::{ToolChoice, ToolDefinition};
use simple_agents_core::CompletionOutcome;

pub(crate) type CompletionStream = Box<dyn Stream<Item = Result<CompletionChunk>> + Send + Unpin>;

pub(crate) struct ResponsePlan {
    pub(crate) response_format: Option<ResponseFormat>,
    pub(crate) schema_value: Option<Value>,
    pub(crate) expects_json: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_request_with_messages(
    model: &str,
    messages: Vec<Message>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    response_format: Option<ResponseFormat>,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<ToolChoice>,
    stream: Option<bool>,
) -> Result<CompletionRequest> {
    if model.is_empty() {
        return Err(SimpleAgentsError::Config(
            "model cannot be empty".to_string(),
        ));
    }
    if messages.is_empty() {
        return Err(SimpleAgentsError::Config(
            "messages cannot be empty".to_string(),
        ));
    }

    let mut builder = CompletionRequest::builder().model(model);
    for message in messages {
        builder = builder.message(message);
    }

    if let Some(max_tokens) = max_tokens {
        builder = builder.max_tokens(max_tokens);
    }
    if let Some(temperature) = temperature {
        builder = builder.temperature(temperature);
    }
    if let Some(top_p) = top_p {
        builder = builder.top_p(top_p);
    }
    if let Some(format) = response_format {
        builder = builder.response_format(format);
    }
    if let Some(tools) = tools {
        builder = builder.tools(tools);
    }
    if let Some(tool_choice) = tool_choice {
        builder = builder.tool_choice(tool_choice);
    }
    if let Some(stream) = stream {
        builder = builder.stream(stream);
    }

    builder.build()
}

pub(crate) fn resolve_response_plan(
    schema: Option<&Bound<'_, PyAny>>,
    schema_name: Option<String>,
    strict: bool,
    response_format: Option<String>,
) -> PyResult<ResponsePlan> {
    if let Some(schema_obj) = schema {
        let schema_json = schema_to_json_value(schema_obj)?;
        let schema_name = schema_name.unwrap_or_else(|| "schema".to_string());
        return Ok(ResponsePlan {
            response_format: Some(ResponseFormat::JsonSchema {
                json_schema: JsonSchemaFormat {
                    name: schema_name,
                    schema: schema_json.clone(),
                    strict: Some(strict),
                },
            }),
            schema_value: Some(schema_json),
            expects_json: true,
        });
    }

    if let Some(format) = response_format {
        match format.to_lowercase().as_str() {
            "json" | "json_object" => {
                return Ok(ResponsePlan {
                    response_format: Some(ResponseFormat::JsonObject),
                    schema_value: None,
                    expects_json: true,
                });
            }
            "text" => {
                return Ok(ResponsePlan {
                    response_format: None,
                    schema_value: None,
                    expects_json: false,
                });
            }
            _ => {
                return Err(PyRuntimeError::new_err(
                    "response_format must be 'json', 'json_object', or 'text'".to_string(),
                ));
            }
        }
    }

    Ok(ResponsePlan {
        response_format: None,
        schema_value: None,
        expects_json: false,
    })
}

pub(crate) fn expect_stream(outcome: CompletionOutcome) -> PyResult<CompletionStream> {
    match outcome {
        CompletionOutcome::Stream(stream) => Ok(stream),
        CompletionOutcome::Response(_) => Err(PyRuntimeError::new_err(
            "expected streaming response, got completion response".to_string(),
        )),
        CompletionOutcome::HealedJson(_) => Err(PyRuntimeError::new_err(
            "expected streaming response, got healed json response".to_string(),
        )),
        CompletionOutcome::CoercedSchema(_) => Err(PyRuntimeError::new_err(
            "expected streaming response, got schema response".to_string(),
        )),
    }
}

pub(crate) fn expect_healed_json(
    outcome: CompletionOutcome,
) -> PyResult<simple_agents_core::HealedJsonResponse> {
    match outcome {
        CompletionOutcome::HealedJson(healed) => Ok(healed),
        CompletionOutcome::Response(_) => Err(PyRuntimeError::new_err(
            "expected healed json response, got completion response".to_string(),
        )),
        CompletionOutcome::Stream(_) => Err(PyRuntimeError::new_err(
            "expected healed json response, got streaming response".to_string(),
        )),
        CompletionOutcome::CoercedSchema(_) => Err(PyRuntimeError::new_err(
            "expected healed json response, got schema response".to_string(),
        )),
    }
}

pub(crate) fn expect_response(outcome: CompletionOutcome) -> PyResult<CompletionResponse> {
    match outcome {
        CompletionOutcome::Response(response) => Ok(response),
        CompletionOutcome::Stream(_) => Err(PyRuntimeError::new_err(
            "expected completion response, got streaming response".to_string(),
        )),
        CompletionOutcome::HealedJson(_) => Err(PyRuntimeError::new_err(
            "expected completion response, got healed json response".to_string(),
        )),
        CompletionOutcome::CoercedSchema(_) => Err(PyRuntimeError::new_err(
            "expected completion response, got schema response".to_string(),
        )),
    }
}

pub(crate) fn healed_json_to_py(
    py: Python<'_>,
    healed: simple_agents_core::HealedJsonResponse,
    latency_ms: u64,
) -> PyResult<PyObject> {
    let content = serde_json::to_string(&healed.parsed.value)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize healed JSON: {}", e)))?;
    let raw_response = healed.response.content().unwrap_or_default().to_string();
    let confidence = healed.parsed.confidence;
    let was_healed = !healed.parsed.flags.is_empty();
    let flags = healed
        .parsed
        .flags
        .iter()
        .map(|f| f.description())
        .collect();
    let usage = usage_to_pydict(py, &healed.response.usage)?;
    let finish_reason = healed
        .response
        .choices
        .first()
        .map(|c| finish_reason_to_str(c.finish_reason).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let result = HealedJsonResult {
        content,
        raw_response,
        confidence,
        was_healed,
        provider: healed.response.provider.clone(),
        model: healed.response.model.clone(),
        finish_reason,
        created: healed.response.created,
        latency_ms,
        usage,
        flags,
    };

    Ok(Py::new(py, result)?.into_py(py))
}

pub(crate) fn response_with_metadata_from_response(
    py: Python<'_>,
    response: CompletionResponse,
    latency_ms: u64,
) -> PyResult<ResponseWithMetadata> {
    let usage = usage_to_pydict(py, &response.usage)?;
    let content = response.content().unwrap_or_default().to_string();
    let finish_reason = response
        .choices
        .first()
        .map(|c| finish_reason_to_str(c.finish_reason).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let (was_healed, healing_confidence, healing_error, flags) =
        if let Some(meta) = &response.healing_metadata {
            (
                true,
                Some(meta.confidence),
                Some(meta.original_error.clone()),
                meta.flags
                    .iter()
                    .map(|f: &simple_agent_type::coercion::CoercionFlag| f.description())
                    .collect::<Vec<String>>(),
            )
        } else {
            (false, None, None, Vec::new())
        };

    let tool_calls = response
        .choices
        .first()
        .and_then(|c| c.message.tool_calls.clone())
        .unwrap_or_default();
    let tool_calls_obj = pythonize::pythonize(py, &tool_calls)
        .map_err(|e| PyRuntimeError::new_err(format!("Conversion failed: {}", e)))?;

    Ok(ResponseWithMetadata {
        content,
        provider: response.provider.clone(),
        model: response.model.clone(),
        finish_reason,
        created: response.created,
        latency_ms,
        was_healed,
        healing_confidence,
        healing_error,
        flags,
        usage,
        tool_calls: tool_calls_obj.into(),
    })
}

pub(crate) fn finish_reason_to_str(reason: FinishReason) -> &'static str {
    reason.as_str()
}

pub(crate) fn parse_messages(messages: &Bound<'_, PyAny>) -> Result<Vec<Message>> {
    let value: Value = pythonize::depythonize(messages)
        .map_err(|_| SimpleAgentsError::Config("messages must be a list of dicts".to_string()))?;
    parse_messages_value(&value).map_err(SimpleAgentsError::Config)
}

pub(crate) fn parse_tools(tools: &Bound<'_, PyAny>) -> Result<Vec<ToolDefinition>> {
    pythonize::depythonize(tools).map_err(|_| {
        SimpleAgentsError::Config("tools must be a list of tool definitions".to_string())
    })
}

pub(crate) fn parse_tool_choice(tool_choice: &Bound<'_, PyAny>) -> Result<ToolChoice> {
    pythonize::depythonize(tool_choice).map_err(|_| {
        SimpleAgentsError::Config(
            "tool_choice must be a string (\"auto\"/\"none\") or a tool choice object".to_string(),
        )
    })
}

pub(crate) fn py_err(error: SimpleAgentsError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

pub(crate) fn usage_to_pydict(py: Python<'_>, usage: &Usage) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    dict.set_item("prompt_tokens", usage.prompt_tokens)?;
    dict.set_item("completion_tokens", usage.completion_tokens)?;
    dict.set_item("total_tokens", usage.total_tokens)?;
    Ok(dict.into())
}
