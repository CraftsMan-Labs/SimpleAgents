//! Python bindings for SimpleAgents using PyO3.

#![allow(clippy::useless_conversion)]

use futures_util::{Stream, StreamExt};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{CompletionChunk, Result};
use simple_agent_type::provider::RetryConfig;
use simple_agent_type::request::ResponseFormat;
use simple_agents_core::{
    ClientConfig, CompletionMode, CompletionOptions, HealedJsonResponse, SimpleAgentsClient,
};
use simple_agents_healing::schema::Schema;
use simple_agents_healing::{CoercionEngine, JsonishParser};
use simple_agents_providers::schema_converter;
use simple_agents_workflow::yaml_runner::workflow_execution;
use simple_agents_workflow::yaml_runner::{
    YamlWorkflowExecutionRequest, YamlWorkflowExecutorBinding, YamlWorkflowSource,
};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod client_builder;
mod completion_helpers;
mod provider_helpers;
mod workflow_helpers;

use client_builder::{ClientBuilder, ProviderConfig};
use completion_helpers::{
    build_request_with_messages, expect_coerced_schema, expect_healed_json, expect_response,
    expect_stream, finish_reason_to_str, parse_messages, parse_tool_choice, parse_tools, py_err,
    response_with_metadata_from_response, usage_to_pydict,
};
use provider_helpers::build_provider_from_name;
use simple_agents_workflow::evals::{run_eval_suite, EvalSuiteRunRequest};
use simple_agents_workflow::YamlWorkflowRunOptions;
use workflow_helpers::{
    attach_workflow_events, build_workflow_input_from_execution_request,
    parse_workflow_execution_request, workflow_execution_flags, workflow_execution_options,
    workflow_root_path, CombinedWorkflowEventSink, PythonCustomWorkerExecutor,
    PythonWorkflowEventSink, RecordingWorkflowEventSink,
};

fn parse_python_schema(schema: &Bound<'_, PyAny>) -> PyResult<(serde_json::Value, Schema)> {
    let json_value: serde_json::Value = if schema.downcast::<PyDict>().is_ok() {
        pythonize::depythonize(schema).map_err(|e| PyRuntimeError::new_err(e.to_string()))?
    } else if schema.hasattr("model_json_schema")? {
        let schema_dict = schema.call_method0("model_json_schema")?;
        pythonize::depythonize(&schema_dict).map_err(|e| PyRuntimeError::new_err(e.to_string()))?
    } else {
        return Err(PyRuntimeError::new_err(
            "schema must be a dict or a Pydantic model class",
        ));
    };
    let sch = schema_converter::convert(&json_value)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok((json_value, sch))
}

fn parse_timeout_seconds(timeout_seconds: Option<f64>) -> PyResult<Option<Duration>> {
    match timeout_seconds {
        Some(value) if value.is_finite() && value > 0.0 => Ok(Some(Duration::from_secs_f64(value))),
        Some(_) => Err(PyValueError::new_err(
            "timeout_seconds must be a positive finite number",
        )),
        None => Ok(None),
    }
}

fn parse_retry_strategy(retry_strategy: Option<&str>) -> PyResult<Option<f32>> {
    match retry_strategy {
        Some("none") => Ok(None),
        Some("fixed") => Ok(Some(1.0)),
        Some("exponential") | None => Ok(Some(2.0)),
        Some(other) => Err(PyValueError::new_err(format!(
            "unknown retry_strategy '{other}'; expected 'none', 'fixed', or 'exponential'"
        ))),
    }
}

fn parse_retry_config(
    retry_attempts: Option<u32>,
    retry_strategy: Option<&str>,
) -> PyResult<RetryConfig> {
    let mut retry = RetryConfig::default();
    if let Some(attempts) = retry_attempts {
        if attempts == 0 {
            return Err(PyValueError::new_err(
                "retry_attempts must be greater than or equal to 1",
            ));
        }
        retry.max_attempts = attempts;
    }
    match parse_retry_strategy(retry_strategy)? {
        Some(multiplier) => {
            retry.backoff_multiplier = multiplier;
            retry.jitter = false;
        }
        None => retry.max_attempts = 1,
    }
    Ok(retry)
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientCreateRequest {
    provider: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    api_base: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    api_format: Option<String>,
    #[serde(default)]
    timeout_seconds: Option<f64>,
    #[serde(default)]
    retry_attempts: Option<u32>,
    #[serde(default)]
    retry_strategy: Option<String>,
}

fn py_client_config_error(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(format!("invalid Client request: {error}"))
}

fn depythonize_client_request_mapping(
    value: &Bound<'_, PyAny>,
) -> PyResult<serde_json::Map<String, serde_json::Value>> {
    let value: serde_json::Value = pythonize::depythonize(value).map_err(py_client_config_error)?;
    match value {
        serde_json::Value::Object(map) => Ok(map),
        _ => Err(PyValueError::new_err(
            "Client request must be a mapping or provider string",
        )),
    }
}

fn parse_client_create_request(
    request: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<ClientCreateRequest> {
    let mut raw = match request {
        Some(request) => {
            if let Ok(provider) = request.extract::<String>() {
                let mut raw = serde_json::Map::new();
                raw.insert("provider".to_string(), serde_json::Value::String(provider));
                raw
            } else {
                if kwargs.is_some_and(|kwargs| !kwargs.is_empty()) {
                    return Err(PyValueError::new_err(
                        "Client request mapping cannot be combined with keyword options",
                    ));
                }
                depythonize_client_request_mapping(request)?
            }
        }
        None => serde_json::Map::new(),
    };

    if let Some(kwargs) = kwargs {
        let keyword_options = depythonize_client_request_mapping(kwargs.as_any())?;
        for (key, value) in keyword_options {
            if raw.insert(key.clone(), value).is_some() {
                return Err(PyValueError::new_err(format!(
                    "Client request field '{key}' was provided more than once"
                )));
            }
        }
    }

    serde_json::from_value(serde_json::Value::Object(raw)).map_err(py_client_config_error)
}

fn healed_json_response_to_py(
    py: Python<'_>,
    healed: HealedJsonResponse,
) -> PyResult<HealedJsonResult> {
    let raw_response = healed.response.content().unwrap_or_default().to_string();
    let content = serde_json::to_string(&healed.parsed.value)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let confidence = healed.parsed.confidence as f64;
    let was_healed = !healed.parsed.flags.is_empty();
    let flags: Vec<String> = healed
        .parsed
        .flags
        .iter()
        .map(|f| f.description())
        .collect();
    let usage = usage_to_pydict(py, &healed.response.usage)?;
    Ok(HealedJsonResult {
        content,
        confidence,
        was_healed,
        flags,
        raw_response,
        usage: usage.into_py(py),
    })
}

type Runtime = tokio::runtime::Runtime;

// ---------------------------------------------------------------------------
// Typed message classes
// ---------------------------------------------------------------------------

/// LLM conversation role.
#[pyclass(eq, eq_int)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Role {
    User,
    System,
    Assistant,
    Tool,
}

#[pymethods]
impl Role {
    fn __repr__(&self) -> String {
        match self {
            Role::User => "Role.User".to_string(),
            Role::System => "Role.System".to_string(),
            Role::Assistant => "Role.Assistant".to_string(),
            Role::Tool => "Role.Tool".to_string(),
        }
    }
}

/// A single content part (text or image).
#[pyclass]
#[derive(Clone, Debug)]
pub struct ContentPart {
    inner: simple_agent_type::message::ContentPart,
}

#[pymethods]
impl ContentPart {
    /// Create a text content part.
    #[staticmethod]
    fn text(text: &str) -> Self {
        ContentPart {
            inner: simple_agent_type::message::ContentPart::Text {
                text: text.to_string(),
            },
        }
    }

    /// Create an image_url content part.
    #[staticmethod]
    fn image_url(url: &str) -> Self {
        ContentPart {
            inner: simple_agent_type::message::ContentPart::image_url(url),
        }
    }

    /// Create an image content part from base64 data (MIME type + base64 string).
    #[staticmethod]
    fn image(media_type: &str, data: &str) -> Self {
        ContentPart {
            inner: simple_agent_type::message::ContentPart::image(media_type, data),
        }
    }

    /// Create an audio content part from base64 data.
    #[staticmethod]
    fn audio(media_type: &str, data: &str) -> Self {
        ContentPart {
            inner: simple_agent_type::message::ContentPart::audio(media_type, data),
        }
    }

    /// Create a video content part from base64 data.
    #[staticmethod]
    fn video(media_type: &str, data: &str) -> Self {
        ContentPart {
            inner: simple_agent_type::message::ContentPart::video(media_type, data),
        }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            simple_agent_type::message::ContentPart::Text { text } => {
                format!(
                    "ContentPart.text({:?})",
                    &text.chars().take(40).collect::<String>()
                )
            }
            simple_agent_type::message::ContentPart::ImageUrl { image_url } => {
                format!(
                    "ContentPart.image_url({:?})",
                    &image_url.url.chars().take(60).collect::<String>()
                )
            }
            simple_agent_type::message::ContentPart::Audio { input_audio } => {
                format!(
                    "ContentPart.audio({:?}, <{} bytes>)",
                    input_audio.media_type,
                    input_audio.data.len()
                )
            }
            simple_agent_type::message::ContentPart::Video { video } => {
                format!(
                    "ContentPart.video({:?}, <{} bytes>)",
                    video.media_type,
                    video.data.len()
                )
            }
        }
    }
}

/// A typed conversation message.
#[pyclass]
#[derive(Clone, Debug)]
pub struct PyMessage {
    inner: Message,
}

#[pymethods]
impl PyMessage {
    /// Create a user message with a plain text string.
    #[staticmethod]
    fn user(content: &str) -> Self {
        PyMessage {
            inner: Message::user(content),
        }
    }

    /// Create a system message.
    #[staticmethod]
    fn system(content: &str) -> Self {
        PyMessage {
            inner: Message::system(content),
        }
    }

    /// Create an assistant message.
    #[staticmethod]
    fn assistant(content: &str) -> Self {
        PyMessage {
            inner: Message::assistant(content),
        }
    }

    /// Create a user message with typed content parts (text + images).
    #[staticmethod]
    fn user_parts(parts: Vec<PyRef<'_, ContentPart>>) -> Self {
        let content: Vec<simple_agent_type::message::ContentPart> =
            parts.iter().map(|p| p.inner.clone()).collect();
        PyMessage {
            inner: Message {
                role: simple_agent_type::message::Role::User,
                content: simple_agent_type::message::MessageContent::Parts(content),
                name: None,
                tool_call_id: None,
                tool_calls: None,
            },
        }
    }

    #[getter]
    fn role(&self) -> Role {
        match self.inner.role {
            simple_agent_type::message::Role::User => Role::User,
            simple_agent_type::message::Role::System => Role::System,
            simple_agent_type::message::Role::Assistant => Role::Assistant,
            simple_agent_type::message::Role::Tool => Role::Tool,
        }
    }

    fn __repr__(&self) -> String {
        format!("Message(role={:?})", self.inner.role)
    }
}

/// A single chunk from a streaming completion.
#[pyclass]
pub struct StreamChunk {
    #[pyo3(get)]
    content: String,
    #[pyo3(get)]
    finish_reason: Option<String>,
    #[pyo3(get)]
    model: String,
    #[pyo3(get)]
    index: u32,
}

#[pymethods]
impl StreamChunk {
    fn __repr__(&self) -> String {
        if let Some(reason) = &self.finish_reason {
            format!(
                "StreamChunk(content={:?}..., finish_reason={:?})",
                &self.content.chars().take(30).collect::<String>(),
                reason
            )
        } else {
            format!(
                "StreamChunk(content={:?}...)",
                &self.content.chars().take(30).collect::<String>()
            )
        }
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Streaming iterator that yields StreamChunk objects.
#[pyclass]
pub struct PyStreamIterator {
    stream: Option<Pin<Box<dyn Stream<Item = Result<CompletionChunk>> + Send>>>,
    runtime: Arc<Mutex<Runtime>>,
}

/// Streaming iterator that accumulates chunks, heals JSON, and yields a PyStructuredEvent.
#[pyclass]
pub struct PyStructuredStreamIterator {
    stream: Option<Pin<Box<dyn Stream<Item = Result<CompletionChunk>> + Send>>>,
    runtime: Arc<Mutex<Runtime>>,
    buffer: String,
    schema: Option<Schema>,
}

#[pymethods]
impl PyStructuredStreamIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<PyObject>> {
        let runtime = Arc::clone(&slf.runtime);
        let stream = slf
            .stream
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Stream exhausted"))?;

        let next = py.allow_threads(|| {
            let runtime = runtime
                .lock()
                .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
            Ok::<_, PyErr>(runtime.block_on(stream.next()))
        })?;

        let Some(next_item) = next else {
            slf.stream = None;
            return Ok(None);
        };

        let chunk = next_item.map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let is_complete = chunk
            .choices
            .first()
            .and_then(|choice| choice.finish_reason)
            .is_some();
        if let Some(content) = chunk
            .choices
            .first()
            .and_then(|choice| choice.delta.content.as_ref())
        {
            slf.buffer.push_str(content);
        }

        let (json_val, confidence, was_healed) = if slf.buffer.trim().is_empty() {
            (serde_json::Value::Null, 0.0_f64, false)
        } else {
            match JsonishParser::new().parse(&slf.buffer) {
                Ok(r) => (r.value, r.confidence as f64, !r.flags.is_empty()),
                Err(_) => (serde_json::Value::String(slf.buffer.clone()), 0.0, false),
            }
        };
        let coerced_value = if let Some(schema) = slf.schema.as_ref() {
            CoercionEngine::new()
                .coerce(&json_val, schema)
                .ok()
                .map(|coerced| coerced.value)
        } else {
            None
        };
        let py_value = serde_json_to_py(py, &json_val)?;
        let py_coerced_value = match coerced_value.as_ref() {
            Some(value) => serde_json_to_py(py, value)?,
            None => py.None(),
        };
        let event = PyStructuredEvent {
            is_partial: !is_complete,
            is_complete,
            value: py_value.clone_ref(py),
            partial_value: py_value,
            confidence,
            was_healed,
            coerced_value: py_coerced_value,
            coerced_confidence: if coerced_value.is_some() {
                Some(confidence)
            } else {
                None
            },
            coercion_flags: Vec::new(),
        };
        Ok(Some(Py::new(py, event)?.into_py(py)))
    }

    fn __repr__(&self) -> String {
        format!(
            "PyStructuredStreamIterator(active={})",
            self.stream.is_some(),
        )
    }
}

#[pymethods]
impl PyStreamIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<StreamChunk>> {
        let runtime = Arc::clone(&slf.runtime);
        let stream = slf
            .stream
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Stream exhausted"))?;

        let result = py.allow_threads(|| {
            let runtime = runtime
                .lock()
                .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
            Ok::<_, PyErr>(runtime.block_on(stream.next()))
        })?;

        match result {
            Some(Ok(chunk)) => {
                let content = chunk
                    .choices
                    .first()
                    .and_then(|c| c.delta.content.clone())
                    .unwrap_or_default();

                let finish_reason = chunk
                    .choices
                    .first()
                    .and_then(|c| c.finish_reason)
                    .map(|fr| finish_reason_to_str(fr).to_string());

                Ok(Some(StreamChunk {
                    content,
                    finish_reason,
                    model: chunk.model,
                    index: chunk.choices.first().map(|c| c.index).unwrap_or(0),
                }))
            }
            Some(Err(e)) => Err(PyRuntimeError::new_err(e.to_string())),
            None => {
                slf.stream = None;
                Ok(None)
            }
        }
    }

    fn __repr__(&self) -> String {
        format!("PyStreamIterator(active={})", self.stream.is_some())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Completion response with metadata.
#[pyclass]
pub struct ResponseWithMetadata {
    #[pyo3(get)]
    pub(crate) content: String,
    #[pyo3(get)]
    pub(crate) provider: Option<String>,
    #[pyo3(get)]
    pub(crate) model: String,
    #[pyo3(get)]
    pub(crate) finish_reason: String,
    #[pyo3(get)]
    pub(crate) created: Option<i64>,
    #[pyo3(get)]
    pub(crate) latency_ms: u64,
    #[pyo3(get)]
    pub(crate) tool_calls: PyObject,
    pub(crate) usage: PyObject,
}

#[pymethods]
impl ResponseWithMetadata {
    #[getter]
    fn usage(&self, py: Python<'_>) -> PyObject {
        self.usage.clone_ref(py).into()
    }

    fn __repr__(&self) -> String {
        format!(
            "ResponseWithMetadata(model={:?}, provider={:?}, latency_ms={})",
            self.model, self.provider, self.latency_ms
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

#[pyclass]
pub(crate) struct Client {
    runtime: Arc<Mutex<Runtime>>,
    client: SimpleAgentsClient,
}

impl Client {
    pub(crate) fn from_parts(runtime: Arc<Mutex<Runtime>>, client: SimpleAgentsClient) -> Self {
        Self { runtime, client }
    }
}

#[pymethods]
#[allow(clippy::useless_conversion)]
impl Client {
    /// Create a new Client.
    ///
    /// The first argument is the provider name (currently only "openai" is supported).
    /// When `api_key` is supplied as a keyword argument it is used directly;
    /// otherwise the `OPENAI_API_KEY` environment variable is read.
    /// `api_base` and `base_url` are synonymous base-URL overrides.
    #[new]
    #[pyo3(signature = (request=None, **kwargs))]
    fn new(
        request: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let request = parse_client_create_request(request, kwargs)?;
        if request.model.is_some() {
            return Err(PyValueError::new_err(
                "Client(model=...) is not supported yet; pass the model per request via completion model or workflow execution.model",
            ));
        }
        let ClientCreateRequest {
            provider,
            api_key,
            api_base,
            base_url,
            model: _,
            api_format,
            timeout_seconds,
            retry_attempts,
            retry_strategy,
        } = request;
        let timeout = parse_timeout_seconds(timeout_seconds)?;
        let retry = parse_retry_config(retry_attempts, retry_strategy.as_deref())?;
        let effective_base = api_base.or(base_url);
        let prov = build_provider_from_name(
            provider.as_str(),
            api_key.as_deref(),
            effective_base.as_deref(),
            api_format.as_deref(),
            timeout,
        )
        .map_err(py_err)?;
        let config = ClientConfig {
            provider,
            api_key: api_key.unwrap_or_default(),
            base_url: effective_base,
            default_retry: retry,
            ..Default::default()
        };
        let client = SimpleAgentsClient::from_config(prov, config);
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            client,
        })
    }

    /// Send a completion request.
    #[pyo3(signature = (model, input, max_tokens=None, temperature=None, top_p=None, tools=None, tool_choice=None, response_format=None, heal=None, stream=None, schema=None, schema_name=None, send_schema=None))]
    #[allow(clippy::too_many_arguments)]
    fn complete(
        &self,
        py: Python<'_>,
        model: &str,
        input: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        tools: Option<&Bound<'_, PyAny>>,
        tool_choice: Option<&Bound<'_, PyAny>>,
        response_format: Option<String>,
        heal: Option<bool>,
        stream: Option<bool>,
        schema: Option<&Bound<'_, PyAny>>,
        schema_name: Option<&str>,
        send_schema: Option<bool>,
    ) -> PyResult<PyObject> {
        let messages = if let Ok(prompt) = input.extract::<&str>() {
            if prompt.is_empty() {
                return Err(PyRuntimeError::new_err("prompt cannot be empty"));
            }
            vec![Message::user(prompt)]
        } else {
            parse_messages(input).map_err(py_err)?
        };

        if stream.unwrap_or(false) {
            let mut resolved_schema: Option<Schema> = None;
            let mut json_schema_pair: Option<(String, serde_json::Value)> = None;
            if let Some(schema_ref) = schema {
                let (json_schema_value, healing_schema) = parse_python_schema(schema_ref)?;
                resolved_schema = Some(healing_schema);
                if send_schema.unwrap_or(false) {
                    let name = schema_name.unwrap_or("structured_output").to_string();
                    json_schema_pair = Some((name, json_schema_value));
                }
            }
            let request = build_request_with_messages(
                model,
                messages,
                max_tokens,
                temperature,
                top_p,
                None,
                None,
                None,
                Some(true),
                json_schema_pair,
            )
            .map_err(py_err)?;
            let outcome = {
                let runtime = self
                    .runtime
                    .lock()
                    .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
                py.allow_threads(|| {
                    runtime.block_on(self.client.complete(&request, CompletionOptions::default()))
                })
                .map_err(py_err)?
            };
            let underlying_stream = expect_stream(outcome)?;
            if schema.is_some() {
                let iter = PyStructuredStreamIterator {
                    stream: Some(Box::pin(underlying_stream)),
                    runtime: Arc::clone(&self.runtime),
                    buffer: String::new(),
                    schema: resolved_schema,
                };
                return Ok(Bound::new(py, iter)?.into_any().into_py(py));
            }
            let iter = PyStreamIterator {
                stream: Some(Box::pin(underlying_stream)),
                runtime: Arc::clone(&self.runtime),
            };
            return Ok(Bound::new(py, iter)?.into_any().into_py(py));
        }

        let tools = match tools {
            Some(t) => Some(parse_tools(t).map_err(py_err)?),
            None => None,
        };
        let tool_choice = match tool_choice {
            Some(tc) => Some(parse_tool_choice(tc).map_err(py_err)?),
            None => None,
        };

        if let Some(schema_ref) = schema {
            let (json_schema_value, healing_schema) = parse_python_schema(schema_ref)?;
            let json_schema_pair = if send_schema.unwrap_or(false) {
                Some((
                    schema_name.unwrap_or("structured_output").to_string(),
                    json_schema_value,
                ))
            } else {
                None
            };
            let response_format_for_req = if json_schema_pair.is_some() {
                None
            } else {
                Some(ResponseFormat::JsonObject)
            };
            let request = build_request_with_messages(
                model,
                messages,
                max_tokens,
                temperature,
                top_p,
                response_format_for_req,
                tools,
                tool_choice,
                None,
                json_schema_pair,
            )
            .map_err(py_err)?;
            let options = CompletionOptions {
                mode: CompletionMode::CoercedSchema(healing_schema),
            };
            let outcome = {
                let runtime = self
                    .runtime
                    .lock()
                    .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
                py.allow_threads(|| runtime.block_on(self.client.complete(&request, options)))
                    .map_err(py_err)?
            };
            let coerced = expect_coerced_schema(outcome)?;
            let json_str = serde_json::to_string(&coerced.coerced.value)
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            return Ok(json_str.into_py(py));
        }

        if heal.unwrap_or(false) {
            let resp_format = completion_helpers::resolve_response_format(response_format)?;
            let merged_format = match resp_format {
                Some(f) => Some(f),
                None => Some(ResponseFormat::JsonObject),
            };
            let request = build_request_with_messages(
                model,
                messages,
                max_tokens,
                temperature,
                top_p,
                merged_format,
                tools,
                tool_choice,
                None,
                None,
            )
            .map_err(py_err)?;
            let options = CompletionOptions {
                mode: CompletionMode::HealedJson,
            };
            let outcome = {
                let runtime = self
                    .runtime
                    .lock()
                    .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
                py.allow_threads(|| runtime.block_on(self.client.complete(&request, options)))
                    .map_err(py_err)?
            };
            let healed = expect_healed_json(outcome)?;
            let healed_py = healed_json_response_to_py(py, healed)?;
            return Ok(Py::new(py, healed_py)?.into_py(py));
        }

        let resp_format = completion_helpers::resolve_response_format(response_format)?;

        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            resp_format,
            tools,
            tool_choice,
            None,
            None,
        )
        .map_err(py_err)?;

        let start = Instant::now();
        let outcome = {
            let runtime = self
                .runtime
                .lock()
                .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
            py.allow_threads(|| {
                runtime.block_on(self.client.complete(&request, CompletionOptions::default()))
            })
            .map_err(py_err)?
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        let response = expect_response(outcome)?;
        let response_with_metadata =
            response_with_metadata_from_response(py, response, latency_ms)?;
        Ok(Py::new(py, response_with_metadata)?.into_py(py))
    }

    /// Send a streaming completion request.
    #[pyo3(signature = (model, input, max_tokens=None, temperature=None, top_p=None))]
    fn stream_complete(
        &self,
        py: Python<'_>,
        model: &str,
        input: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<PyObject> {
        let messages = if let Ok(prompt) = input.extract::<&str>() {
            if prompt.is_empty() {
                return Err(PyRuntimeError::new_err("prompt cannot be empty"));
            }
            vec![Message::user(prompt)]
        } else {
            parse_messages(input).map_err(py_err)?
        };

        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            None,
            None,
            None,
            Some(true),
            None,
        )
        .map_err(py_err)?;

        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;

        let outcome = py
            .allow_threads(|| {
                runtime.block_on(self.client.complete(&request, CompletionOptions::default()))
            })
            .map_err(py_err)?;
        let stream = expect_stream(outcome)?;

        let iterator = PyStreamIterator {
            stream: Some(Box::pin(stream)),
            runtime: Arc::clone(&self.runtime),
        };
        Ok(Bound::new(py, iterator)?.into_any().into_py(py))
    }

    /// Run a YAML workflow (blocking).
    #[pyo3(signature = (request))]
    fn run_workflow(&self, py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        let request = parse_workflow_execution_request(request)?;
        let workflow_path_buf = std::path::PathBuf::from(request.workflow_path.as_str());
        let workflow_root = workflow_root_path(workflow_path_buf.as_path());
        let workflow_input_value = build_workflow_input_from_execution_request(&request)?;
        let flags = workflow_execution_flags(&request.execution);
        let options = workflow_execution_options(&request);

        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let custom_executor = PythonCustomWorkerExecutor { workflow_root };
        let execution_request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(workflow_path_buf.as_path()),
            workflow_input: &workflow_input_value,
            executor: YamlWorkflowExecutorBinding::Client(&self.client),
            custom_worker: Some(&custom_executor),
            options: &options,
            flags,
        };

        let output = py
            .allow_threads(|| runtime.block_on(workflow_execution::run(execution_request)))
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;

        let value = serde_json::to_value(output)
            .map_err(|error| PyRuntimeError::new_err(format!("serialization failed: {error}")))?;
        let py_value = pythonize::pythonize(py, &value)
            .map_err(|error| PyRuntimeError::new_err(format!("pythonize failed: {error}")))?;
        Ok(py_value.into_py(py))
    }

    /// Run an output-shaped eval dataset against a YAML workflow.
    #[pyo3(signature = (request))]
    fn run_eval_suite(&self, py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        let raw: serde_json::Value = pythonize::depythonize(request).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "invalid eval suite request: {error}. expected keys: suite_path"
            ))
        })?;
        let object = raw.as_object().ok_or_else(|| {
            PyRuntimeError::new_err("eval suite request must be a dict/object".to_string())
        })?;
        if object.keys().any(|key| key != "suite_path") {
            return Err(PyRuntimeError::new_err(
                "eval suite request only supports suite_path".to_string(),
            ));
        }
        let suite_path = object
            .get("suite_path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| PyRuntimeError::new_err("suite_path is required".to_string()))?;
        if suite_path.trim().is_empty() {
            return Err(PyRuntimeError::new_err("suite_path cannot be empty"));
        }

        let suite_path_buf = std::path::PathBuf::from(suite_path);
        let workflow_root = suite_path_buf
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let custom_executor = PythonCustomWorkerExecutor { workflow_root };
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let request = EvalSuiteRunRequest {
            suite_path: suite_path_buf.as_path(),
            executor: YamlWorkflowExecutorBinding::Client(&self.client),
            custom_worker: Some(&custom_executor),
        };
        let report = py
            .allow_threads(|| runtime.block_on(run_eval_suite(request)))
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let value = serde_json::to_value(report)
            .map_err(|error| PyRuntimeError::new_err(format!("serialization failed: {error}")))?;
        let py_value = pythonize::pythonize(py, &value)
            .map_err(|error| PyRuntimeError::new_err(format!("pythonize failed: {error}")))?;
        Ok(py_value.into_py(py))
    }

    /// Run a YAML workflow with streaming events.
    #[pyo3(signature = (request, on_event=None, include_events_in_output=false))]
    fn stream_workflow(
        &self,
        py: Python<'_>,
        request: &Bound<'_, PyAny>,
        on_event: Option<Py<PyAny>>,
        include_events_in_output: bool,
    ) -> PyResult<PyObject> {
        let request = parse_workflow_execution_request(request)?;
        let workflow_path_buf = std::path::PathBuf::from(request.workflow_path.as_str());
        let workflow_root = workflow_root_path(workflow_path_buf.as_path());
        let workflow_input_value = build_workflow_input_from_execution_request(&request)?;
        let mut flags = workflow_execution_flags(&request.execution);
        let options = workflow_execution_options(&request);
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let custom_executor = PythonCustomWorkerExecutor { workflow_root };

        if let Some(callback) = on_event {
            flags.workflow_streaming = true;
            let output = if include_events_in_output {
                let event_sink = CombinedWorkflowEventSink::new(true, Some(callback));
                let execution_request = YamlWorkflowExecutionRequest {
                    source: YamlWorkflowSource::File(workflow_path_buf.as_path()),
                    workflow_input: &workflow_input_value,
                    executor: YamlWorkflowExecutorBinding::Client(&self.client),
                    custom_worker: Some(&custom_executor),
                    options: &options,
                    flags,
                };
                let output = py
                    .allow_threads(|| {
                        runtime.block_on(workflow_execution::stream(execution_request, &event_sink))
                    })
                    .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
                let mut value = serde_json::to_value(output).map_err(|error| {
                    PyRuntimeError::new_err(format!("serialization failed: {error}"))
                })?;
                event_sink.attach_to_output(&mut value)?;
                value
            } else {
                let event_sink = PythonWorkflowEventSink {
                    callback: Some(callback),
                    callback_error: std::sync::Mutex::new(None),
                };
                let execution_request = YamlWorkflowExecutionRequest {
                    source: YamlWorkflowSource::File(workflow_path_buf.as_path()),
                    workflow_input: &workflow_input_value,
                    executor: YamlWorkflowExecutorBinding::Client(&self.client),
                    custom_worker: Some(&custom_executor),
                    options: &options,
                    flags,
                };
                let output = py
                    .allow_threads(|| {
                        runtime.block_on(workflow_execution::stream(execution_request, &event_sink))
                    })
                    .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
                serde_json::to_value(output).map_err(|error| {
                    PyRuntimeError::new_err(format!("serialization failed: {error}"))
                })?
            };
            let py_value = pythonize::pythonize(py, &output)
                .map_err(|error| PyRuntimeError::new_err(format!("pythonize failed: {error}")))?;
            return Ok(py_value.into_py(py));
        }

        let recording_sink = RecordingWorkflowEventSink::new();
        let execution_request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(workflow_path_buf.as_path()),
            workflow_input: &workflow_input_value,
            executor: YamlWorkflowExecutorBinding::Client(&self.client),
            custom_worker: Some(&custom_executor),
            options: &options,
            flags,
        };
        let output = py
            .allow_threads(|| {
                runtime.block_on(workflow_execution::stream(
                    execution_request,
                    &recording_sink,
                ))
            })
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let mut value = serde_json::to_value(output)
            .map_err(|error| PyRuntimeError::new_err(format!("serialization failed: {error}")))?;
        attach_workflow_events(&mut value, &recording_sink)?;
        let py_value = pythonize::pythonize(py, &value)
            .map_err(|error| PyRuntimeError::new_err(format!("pythonize failed: {error}")))?;
        Ok(py_value.into_py(py))
    }

    /// Resume a workflow from a checkpoint dict.
    #[pyo3(signature = (checkpoint, *, options=None))]
    fn resume(
        &self,
        py: Python<'_>,
        checkpoint: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        // Extract workflow_path and messages from the checkpoint dict, then delegate to run()
        let checkpoint_val: serde_json::Value = pythonize::depythonize(checkpoint)
            .map_err(|e| PyRuntimeError::new_err(format!("invalid checkpoint: {e}")))?;
        let workflow_path = checkpoint_val
            .get("workflow_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PyRuntimeError::new_err("checkpoint must have workflow_path"))?
            .to_string();

        let messages_val = checkpoint_val
            .get("original_messages")
            .cloned()
            .unwrap_or(serde_json::json!([]));

        let workflow_input = serde_json::json!({ "messages": messages_val });
        let workflow_path_buf = std::path::PathBuf::from(&workflow_path);
        let workflow_root = workflow_root_path(workflow_path_buf.as_path());
        let run_options = if let Some(options) = options {
            let value: serde_json::Value = pythonize::depythonize(options)
                .map_err(|e| PyRuntimeError::new_err(format!("invalid resume options: {e}")))?;
            let object = value
                .as_object()
                .ok_or_else(|| PyRuntimeError::new_err("resume options must be a dict/object"))?;
            for key in object.keys() {
                if key != "workflow_options" && key != "workflowOptions" {
                    return Err(PyRuntimeError::new_err(format!(
                        "resume options only supports workflow_options/workflowOptions; unknown key '{key}'"
                    )));
                }
            }
            let workflow_options = object
                .get("workflow_options")
                .or_else(|| object.get("workflowOptions"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if workflow_options.is_null() {
                YamlWorkflowRunOptions::default()
            } else {
                serde_json::from_value::<YamlWorkflowRunOptions>(workflow_options).map_err(|e| {
                    PyRuntimeError::new_err(format!("invalid workflow_options: {e}"))
                })?
            }
        } else {
            YamlWorkflowRunOptions::default()
        };
        let flags = simple_agents_workflow::yaml_runner::YamlWorkflowExecutionFlags::default();
        let custom_executor = PythonCustomWorkerExecutor { workflow_root };
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;

        let execution_request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(workflow_path_buf.as_path()),
            workflow_input: &workflow_input,
            executor: YamlWorkflowExecutorBinding::Client(&self.client),
            custom_worker: Some(&custom_executor),
            options: &run_options,
            flags,
        };

        let output = py
            .allow_threads(|| runtime.block_on(workflow_execution::run(execution_request)))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let value = serde_json::to_value(output)
            .map_err(|e| PyRuntimeError::new_err(format!("serialization failed: {e}")))?;
        let py_value = pythonize::pythonize(py, &value)
            .map_err(|e| PyRuntimeError::new_err(format!("pythonize failed: {e}")))?;
        Ok(py_value.into_py(py))
    }
}

// ---------------------------------------------------------------------------
// Healing result types
// ---------------------------------------------------------------------------

/// Result of parsing/healing a raw JSON string.
#[pyclass]
pub struct ParseResult {
    #[pyo3(get)]
    pub value: PyObject,
    #[pyo3(get)]
    pub confidence: f64,
    #[pyo3(get)]
    pub was_healed: bool,
    #[pyo3(get)]
    pub flags: Vec<String>,
}

#[pymethods]
impl ParseResult {
    #[new]
    #[pyo3(signature = (value, confidence, was_healed, flags))]
    fn new(
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        confidence: f64,
        was_healed: bool,
        flags: Vec<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            value: value.into_py(py),
            confidence,
            was_healed,
            flags,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "ParseResult(confidence={:.2}, was_healed={}, flags={})",
            self.confidence,
            self.was_healed,
            self.flags.len()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Result of coercing a value to a schema.
#[pyclass]
pub struct PyCoercionResult {
    #[pyo3(get)]
    pub value: PyObject,
    #[pyo3(get)]
    pub confidence: f64,
    #[pyo3(get)]
    pub was_coerced: bool,
    #[pyo3(get)]
    pub flags: Vec<String>,
}

#[pymethods]
impl PyCoercionResult {
    #[new]
    #[pyo3(signature = (value, confidence, was_coerced, flags))]
    fn new(
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        confidence: f64,
        was_coerced: bool,
        flags: Vec<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            value: value.into_py(py),
            confidence,
            was_coerced,
            flags,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "CoercionResult(confidence={:.2}, was_coerced={}, flags={})",
            self.confidence,
            self.was_coerced,
            self.flags.len()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Raw string result of a heal_json call (for backwards-compat tests).
#[pyclass]
pub struct HealedJsonResult {
    #[pyo3(get)]
    pub content: String,
    #[pyo3(get)]
    pub confidence: f64,
    #[pyo3(get)]
    pub was_healed: bool,
    #[pyo3(get)]
    pub flags: Vec<String>,
    #[pyo3(get)]
    pub raw_response: String,
    #[pyo3(get)]
    pub(crate) usage: PyObject,
}

#[pymethods]
impl HealedJsonResult {
    #[new]
    #[pyo3(signature = (content, confidence, was_healed, flags, *, raw_response=None, usage=None))]
    fn new(
        py: Python<'_>,
        content: String,
        confidence: f64,
        was_healed: bool,
        flags: Vec<String>,
        raw_response: Option<String>,
        usage: Option<PyObject>,
    ) -> PyResult<Self> {
        let usage = usage.unwrap_or_else(|| PyDict::new_bound(py).into_py(py));
        Ok(Self {
            content,
            confidence,
            was_healed,
            flags,
            raw_response: raw_response.unwrap_or_default(),
            usage,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "HealedJsonResult(confidence={:.2}, was_healed={}, flags={})",
            self.confidence,
            self.was_healed,
            self.flags.len()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Placeholder class for structured streaming events (class must exist for tests).
#[pyclass]
pub struct PyStructuredEvent {
    #[pyo3(get)]
    pub is_partial: bool,
    #[pyo3(get)]
    pub is_complete: bool,
    #[pyo3(get)]
    pub value: PyObject,
    #[pyo3(get)]
    pub partial_value: PyObject,
    #[pyo3(get)]
    pub confidence: f64,
    #[pyo3(get)]
    pub was_healed: bool,
    #[pyo3(get)]
    pub coerced_value: PyObject,
    #[pyo3(get)]
    pub coerced_confidence: Option<f64>,
    #[pyo3(get)]
    pub coercion_flags: Vec<String>,
}

#[pymethods]
impl PyStructuredEvent {
    fn __repr__(&self) -> String {
        format!(
            "PyStructuredEvent(is_partial={}, confidence={:.2})",
            self.is_partial, self.confidence
        )
    }
}

// ---------------------------------------------------------------------------
// StreamingParser
// ---------------------------------------------------------------------------

/// Incremental JSON healing parser for streaming LLM output.
#[pyclass]
pub struct PyStreamingParser {
    buffer: String,
    finalized: bool,
}

#[pymethods]
impl PyStreamingParser {
    #[new]
    fn new() -> Self {
        Self {
            buffer: String::new(),
            finalized: false,
        }
    }

    /// Feed a chunk of text into the parser.
    fn feed(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
    }

    /// Attempt to parse the current buffer; returns None if the buffer is empty.
    fn try_parse(&self, py: Python<'_>) -> PyResult<Option<ParseResult>> {
        if self.buffer.trim().is_empty() {
            return Ok(None);
        }
        match JsonishParser::new().parse(&self.buffer) {
            Ok(result) => {
                let was_healed = !result.flags.is_empty();
                let flags: Vec<CoercionFlagStr> = result
                    .flags
                    .iter()
                    .map(|f: &simple_agent_type::coercion::CoercionFlag| f.description())
                    .collect();
                let py_value = serde_json_to_py(py, &result.value)?;
                Ok(Some(ParseResult {
                    value: py_value,
                    confidence: result.confidence as f64,
                    was_healed,
                    flags,
                }))
            }
            Err(_) => Ok(None),
        }
    }

    /// Finalize the parser and return the healed result.
    fn finalize(&mut self, py: Python<'_>) -> PyResult<ParseResult> {
        if self.finalized {
            return Err(PyRuntimeError::new_err("Parser already finalized"));
        }
        if self.buffer.trim().is_empty() {
            return Err(PyRuntimeError::new_err("Parsing failed: buffer is empty"));
        }
        let result = JsonishParser::new()
            .parse(&self.buffer)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.finalized = true;
        let was_healed = !result.flags.is_empty();
        let flags: Vec<CoercionFlagStr> = result
            .flags
            .iter()
            .map(|f: &simple_agent_type::coercion::CoercionFlag| f.description())
            .collect();
        let py_value = serde_json_to_py(py, &result.value)?;
        Ok(ParseResult {
            value: py_value,
            confidence: result.confidence as f64,
            was_healed,
            flags,
        })
    }

    fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn clear(&mut self) {
        self.buffer.clear();
        self.finalized = false;
    }

    fn __repr__(&self) -> String {
        format!(
            "StreamingParser(buffer_len={}, finalized={})",
            self.buffer.len(),
            if self.finalized { "True" } else { "False" }
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

type CoercionFlagStr = String;

// ---------------------------------------------------------------------------
// Module-level healing functions
// ---------------------------------------------------------------------------

/// Parse and heal a raw JSON string. Returns a ParseResult.
#[pyfunction]
fn heal_json(py: Python<'_>, raw: &str) -> PyResult<ParseResult> {
    let parser = JsonishParser::new();
    let result = parser.parse(raw).map_err(py_err)?;
    let was_healed = !result.flags.is_empty();
    let flags: Vec<String> = result.flags.iter().map(|f| f.description()).collect();
    let py_value = serde_json_to_py(py, &result.value)?;
    Ok(ParseResult {
        value: py_value,
        confidence: result.confidence as f64,
        was_healed,
        flags,
    })
}

/// Coerce a Python dict/list value to match a JSON Schema. Returns a CoercionResult.
#[pyfunction]
fn coerce_to_schema(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
) -> PyResult<PyCoercionResult> {
    let value_json: serde_json::Value =
        pythonize::depythonize(value).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let schema_json: serde_json::Value =
        pythonize::depythonize(schema).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let schema = schema_converter::convert(&schema_json)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let engine = CoercionEngine::new();
    let result = engine
        .coerce(&value_json, &schema)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let was_coerced = !result.flags.is_empty();
    let flags: Vec<String> = result.flags.iter().map(|f| f.description()).collect();
    let py_value = serde_json_to_py(py, &result.value)?;
    Ok(PyCoercionResult {
        value: py_value,
        confidence: result.confidence as f64,
        was_coerced,
        flags,
    })
}

fn serde_json_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    Ok(pythonize::pythonize(py, value)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        .into_py(py))
}

#[pymodule]
fn simple_agents_py(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    // Typed message classes
    module.add_class::<Role>()?;
    module.add_class::<ContentPart>()?;
    module.add_class::<PyMessage>()?;
    // Expose PyMessage as "Message" for the clean API
    module.add("Message", module.getattr("PyMessage")?)?;
    module.add_class::<Client>()?;
    module.add_class::<ClientBuilder>()?;
    module.add_class::<ProviderConfig>()?;
    module.add_class::<StreamChunk>()?;
    module.add_class::<PyStreamIterator>()?;
    module.add_class::<PyStructuredStreamIterator>()?;
    module.add(
        "StructuredStreamIterator",
        module.getattr("PyStructuredStreamIterator")?,
    )?;
    module.add_class::<ResponseWithMetadata>()?;
    // Healing types
    module.add_class::<ParseResult>()?;
    module.add_class::<PyCoercionResult>()?;
    module.add_class::<HealedJsonResult>()?;
    module.add_class::<PyStructuredEvent>()?;
    module.add_class::<PyStreamingParser>()?;
    // Healing functions
    module.add_function(wrap_pyfunction!(heal_json, module)?)?;
    module.add_function(wrap_pyfunction!(coerce_to_schema, module)?)?;
    // Expose PyCoercionResult under the name "CoercionResult" for test compatibility
    module.add("CoercionResult", module.getattr("PyCoercionResult")?)?;
    // Expose PyStreamingParser under the name "StreamingParser" for test compatibility
    module.add("StreamingParser", module.getattr("PyStreamingParser")?)?;
    Ok(())
}
