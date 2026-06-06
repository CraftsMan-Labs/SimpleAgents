use crate::ResponseWithMetadata;
use futures_util::Stream;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use simple_agent_type::message::{parse_messages_value, Message};
use simple_agent_type::prelude::{CompletionChunk, CompletionRequest, Result, SimpleAgentsError};
use simple_agent_type::request::{ReasoningEffort, ResponseFormat};
use simple_agent_type::response::{CompletionResponse, FinishReason, Usage};
use simple_agent_type::tool::{ToolChoice, ToolDefinition};
use simple_agents_core::{CompletionOutcome, HealedJsonResponse, HealedSchemaResponse};

pub(crate) type CompletionStream = Box<dyn Stream<Item = Result<CompletionChunk>> + Send + Unpin>;

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
    json_schema: Option<(String, serde_json::Value)>,
    reasoning_effort: Option<ReasoningEffort>,
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
    if let Some((name, schema)) = json_schema {
        builder = builder.json_schema(name, schema);
    } else if let Some(format) = response_format {
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
    if let Some(effort) = reasoning_effort {
        builder = builder.reasoning_effort(effort);
    }

    builder.build()
}

/// Parse a Python `str` or `int` into [`ReasoningEffort`].
pub(crate) fn resolve_reasoning_effort(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<ReasoningEffort>> {
    let Some(val) = value else {
        return Ok(None);
    };
    if let Ok(s) = val.extract::<String>() {
        let effort = serde_json::from_value::<ReasoningEffort>(serde_json::Value::String(s))
            .map_err(|e| PyRuntimeError::new_err(format!("invalid reasoning_effort: {e}")))?;
        Ok(Some(effort))
    } else if let Ok(n) = val.extract::<u32>() {
        Ok(Some(ReasoningEffort::Budget(n)))
    } else {
        Err(PyRuntimeError::new_err(
            "reasoning_effort must be a string (e.g. 'low', 'medium', 'high') or an integer budget",
        ))
    }
}

pub(crate) fn resolve_response_format(
    response_format: Option<String>,
) -> PyResult<Option<ResponseFormat>> {
    let Some(format) = response_format else {
        return Ok(None);
    };
    match format.to_lowercase().as_str() {
        "json" | "json_object" => Ok(Some(ResponseFormat::JsonObject)),
        "text" => Ok(None),
        _ => Err(PyRuntimeError::new_err(
            "response_format must be 'json', 'json_object', or 'text'",
        )),
    }
}

pub(crate) fn expect_stream(outcome: CompletionOutcome) -> PyResult<CompletionStream> {
    match outcome {
        CompletionOutcome::Stream(stream) => Ok(stream),
        _ => Err(PyRuntimeError::new_err("expected streaming response")),
    }
}

pub(crate) fn expect_response(outcome: CompletionOutcome) -> PyResult<CompletionResponse> {
    match outcome {
        CompletionOutcome::Response(response) => Ok(response),
        _ => Err(PyRuntimeError::new_err("expected completion response")),
    }
}

pub(crate) fn expect_healed_json(outcome: CompletionOutcome) -> PyResult<HealedJsonResponse> {
    match outcome {
        CompletionOutcome::HealedJson(healed) => Ok(healed),
        _ => Err(PyRuntimeError::new_err("expected healed JSON response")),
    }
}

pub(crate) fn expect_coerced_schema(outcome: CompletionOutcome) -> PyResult<HealedSchemaResponse> {
    match outcome {
        CompletionOutcome::CoercedSchema(healed) => Ok(healed),
        _ => Err(PyRuntimeError::new_err("expected schema-coerced response")),
    }
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
        usage,
        tool_calls: tool_calls_obj.into(),
    })
}

pub(crate) fn finish_reason_to_str(reason: FinishReason) -> &'static str {
    reason.as_str()
}

pub(crate) fn parse_messages(messages: &Bound<'_, PyAny>) -> Result<Vec<Message>> {
    let value: serde_json::Value = pythonize::depythonize(messages)
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
    dict.set_item("reasoning_tokens", usage.reasoning_tokens)?;
    Ok(dict.into())
}
