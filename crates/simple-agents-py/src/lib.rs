//! Python bindings for SimpleAgents using PyO3.

#![allow(clippy::useless_conversion)]

use futures_util::{Stream, StreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{CompletionChunk, Result};
use simple_agents_core::{CompletionOptions, SimpleAgentsClient};
use simple_agents_healing::{CoercionEngine, JsonishParser};
use simple_agents_providers::schema_converter;
use simple_agents_workflow::yaml_runner::workflow_execution;
use simple_agents_workflow::yaml_runner::{
    YamlWorkflowExecutionRequest, YamlWorkflowExecutorBinding, YamlWorkflowSource,
};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod completion_helpers;
mod provider_helpers;
mod workflow_helpers;

use completion_helpers::{
    build_request_with_messages, expect_response, expect_stream, finish_reason_to_str,
    parse_messages, parse_tool_choice, parse_tools, py_err, response_with_metadata_from_response,
};
use provider_helpers::build_provider_from_name;
use workflow_helpers::{
    attach_workflow_events, build_workflow_input_from_execution_request,
    parse_workflow_execution_request, workflow_execution_flags, workflow_execution_options,
    workflow_root_path, CombinedWorkflowEventSink, PythonCustomWorkerExecutor,
    PythonWorkflowEventSink, RecordingWorkflowEventSink,
};

type Runtime = tokio::runtime::Runtime;

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
    /// Parsed (value, confidence, was_healed) — populated after stream exhausted.
    result: Option<(serde_json::Value, f64, bool)>,
    yielded: bool,
}

#[pymethods]
impl PyStructuredStreamIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<PyObject>> {
        if slf.yielded {
            return Ok(None);
        }

        if slf.stream.is_some() {
            let runtime = Arc::clone(&slf.runtime);
            let runtime_lock = runtime
                .lock()
                .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
            loop {
                let stream = slf.stream.as_mut().unwrap();
                let next =
                    py.allow_threads(|| Ok::<_, PyErr>(runtime_lock.block_on(stream.next())))?;
                match next {
                    Some(Ok(chunk)) => {
                        if let Some(c) = chunk.choices.first() {
                            if let Some(content) = &c.delta.content {
                                slf.buffer.push_str(content);
                            }
                        }
                    }
                    Some(Err(e)) => return Err(PyRuntimeError::new_err(e.to_string())),
                    None => break,
                }
            }
            drop(runtime_lock);
            slf.stream = None;

            let result = if slf.buffer.trim().is_empty() {
                (serde_json::Value::Null, 0.0_f64, false)
            } else {
                match JsonishParser::new().parse(&slf.buffer) {
                    Ok(r) => {
                        let healed = !r.flags.is_empty();
                        (r.value, r.confidence as f64, healed)
                    }
                    Err(_) => (serde_json::Value::String(slf.buffer.clone()), 0.0, false),
                }
            };
            slf.result = Some(result);
        }

        slf.yielded = true;
        let (json_val, confidence, was_healed) = slf
            .result
            .take()
            .unwrap_or((serde_json::Value::Null, 0.0, false));
        let py_value = serde_json_to_py(py, &json_val)?;
        let event = PyStructuredEvent {
            is_partial: false,
            is_complete: true,
            value: py_value.clone_ref(py),
            partial_value: py_value,
            confidence,
            was_healed,
        };
        Ok(Some(Py::new(py, event)?.into_py(py)))
    }

    fn __repr__(&self) -> String {
        format!(
            "PyStructuredStreamIterator(active={}, yielded={})",
            self.stream.is_some(),
            self.yielded
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
struct Client {
    runtime: Arc<Mutex<Runtime>>,
    client: SimpleAgentsClient,
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
    #[pyo3(signature = (provider, *, api_key=None, api_base=None, base_url=None, model=None, api_format=None, timeout_seconds=None))]
    fn new(
        provider: &str,
        api_key: Option<&str>,
        api_base: Option<&str>,
        base_url: Option<&str>,
        model: Option<&str>,
        api_format: Option<&str>,
        timeout_seconds: Option<f64>,
    ) -> PyResult<Self> {
        let _ = (model, timeout_seconds);
        let effective_base = api_base.or(base_url);
        let prov = build_provider_from_name(provider, api_key, effective_base, api_format)
            .map_err(py_err)?;
        let client = SimpleAgentsClient::new(prov);
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            client,
        })
    }

    /// Send a completion request.
    #[pyo3(signature = (model, input, max_tokens=None, temperature=None, top_p=None, tools=None, tool_choice=None, response_format=None, heal=None, stream=None, schema=None))]
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
    ) -> PyResult<PyObject> {
        if heal.unwrap_or(false) && stream.unwrap_or(false) {
            return Err(PyRuntimeError::new_err(
                "heal is not supported with stream=True",
            ));
        }
        let messages = if let Ok(prompt) = input.extract::<&str>() {
            if prompt.is_empty() {
                return Err(PyRuntimeError::new_err("prompt cannot be empty"));
            }
            vec![Message::user(prompt)]
        } else {
            parse_messages(input).map_err(py_err)?
        };

        if stream.unwrap_or(false) {
            if let Some(schema_ref) = schema {
                if schema_ref.downcast::<pyo3::types::PyDict>().is_err() {
                    return Err(PyRuntimeError::new_err(
                        "schema must be a dict/mapping object",
                    ));
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
                    result: None,
                    yielded: false,
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

    /// Stream a YAML workflow using separate positional path + input arguments.
    ///
    /// Signature mirrors the Python test expectation:
    ///   `client.run_workflow_yaml_stream(path, input_dict, *, on_event=..., workflow_options=...)`
    #[pyo3(signature = (workflow_path, input, *, on_event=None, workflow_options=None))]
    fn run_workflow_yaml_stream(
        &self,
        py: Python<'_>,
        workflow_path: &str,
        input: &Bound<'_, PyAny>,
        on_event: Option<Py<PyAny>>,
        workflow_options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let mut request_map: serde_json::Map<String, serde_json::Value> =
            pythonize::depythonize::<serde_json::Value>(input)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
        request_map.insert(
            "workflow_path".to_string(),
            serde_json::Value::String(workflow_path.to_string()),
        );
        if let Some(wo) = workflow_options {
            let wo_json: serde_json::Value = pythonize::depythonize(wo)
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            request_map.insert("workflow_options".to_string(), wo_json);
        }
        let request_value = serde_json::Value::Object(request_map);
        let py_request = pythonize::pythonize(py, &request_value)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        self.stream_workflow(py, &py_request, on_event, false)
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
#[derive(Clone)]
pub struct HealedJsonResult {
    #[pyo3(get)]
    pub content: String,
    #[pyo3(get)]
    pub confidence: f64,
    #[pyo3(get)]
    pub was_healed: bool,
    #[pyo3(get)]
    pub flags: Vec<String>,
}

#[pymethods]
impl HealedJsonResult {
    #[new]
    #[pyo3(signature = (content, confidence, was_healed, flags))]
    fn new(
        content: String,
        confidence: f64,
        was_healed: bool,
        flags: Vec<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            content,
            confidence,
            was_healed,
            flags,
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
            return Err(PyRuntimeError::new_err(
                "Parsing failed: buffer is empty",
            ));
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
    let schema =
        schema_converter::convert(&schema_json).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
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
    module.add_class::<Client>()?;
    module.add_class::<StreamChunk>()?;
    module.add_class::<PyStreamIterator>()?;
    module.add_class::<PyStructuredStreamIterator>()?;
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
