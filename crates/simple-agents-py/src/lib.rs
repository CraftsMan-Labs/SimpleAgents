//! Python bindings for SimpleAgents using PyO3.

#![allow(clippy::useless_conversion)]

use futures_util::{Stream, StreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{CompletionChunk, Result};
use simple_agents_core::{CompletionOptions, SimpleAgentsClient};
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
use provider_helpers::build_provider;
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
    /// # Arguments
    /// * `api_key` - API key for the provider
    /// * `model` - Default model name (reserved for future use)
    /// * `base_url` - Optional custom base URL
    /// * `api_format` - Optional API format: "chat_completions" (default) or "responses"
    #[new]
    #[pyo3(signature = (api_key, model=None, base_url=None, api_format=None))]
    fn new(
        api_key: &str,
        model: Option<&str>,
        base_url: Option<&str>,
        api_format: Option<&str>,
    ) -> PyResult<Self> {
        let _ = model;
        let provider = build_provider(api_key, base_url, api_format).map_err(py_err)?;
        let client = SimpleAgentsClient::new(provider);
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            client,
        })
    }

    /// Send a completion request.
    #[pyo3(signature = (model, input, max_tokens=None, temperature=None, top_p=None, tools=None, tool_choice=None, response_format=None))]
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
    ) -> PyResult<PyObject> {
        let messages = if let Ok(prompt) = input.extract::<&str>() {
            if prompt.is_empty() {
                return Err(PyRuntimeError::new_err("prompt cannot be empty"));
            }
            vec![Message::user(prompt)]
        } else {
            parse_messages(input).map_err(py_err)?
        };

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
            model, messages, max_tokens, temperature, top_p, resp_format, tools, tool_choice, None,
        )
        .map_err(py_err)?;

        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let start = Instant::now();

        let outcome = py
            .allow_threads(|| runtime.block_on(self.client.complete(&request, CompletionOptions::default())))
            .map_err(py_err)?;

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
            model, messages, max_tokens, temperature, top_p, None, None, None, Some(true),
        )
        .map_err(py_err)?;

        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;

        let outcome = py
            .allow_threads(|| runtime.block_on(self.client.complete(&request, CompletionOptions::default())))
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
}

#[pymodule]
fn simple_agents_py(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Client>()?;
    module.add_class::<StreamChunk>()?;
    module.add_class::<PyStreamIterator>()?;
    module.add_class::<ResponseWithMetadata>()?;
    Ok(())
}
