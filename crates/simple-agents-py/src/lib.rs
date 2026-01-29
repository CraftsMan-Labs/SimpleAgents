//! Python bindings for SimpleAgents using PyO3.

#![allow(clippy::useless_conversion)]

use futures_util::{Stream, StreamExt};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use reqwest::Client as HttpClient;
use serde_json::Value;
use simple_agent_type::cache::Cache;
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{
    ApiKey, CompletionChunk, CompletionRequest, Provider, Result, SimpleAgentsError,
};
use simple_agent_type::request::{JsonSchemaFormat, ResponseFormat};
use simple_agent_type::response::Usage;
use simple_agent_type::tool::{ToolCall, ToolChoice, ToolDefinition};
use simple_agents_core::{
    HealedJsonResponse, HealingSettings, Middleware, SimpleAgentsClient, SimpleAgentsClientBuilder,
};
use simple_agents_healing::coercion::CoercionConfig;
use simple_agents_healing::parser::ParserConfig;
use simple_agents_healing::schema::{Field as HealingField, ObjectSchema, StreamAnnotation};
use simple_agents_healing::streaming::StreamingParser as RustStreamingParser;
use simple_agents_healing::{CoercionEngine, JsonishParser, Schema};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use simple_agents_providers::healing_integration::{HealingConfig, HealingIntegration};
use simple_agents_providers::streaming_structured::{StructuredEvent, StructuredStream};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Runtime = tokio::runtime::Runtime;

/// Result from parsing JSON-ish text with healing metadata.
#[pyclass]
pub struct ParseResult {
    #[pyo3(get)]
    value: PyObject,
    #[pyo3(get)]
    confidence: f32,
    #[pyo3(get)]
    was_healed: bool,
    flags: Vec<String>,
}

#[pymethods]
impl ParseResult {
    #[new]
    #[pyo3(signature = (value, confidence, was_healed, flags))]
    fn new(value: PyObject, confidence: f32, was_healed: bool, flags: Vec<String>) -> Self {
        Self {
            value,
            confidence,
            was_healed,
            flags,
        }
    }

    #[getter]
    fn flags(&self) -> Vec<String> {
        self.flags.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "ParseResult(confidence={:.2}, flags={})",
            self.confidence,
            self.flags.len()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Result from coercing data to match a schema with healing metadata.
#[pyclass]
pub struct CoercionResult {
    #[pyo3(get)]
    value: PyObject,
    #[pyo3(get)]
    confidence: f32,
    #[pyo3(get)]
    was_coerced: bool,
    flags: Vec<String>,
}

/// Internal schema wrapper for Python usage.
#[pyclass]
pub struct PySchema {
    schema: Schema,
}

#[pymethods]
impl PySchema {
    fn __repr__(&self) -> String {
        "Schema()".to_string()
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Builder for healing schemas with aliases and defaults.
#[pyclass]
pub struct SchemaBuilder {
    fields: Vec<HealingField>,
    allow_additional_fields: bool,
}

#[pymethods]
impl SchemaBuilder {
    #[new]
    fn new() -> Self {
        Self {
            fields: Vec::new(),
            allow_additional_fields: true,
        }
    }

    /// Allow or deny additional fields not defined in the schema.
    fn allow_additional_fields(&mut self, allow: bool) {
        self.allow_additional_fields = allow;
    }

    /// Add a field to the schema.
    ///
    /// # Arguments
    ///
    /// * `name` - Field name
    /// * `type` - Field type string (e.g., "string", "integer", "number", "boolean", "array")
    /// * `required` - Whether the field is required
    /// * `aliases` - Optional list of aliases
    /// * `default` - Optional default value
    /// * `description` - Optional description
    /// * `stream` - Optional stream annotation: "normal", "not_null", "done"
    /// * `items` - For arrays, the item type (string or Schema)
    #[pyo3(signature = (name, field_type, required=true, aliases=None, default=None, description=None, stream=None, items=None))]
    fn field(
        &mut self,
        py: Python<'_>,
        name: &str,
        field_type: &Bound<'_, PyAny>,
        required: bool,
        aliases: Option<&Bound<'_, PyAny>>,
        default: Option<&Bound<'_, PyAny>>,
        description: Option<String>,
        stream: Option<String>,
        items: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let schema = parse_schema_from_py(py, field_type, items)?;
        let aliases_vec = if let Some(alias_obj) = aliases {
            let values: Vec<String> =
                pythonize::depythonize(alias_obj).map_err(|_| {
                    PyRuntimeError::new_err("aliases must be a list of strings".to_string())
                })?;
            values
        } else {
            Vec::new()
        };

        let default_value = if let Some(default_obj) = default {
            let value: serde_json::Value = pythonize::depythonize(default_obj).map_err(|_| {
                PyRuntimeError::new_err("default must be JSON-serializable".to_string())
            })?;
            Some(value)
        } else {
            None
        };

        let stream_annotation = match stream.as_deref() {
            None | Some("normal") => StreamAnnotation::Normal,
            Some("not_null") => StreamAnnotation::NotNull,
            Some("done") => StreamAnnotation::Done,
            Some(other) => {
                return Err(PyRuntimeError::new_err(format!(
                    "Unknown stream annotation: {}",
                    other
                )))
            }
        };

        self.fields.push(HealingField {
            name: name.to_string(),
            schema,
            required,
            aliases: aliases_vec,
            default: default_value,
            description,
            stream_annotation,
        });

        Ok(())
    }

    /// Build the schema.
    fn build(&self) -> PySchema {
        PySchema {
            schema: Schema::Object(ObjectSchema {
                fields: self.fields.clone(),
                allow_additional_fields: self.allow_additional_fields,
            }),
        }
    }
}

#[pymethods]
impl CoercionResult {
    #[new]
    #[pyo3(signature = (value, confidence, was_coerced, flags))]
    fn new(value: PyObject, confidence: f32, was_coerced: bool, flags: Vec<String>) -> Self {
        Self {
            value,
            confidence,
            was_coerced,
            flags,
        }
    }

    #[getter]
    fn flags(&self) -> Vec<String> {
        self.flags.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "CoercionResult(confidence={:.2}, flags={})",
            self.confidence,
            self.flags.len()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Streaming JSON parser for incremental parsing.
///
/// Accumulates chunks and extracts complete JSON values as they become available.
///
/// # Example
///
/// ```python
/// from simple_agents_py import StreamingParser
///
/// parser = StreamingParser()
/// parser.feed('{"name": "Alice", ')
/// parser.feed('"age": 30}')
///
/// result = parser.finalize()
/// print(result.value)  # {"name": "Alice", "age": 30}
/// ```
#[pyclass]
pub struct StreamingParser {
    parser: Option<RustStreamingParser>,
}

#[pymethods]
impl StreamingParser {
    /// Create a new streaming parser with default configuration.
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let parser = if let Some(_cfg) = config {
            RustStreamingParser::with_config(ParserConfig::default())
        } else {
            RustStreamingParser::new()
        };
        Ok(Self {
            parser: Some(parser),
        })
    }

    /// Feed a chunk of JSON data to the parser.
    ///
    /// For single objects, this doesn't return values until finalize() is called.
    /// For arrays, this can return completed array elements in future implementations.
    ///
    /// # Arguments
    ///
    /// * `chunk` - A string chunk of JSON data
    fn feed(&mut self, chunk: &str) -> PyResult<()> {
        if let Some(parser) = &mut self.parser {
            parser.feed(chunk);
        }
        Ok(())
    }

    /// Finalize the stream and get the complete parsed value.
    ///
    /// This attempts to parse the entire accumulated buffer as a single JSON value.
    /// Call this when the stream is complete. After calling finalize, the parser
    /// cannot be used again.
    ///
    /// # Returns
    ///
    /// A ParseResult containing the parsed value, confidence, and healing metadata.
    ///
    /// # Errors
    ///
    /// Raises RuntimeError if the accumulated buffer cannot be parsed as valid JSON.
    /// Raises RuntimeError if finalize is called more than once.
    fn finalize(&mut self, py: Python<'_>) -> PyResult<ParseResult> {
        let parser = self
            .parser
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("Parser already finalized"))?;

        let result = parser
            .finalize()
            .map_err(|e| PyRuntimeError::new_err(format!("Parsing failed: {}", e)))?;

        let py_value = pythonize::pythonize(py, &result.value)
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion failed: {}", e)))?;

        let was_healed = !result.flags.is_empty();
        let flags: Vec<String> = result.flags.iter().map(|f| f.description()).collect();

        Ok(ParseResult {
            value: py_value.into(),
            confidence: result.confidence,
            was_healed,
            flags,
        })
    }

    /// Get the current buffer size in bytes.
    fn buffer_len(&self) -> usize {
        self.parser.as_ref().map(|p| p.buffer_len()).unwrap_or(0)
    }

    /// Check if the buffer is empty.
    fn is_empty(&self) -> bool {
        self.parser.as_ref().map(|p| p.is_empty()).unwrap_or(true)
    }

    /// Clear the parser state and buffer.
    fn clear(&mut self) {
        if let Some(parser) = &mut self.parser {
            parser.clear();
        }
    }

    fn __repr__(&self) -> String {
        let finalized = if self.parser.is_none() { "True" } else { "False" };
        format!(
            "StreamingParser(buffer_len={}, finalized={})",
            self.buffer_len(),
            finalized
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// A single chunk from a streaming completion.
///
/// # Example
///
/// ```python
/// from simple_agents_py import Client
///
/// client = Client("openai")
/// for chunk in client.stream("gpt-4o-mini", "Hello!"):
///     print(chunk.content, end="", flush=True)
/// ```
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
///
/// Bridges Rust async streams to Python's iterator protocol.
#[pyclass]
pub struct PyStreamIterator {
    stream: Option<Pin<Box<dyn Stream<Item = Result<CompletionChunk>> + Send>>>,
}

#[pymethods]
impl PyStreamIterator {
    #[new]
    #[pyo3(signature = (client, model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn new(
        client: &Client,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<Self> {
        let messages = parse_messages(messages).map_err(py_err)?;
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

        let runtime = client
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;

        let client_ref = &client.client;
        let stream_result = runtime.block_on(client_ref.stream(&request));

        let stream = stream_result.map_err(py_err)?;

        Ok(Self {
            stream: Some(Box::pin(stream)),
        })
    }

    /// Iterator protocol: returns self.
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Iterator protocol: return next chunk or raise StopIteration.
    fn __next__(mut slf: PyRefMut<'_, Self>) -> PyResult<Option<StreamChunk>> {
        let stream = slf
            .stream
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Stream exhausted"))?;

        // Create a new runtime just for this poll
        let rt = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let result = rt.block_on(stream.next());

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
                    .map(|fr| format!("{:?}", fr));

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

/// Result from a JSON completion with healing metadata.
#[pyclass]
pub struct HealedJsonResult {
    #[pyo3(get)]
    content: String,
    #[pyo3(get)]
    confidence: f32,
    #[pyo3(get)]
    was_healed: bool,
    flags: Vec<String>,
}

#[pymethods]
impl HealedJsonResult {
    #[new]
    #[pyo3(signature = (content, confidence, was_healed, flags))]
    fn new(content: String, confidence: f32, was_healed: bool, flags: Vec<String>) -> Self {
        Self {
            content,
            confidence,
            was_healed,
            flags,
        }
    }

    #[getter]
    fn flags(&self) -> Vec<String> {
        self.flags.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "HealedJsonResult(confidence={:.2}, flags={}, content={:?}...)",
            self.confidence,
            self.flags.len(),
            &self.content.chars().take(50).collect::<String>()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Event emitted during structured streaming.
///
/// Can represent either a partial update (progressive parsing) or
/// final complete value with healing metadata.
#[pyclass]
pub struct PyStructuredEvent {
    #[pyo3(get)]
    is_partial: bool,
    #[pyo3(get)]
    is_complete: bool,
    #[pyo3(get)]
    value: PyObject,
    #[pyo3(get)]
    partial_value: PyObject,
    #[pyo3(get)]
    confidence: f32,
    #[pyo3(get)]
    was_healed: bool,
}

#[pymethods]
impl PyStructuredEvent {
    fn __repr__(&self) -> String {
        if self.is_partial {
            format!(
                "PyStructuredEvent(partial, confidence={:.2})",
                self.confidence
            )
        } else {
            format!(
                "PyStructuredEvent(complete, confidence={:.2}, healed={})",
                self.confidence, self.was_healed
            )
        }
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Completion response with metadata for debugging and observability.
#[pyclass]
pub struct ResponseWithMetadata {
    #[pyo3(get)]
    content: String,
    #[pyo3(get)]
    provider: Option<String>,
    #[pyo3(get)]
    model: String,
    #[pyo3(get)]
    finish_reason: String,
    #[pyo3(get)]
    created: Option<i64>,
    #[pyo3(get)]
    latency_ms: u64,
    #[pyo3(get)]
    was_healed: bool,
    #[pyo3(get)]
    healing_confidence: Option<f32>,
    #[pyo3(get)]
    healing_error: Option<String>,
    #[pyo3(get)]
    tool_calls: PyObject,
    flags: Vec<String>,
    usage: PyObject,
}

#[pymethods]
impl ResponseWithMetadata {
    #[getter]
    fn usage(&self, py: Python<'_>) -> PyObject {
        self.usage.clone_ref(py).into()
    }

    #[getter]
    fn flags(&self) -> Vec<String> {
        self.flags.clone()
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

/// Streaming iterator for structured output events.
///
/// Bridges Rust's StructuredStream to Python's iterator protocol.
#[pyclass]
pub struct StructuredStreamIterator {
    stream: Option<Pin<Box<dyn Stream<Item = Result<StructuredEvent<Value>>> + Send>>>,
}

#[pymethods]
impl StructuredStreamIterator {
    /// Iterator protocol: returns self.
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Iterator protocol: return next event or raise StopIteration.
    fn __next__(
        mut slf: PyRefMut<'_, Self>,
        py: Python<'_>,
    ) -> PyResult<Option<PyStructuredEvent>> {
        let stream = slf
            .stream
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Stream exhausted"))?;

        // Create a new runtime just for this poll
        let rt = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let result = rt.block_on(stream.next());

        match result {
            Some(Ok(rust_event)) => {
                // Convert Rust event to Python event
                let (is_partial, is_complete, value, partial_value, confidence, was_healed) =
                    match rust_event {
                        StructuredEvent::Partial(v) => {
                            let py_value = pythonize::pythonize(py, &v).map_err(|e| {
                                PyRuntimeError::new_err(format!("Conversion failed: {}", e))
                            })?;
                            let obj: PyObject = py_value.into();
                            (true, false, obj.clone_ref(py).into(), obj, 0.0, false)
                        }
                        StructuredEvent::Complete {
                            value,
                            confidence,
                            was_healed,
                        } => {
                            let py_value = pythonize::pythonize(py, &value).map_err(|e| {
                                PyRuntimeError::new_err(format!("Conversion failed: {}", e))
                            })?;
                            let obj: PyObject = py_value.into();
                            let none: PyObject = py.None().into();
                            (false, true, obj, none, confidence, was_healed)
                        }
                    };

                Ok(Some(PyStructuredEvent {
                    is_partial,
                    is_complete,
                    value,
                    partial_value,
                    confidence,
                    was_healed,
                }))
            }
            Some(Err(e)) => Err(PyRuntimeError::new_err(format!("{}", e))),
            None => {
                slf.stream = None;
                Ok(None)
            }
        }
    }

    fn __repr__(&self) -> String {
        format!("StructuredStreamIterator(active={})", self.stream.is_some())
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

#[pyclass]
struct Client {
    runtime: Mutex<Runtime>,
    client: SimpleAgentsClient,
}

struct PyMiddlewareAdapter {
    middleware: Py<PyAny>,
}

// Safe because all interaction with the Python object happens under the GIL.
unsafe impl Send for PyMiddlewareAdapter {}
unsafe impl Sync for PyMiddlewareAdapter {}

impl PyMiddlewareAdapter {
    fn call_optional_method(
        &self,
        method: &str,
        args: &[PyObject],
    ) -> std::result::Result<(), SimpleAgentsError> {
        Python::with_gil(|py| {
            let obj = self.middleware.bind(py);
            if !obj
                .hasattr(method)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?
            {
                return Ok(());
            }

            let args = PyTuple::new_bound(py, args);
            let result = obj
                .call_method1(method, args)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;

            if result
                .hasattr("__await__")
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?
            {
                return Err(SimpleAgentsError::Config(format!(
                    "Middleware method '{}' returned awaitable; async middleware not supported",
                    method
                )));
            }

            Ok(())
        })
    }
}

#[async_trait::async_trait]
impl Middleware for PyMiddlewareAdapter {
    async fn before_request(&self, request: &CompletionRequest) -> Result<()> {
        let args = Python::with_gil(|py| -> std::result::Result<Vec<PyObject>, SimpleAgentsError> {
            let py_request = pythonize::pythonize(py, request)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            Ok(vec![py_request.into()])
        })?;
        self.call_optional_method("before_request", &args)
    }

    async fn after_response(
        &self,
        request: &CompletionRequest,
        response: &simple_agent_type::response::CompletionResponse,
        latency: Duration,
    ) -> Result<()> {
        let args = Python::with_gil(|py| -> std::result::Result<Vec<PyObject>, SimpleAgentsError> {
            let py_request = pythonize::pythonize(py, request)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            let py_response = pythonize::pythonize(py, response)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            let latency_ms: u64 = latency.as_millis() as u64;
            Ok(vec![py_request.into(), py_response.into(), latency_ms.into_py(py)])
        })?;
        self.call_optional_method("after_response", &args)
    }

    async fn on_cache_hit(
        &self,
        request: &CompletionRequest,
        response: &simple_agent_type::response::CompletionResponse,
    ) -> Result<()> {
        let args = Python::with_gil(|py| -> std::result::Result<Vec<PyObject>, SimpleAgentsError> {
            let py_request = pythonize::pythonize(py, request)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            let py_response = pythonize::pythonize(py, response)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            Ok(vec![py_request.into(), py_response.into()])
        })?;
        self.call_optional_method("on_cache_hit", &args)
    }

    async fn on_error(
        &self,
        request: &CompletionRequest,
        error: &SimpleAgentsError,
        latency: Duration,
    ) -> Result<()> {
        let args = Python::with_gil(|py| -> std::result::Result<Vec<PyObject>, SimpleAgentsError> {
            let py_request = pythonize::pythonize(py, request)
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            let latency_ms: u64 = latency.as_millis() as u64;
            Ok(vec![
                py_request.into(),
                error.to_string().into_py(py),
                latency_ms.into_py(py),
            ])
        })?;
        self.call_optional_method("on_error", &args)
    }

    fn name(&self) -> &str {
        "py_middleware"
    }
}

struct PyCacheAdapter {
    cache: Py<PyAny>,
}

// Safe because all interaction with the Python object happens under the GIL.
unsafe impl Send for PyCacheAdapter {}
unsafe impl Sync for PyCacheAdapter {}

impl PyCacheAdapter {
    fn call_required_method<'py>(
        &self,
        py: Python<'py>,
        method: &str,
        args: &[PyObject],
    ) -> std::result::Result<Bound<'py, PyAny>, SimpleAgentsError> {
        let obj = self.cache.bind(py);
        if !obj
            .hasattr(method)
            .map_err(|e| SimpleAgentsError::Config(e.to_string()))?
        {
            return Err(SimpleAgentsError::Config(format!(
                "Cache object must implement '{}'",
                method
            )));
        }

        let args = PyTuple::new_bound(py, args);
        let result = obj
            .call_method1(method, args)
            .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;

        if result
            .hasattr("__await__")
            .map_err(|e| SimpleAgentsError::Config(e.to_string()))?
        {
            return Err(SimpleAgentsError::Config(format!(
                "Cache method '{}' returned awaitable; async cache not supported",
                method
            )));
        }

        Ok(result)
    }
}

#[async_trait::async_trait]
impl Cache for PyCacheAdapter {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Python::with_gil(|py| {
            let result = self.call_required_method(py, "get", &[key.into_py(py)])?;
            if result.is_none() {
                return Ok(None);
            }
            let bytes: Vec<u8> = result
                .extract()
                .map_err(|e| SimpleAgentsError::Config(e.to_string()))?;
            Ok(Some(bytes))
        })
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()> {
        Python::with_gil(|py| {
            let py_bytes = PyBytes::new_bound(py, &value).into_py(py);
            let ttl_secs = ttl.as_secs();
            let _ = self.call_required_method(
                py,
                "set",
                &[key.into_py(py), py_bytes, ttl_secs.into_py(py)],
            )?;
            Ok(())
        })
    }

    async fn delete(&self, key: &str) -> Result<()> {
        Python::with_gil(|py| {
            let _ = self.call_required_method(py, "delete", &[key.into_py(py)])?;
            Ok(())
        })
    }

    async fn clear(&self) -> Result<()> {
        Python::with_gil(|py| {
            let _ = self.call_required_method(py, "clear", &[])?;
            Ok(())
        })
    }
}

fn provider_from_params(
    provider_name: &str,
    api_key: Option<&str>,
    api_base: Option<&str>,
) -> Result<Arc<dyn Provider>> {
    let api_key = match api_key {
        Some(value) => Some(ApiKey::new(value)?),
        None => None,
    };

    match provider_name {
        "openai" => {
            let provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        if is_local_base(api_base) {
                            let client = HttpClient::builder()
                                .timeout(Duration::from_secs(30))
                                .pool_max_idle_per_host(10)
                                .pool_idle_timeout(Duration::from_secs(90))
                                .no_proxy()
                                .build()
                                .map_err(|e| {
                                    SimpleAgentsError::Config(format!(
                                        "Failed to create HTTP client: {}",
                                        e
                                    ))
                                })?;
                            OpenAIProvider::with_client(api_key, api_base.to_string(), client)?
                        } else {
                            OpenAIProvider::with_base_url(api_key, api_base.to_string())?
                        }
                    }
                    None => OpenAIProvider::new(api_key)?,
                },
                None => OpenAIProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        "anthropic" => {
            let provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        AnthropicProvider::with_base_url(api_key, api_base.to_string())?
                    }
                    None => AnthropicProvider::new(api_key)?,
                },
                None => AnthropicProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        "openrouter" => {
            let provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        OpenRouterProvider::with_base_url(api_key, api_base.to_string())?
                    }
                    None => OpenRouterProvider::new(api_key)?,
                },
                None => OpenRouterProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        _ => Err(SimpleAgentsError::Config(format!(
            "Unknown provider '{provider_name}'"
        ))),
    }
}

fn is_local_base(api_base: &str) -> bool {
    api_base.contains("localhost") || api_base.contains("127.0.0.1")
}

/// Builder for creating SimpleAgents clients with advanced configuration.
///
/// Allows adding multiple providers, configuring routing, caching, and healing.
///
/// # Example
///
/// ```python
/// from simple_agents_py import ClientBuilder
///
/// client = (
///     ClientBuilder()
///     .add_provider("openai", api_key="sk-...")
///     .add_provider("anthropic", api_key="sk-ant-...")
///     .with_routing("round_robin")
///     .with_cache(ttl_seconds=300)
///     .build()
/// )
/// ```
#[pyclass]
pub struct ClientBuilder {
    providers: Vec<Arc<dyn Provider>>,
    routing_mode: Option<String>,
    routing_config: Option<RoutingConfig>,
    cache_ttl: Option<u64>,
    healing_config: Option<HealingSettings>,
    middleware: Vec<Arc<dyn Middleware>>,
    custom_cache: Option<Arc<dyn Cache>>,
}

#[derive(Debug, Clone)]
enum RoutingConfig {
    Direct,
    RoundRobin,
    Latency {
        alpha: f64,
        slow_threshold_ms: u64,
    },
    Cost {
        costs: Vec<(String, f64)>, // (provider_name, cost_per_1k_tokens)
    },
    Fallback {
        retryable_only: bool,
    },
}

#[pymethods]
impl ClientBuilder {
    /// Create a new client builder.
    #[new]
    fn new() -> Self {
        Self {
            providers: Vec::new(),
            routing_mode: None,
            routing_config: None,
            cache_ttl: None,
            healing_config: None,
            middleware: Vec::new(),
            custom_cache: None,
        }
    }

    /// Add a provider to the builder.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider name: "openai", "anthropic", or "openrouter"
    /// * `api_key` - Optional API key (uses env var if not provided)
    /// * `api_base` - Optional custom API base URL
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    #[pyo3(signature = (provider, api_key=None, api_base=None))]
    fn add_provider<'a>(
        mut slf: PyRefMut<'a, Self>,
        provider: &str,
        api_key: Option<String>,
        api_base: Option<String>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let provider = provider_from_params(provider, api_key.as_deref(), api_base.as_deref())
            .map_err(py_err)?;

        slf.providers.push(provider);
        Ok(slf)
    }

    /// Configure routing mode (simple version with defaults).
    ///
    /// # Arguments
    ///
    /// * `mode` - Routing mode: "direct", "round_robin", "latency", "cost", "fallback"
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    fn with_routing<'a>(mut slf: PyRefMut<'a, Self>, mode: &str) -> PyResult<PyRefMut<'a, Self>> {
        let valid_modes = ["direct", "round_robin", "latency", "cost", "fallback"];
        if !valid_modes.contains(&mode) {
            return Err(PyRuntimeError::new_err(format!(
                "Unknown routing mode: {}. Must be one of: {}",
                mode,
                valid_modes.join(", ")
            )));
        }
        slf.routing_mode = Some(mode.to_string());
        // Set default config for the mode
        slf.routing_config = Some(match mode {
            "direct" => RoutingConfig::Direct,
            "round_robin" => RoutingConfig::RoundRobin,
            "latency" => RoutingConfig::Latency {
                alpha: 0.2,
                slow_threshold_ms: 2000,
            },
            "cost" => RoutingConfig::Cost { costs: Vec::new() },
            "fallback" => RoutingConfig::Fallback {
                retryable_only: true,
            },
            _ => unreachable!(),
        });
        Ok(slf)
    }

    /// Configure latency-based routing with custom settings.
    ///
    /// # Arguments
    ///
    /// * `config` - Dict with latency routing config options:
    ///   - `alpha`: float (default: 0.2) - Exponential moving average factor (0.0-1.0)
    ///   - `slow_threshold_ms`: int (default: 2000) - Threshold in ms for marking providers as degraded
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    #[pyo3(signature = (config))]
    fn with_latency_routing<'a>(
        mut slf: PyRefMut<'a, Self>,
        config: &Bound<'_, PyDict>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let mut alpha = 0.2;
        let mut slow_threshold_ms = 2000u64;

        if let Some(alpha_val) = config.get_item("alpha")? {
            if !alpha_val.is_none() {
                alpha = alpha_val.extract()?;
                if !(0.0..=1.0).contains(&alpha) {
                    return Err(PyRuntimeError::new_err(
                        "alpha must be between 0.0 and 1.0".to_string(),
                    ));
                }
            }
        }

        if let Some(threshold_val) = config.get_item("slow_threshold_ms")? {
            if !threshold_val.is_none() {
                slow_threshold_ms = threshold_val.extract()?;
            }
        }

        slf.routing_mode = Some("latency".to_string());
        slf.routing_config = Some(RoutingConfig::Latency {
            alpha,
            slow_threshold_ms,
        });
        Ok(slf)
    }

    /// Configure cost-based routing with provider costs.
    ///
    /// # Arguments
    ///
    /// * `config` - Dict with cost routing config:
    ///   - `provider_costs`: dict mapping provider names to costs per 1k tokens
    ///     Example: {"openai": 0.002, "anthropic": 0.0003}
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    #[pyo3(signature = (config))]
    fn with_cost_routing<'a>(
        mut slf: PyRefMut<'a, Self>,
        config: &Bound<'_, PyDict>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let costs_dict = config
            .get_item("provider_costs")?
            .ok_or_else(|| PyRuntimeError::new_err("provider_costs is required"))?;

        let costs_dict_ref: &Bound<'_, PyDict> = costs_dict
            .downcast()
            .map_err(|_| PyRuntimeError::new_err("provider_costs must be a dict"))?;

        let mut costs = Vec::new();
        for (provider_name, cost_val) in costs_dict_ref.iter() {
            let name: String = provider_name.extract()?;
            let cost: f64 = cost_val.extract()?;

            if !cost.is_finite() || cost < 0.0 {
                return Err(PyRuntimeError::new_err(format!(
                    "Invalid cost for provider {}: must be non-negative finite number",
                    name
                )));
            }

            costs.push((name, cost));
        }

        slf.routing_mode = Some("cost".to_string());
        slf.routing_config = Some(RoutingConfig::Cost { costs });
        Ok(slf)
    }

    /// Configure fallback routing with custom settings.
    ///
    /// # Arguments
    ///
    /// * `config` - Dict with fallback routing config options:
    ///   - `retryable_only`: bool (default: true) - If true, only fallback on retryable errors
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    #[pyo3(signature = (config))]
    fn with_fallback_routing<'a>(
        mut slf: PyRefMut<'a, Self>,
        config: &Bound<'_, PyDict>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let mut retryable_only = true;

        if let Some(val) = config.get_item("retryable_only")? {
            if !val.is_none() {
                retryable_only = val.extract()?;
            }
        }

        slf.routing_mode = Some("fallback".to_string());
        slf.routing_config = Some(RoutingConfig::Fallback { retryable_only });
        Ok(slf)
    }

    /// Configure response cache with TTL.
    ///
    /// # Arguments
    ///
    /// * `ttl_seconds` - Time to live for cache entries in seconds (0 to disable)
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    fn with_cache<'a>(
        mut slf: PyRefMut<'a, Self>,
        ttl_seconds: u64,
    ) -> PyResult<PyRefMut<'a, Self>> {
        slf.cache_ttl = Some(ttl_seconds);
        Ok(slf)
    }

    /// Configure healing settings.
    ///
    /// # Arguments
    ///
    /// * `config` - Dict with healing config options:
    ///   - `enabled`: bool (default: true)
    ///   - `min_confidence`: float (default: 0.0)
    ///   - `fuzzy_match_threshold`: float (default: 0.8)
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    #[pyo3(signature = (config))]
    fn with_healing_config<'a>(
        mut slf: PyRefMut<'a, Self>,
        config: &Bound<'_, PyDict>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let mut healing = HealingSettings::default();
        let parser_config = ParserConfig::default();
        let mut coercion_config = CoercionConfig::default();

        // Parse enabled flag
        if let Some(enabled) = config.get_item("enabled")? {
            if !enabled.is_none() {
                let val: bool = enabled.extract()?;
                healing.enabled = val;
            }
        }

        // Parse min_confidence
        if let Some(min_conf) = config.get_item("min_confidence")? {
            if !min_conf.is_none() {
                let val: f32 = min_conf.extract()?;
                coercion_config.min_confidence = val;
            }
        }

        // Parse fuzzy_match_threshold
        if let Some(threshold) = config.get_item("fuzzy_match_threshold")? {
            if !threshold.is_none() {
                let val: f64 = threshold.extract()?;
                coercion_config.fuzzy_match_threshold = val;
            }
        }

        healing.parser_config = parser_config;
        healing.coercion_config = coercion_config;
        slf.healing_config = Some(healing);
        Ok(slf)
    }

    /// Add a middleware hook.
    ///
    /// The middleware object can implement any of:
    /// - before_request(request)
    /// - after_response(request, response, latency_ms)
    /// - on_cache_hit(request, response)
    /// - on_error(request, error, latency_ms)
    ///
    /// Methods are optional; only implemented methods are called.
    fn add_middleware<'a>(
        mut slf: PyRefMut<'a, Self>,
        middleware: PyObject,
    ) -> PyResult<PyRefMut<'a, Self>> {
        slf.middleware.push(Arc::new(PyMiddlewareAdapter {
            middleware: middleware.into(),
        }));
        Ok(slf)
    }

    /// Use a custom cache backend provided by Python.
    ///
    /// The cache object must implement:
    /// - get(key) -> Optional[bytes]
    /// - set(key, value: bytes, ttl_seconds: int) -> None
    /// - delete(key) -> None
    /// - clear() -> None
    ///
    /// Async cache methods are not supported.
    #[pyo3(signature = (cache, ttl_seconds=None))]
    fn with_custom_cache<'a>(
        mut slf: PyRefMut<'a, Self>,
        cache: PyObject,
        ttl_seconds: Option<u64>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        slf.custom_cache = Some(Arc::new(PyCacheAdapter { cache: cache.into() }));
        if let Some(ttl) = ttl_seconds {
            slf.cache_ttl = Some(ttl);
        }
        Ok(slf)
    }

    /// Build the client.
    ///
    /// # Errors
    ///
    /// Returns RuntimeError if no providers were added.
    fn build(slf: PyRefMut<'_, Self>) -> PyResult<Client> {
        if slf.providers.is_empty() {
            return Err(PyRuntimeError::new_err("At least one provider is required"));
        }

        let mut builder = SimpleAgentsClientBuilder::new();

        // Add providers
        for provider in slf.providers.iter() {
            builder = builder.with_provider(provider.clone());
        }

        // Set routing mode
        if let Some(config) = &slf.routing_config {
            use simple_agents_core::RoutingMode;
            let routing_mode = match config {
                RoutingConfig::Direct => RoutingMode::Direct,
                RoutingConfig::RoundRobin => RoutingMode::RoundRobin,
                RoutingConfig::Latency {
                    alpha,
                    slow_threshold_ms,
                } => {
                    use simple_agents_router::LatencyRouterConfig;
                    let latency_config = LatencyRouterConfig {
                        alpha: *alpha,
                        slow_threshold: Duration::from_millis(*slow_threshold_ms),
                    };
                    RoutingMode::Latency(latency_config)
                }
                RoutingConfig::Cost { costs } => {
                    use simple_agents_router::{CostRouterConfig, ProviderCost};
                    let mut provider_costs = Vec::new();
                    for (name, cost) in costs.iter() {
                        let pc = ProviderCost::new(name.as_str(), *cost).map_err(|e| {
                            PyRuntimeError::new_err(format!("Invalid cost for {}: {}", name, e))
                        })?;
                        provider_costs.push(pc);
                    }
                    let cost_config = CostRouterConfig::new(provider_costs);
                    RoutingMode::Cost(cost_config)
                }
                RoutingConfig::Fallback { retryable_only } => {
                    use simple_agents_router::FallbackRouterConfig;
                    let fallback_config = FallbackRouterConfig {
                        retryable_only: *retryable_only,
                    };
                    RoutingMode::Fallback(fallback_config)
                }
            };
            builder = builder.with_routing_mode(routing_mode);
        }

        // Set cache
        if let Some(custom_cache) = &slf.custom_cache {
            builder = builder.with_cache(custom_cache.clone());
            if let Some(ttl) = slf.cache_ttl {
                builder = builder.with_cache_ttl(Duration::from_secs(ttl));
            }
        } else if let Some(ttl) = slf.cache_ttl {
            if ttl > 0 {
                use simple_agents_cache::InMemoryCache;
                let cache = Arc::new(InMemoryCache::new(10 * 1024 * 1024, 1000)); // 10MB, 1000 entries
                builder = builder
                    .with_cache(cache)
                    .with_cache_ttl(Duration::from_secs(ttl));
            }
        }

        // Set healing config
        if let Some(healing) = &slf.healing_config {
            builder = builder.with_healing_settings(healing.clone());
        }

        // Add middleware
        for middleware in slf.middleware.iter() {
            builder = builder.with_middleware(middleware.clone());
        }

        let client = builder.build().map_err(py_err)?;
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Client {
            runtime: Mutex::new(runtime),
            client,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "ClientBuilder(providers={}, routing={:?}, cache_ttl={:?}, middleware={})",
            self.providers.len(),
            self.routing_mode,
            self.cache_ttl,
            self.middleware.len()
        )
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

fn build_request(
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> Result<CompletionRequest> {
    if model.is_empty() {
        return Err(SimpleAgentsError::Config(
            "model cannot be empty".to_string(),
        ));
    }
    if prompt.is_empty() {
        return Err(SimpleAgentsError::Config(
            "prompt cannot be empty".to_string(),
        ));
    }

    let mut builder = CompletionRequest::builder()
        .model(model)
        .message(Message::user(prompt));

    if let Some(max_tokens) = max_tokens {
        builder = builder.max_tokens(max_tokens);
    }
    if let Some(temperature) = temperature {
        builder = builder.temperature(temperature);
    }

    builder.build()
}

fn build_request_with_messages(
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

fn parse_messages(messages: &Bound<'_, PyAny>) -> Result<Vec<Message>> {
    let list: &Bound<'_, PyList> = messages
        .downcast()
        .map_err(|_| SimpleAgentsError::Config("messages must be a list of dicts".to_string()))?;
    let mut result = Vec::with_capacity(list.len());

    for (idx, item) in list.iter().enumerate() {
        let dict: &Bound<'_, PyDict> = item
            .downcast()
            .map_err(|_| SimpleAgentsError::Config(format!("message[{idx}] must be a dict")))?;

        let role_obj = dict
            .get_item("role")
            .map_err(|_| SimpleAgentsError::Config(format!("message[{idx}] missing 'role'")))?
            .ok_or_else(|| SimpleAgentsError::Config(format!("message[{idx}] missing 'role'")))?;
        let role: String = role_obj.extract().map_err(|_| {
            SimpleAgentsError::Config(format!("message[{idx}].role must be a string"))
        })?;

        let content_obj = dict
            .get_item("content")
            .map_err(|_| SimpleAgentsError::Config(format!("message[{idx}] missing 'content'")))?
            .ok_or_else(|| {
                SimpleAgentsError::Config(format!("message[{idx}] missing 'content'"))
            })?;
        let content: String = content_obj.extract().map_err(|_| {
            SimpleAgentsError::Config(format!("message[{idx}].content must be a string"))
        })?;

        let mut message = match role.as_str() {
            "user" => Message::user(&content),
            "assistant" => Message::assistant(&content),
            "system" => Message::system(&content),
            "tool" => {
                let tool_call_id = dict
                    .get_item("tool_call_id")
                    .map_err(|_| {
                        SimpleAgentsError::Config(format!(
                            "message[{idx}] missing 'tool_call_id' for tool role"
                        ))
                    })?
                    .ok_or_else(|| {
                        SimpleAgentsError::Config(format!(
                            "message[{idx}] missing 'tool_call_id' for tool role"
                        ))
                    })?
                    .extract::<String>()
                    .map_err(|_| {
                        SimpleAgentsError::Config(format!(
                            "message[{idx}].tool_call_id must be a string"
                        ))
                    })?;
                Message::tool(&content, tool_call_id)
            }
            _ => {
                return Err(SimpleAgentsError::Config(format!(
                    "message[{idx}].role must be one of: user, assistant, system, tool"
                )))
            }
        };

        if let Some(name_obj) = dict.get_item("name").map_err(|_| {
            SimpleAgentsError::Config(format!("message[{idx}].name must be a string"))
        })? {
            if !name_obj.is_none() {
                let name: String = name_obj.extract().map_err(|_| {
                    SimpleAgentsError::Config(format!("message[{idx}].name must be a string"))
                })?;
                message = message.with_name(name);
            }
        }

        if let Some(tool_calls_obj) = dict.get_item("tool_calls").map_err(|_| {
            SimpleAgentsError::Config(format!("message[{idx}].tool_calls must be a list"))
        })? {
            if !tool_calls_obj.is_none() {
                let tool_calls: Vec<ToolCall> = pythonize::depythonize(&tool_calls_obj).map_err(
                    |_| SimpleAgentsError::Config(format!("message[{idx}].tool_calls invalid")),
                )?;
                if !tool_calls.is_empty() {
                    message = message.with_tool_calls(tool_calls);
                }
            }
        }

        result.push(message);
    }

    Ok(result)
}

fn parse_tools(tools: &Bound<'_, PyAny>) -> Result<Vec<ToolDefinition>> {
    pythonize::depythonize(tools).map_err(|_| {
        SimpleAgentsError::Config("tools must be a list of tool definitions".to_string())
    })
}

fn parse_tool_choice(tool_choice: &Bound<'_, PyAny>) -> Result<ToolChoice> {
    pythonize::depythonize(tool_choice).map_err(|_| {
        SimpleAgentsError::Config(
            "tool_choice must be a string (\"auto\"/\"none\") or a tool choice object".to_string(),
        )
    })
}

fn py_err(error: SimpleAgentsError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn parse_schema_from_py(
    py: Python<'_>,
    field_type: &Bound<'_, PyAny>,
    items: Option<&Bound<'_, PyAny>>,
) -> PyResult<Schema> {
    if let Ok(schema_ref) = field_type.extract::<PyRef<PySchema>>() {
        return Ok(schema_ref.schema.clone());
    }

    let type_name: String = field_type.extract().map_err(|_| {
        PyRuntimeError::new_err("field_type must be a string or Schema".to_string())
    })?;

    let schema = match type_name.as_str() {
        "string" => Schema::String,
        "integer" => Schema::Int,
        "number" => Schema::Float,
        "boolean" => Schema::Bool,
        "any" => Schema::Any,
        "array" => {
            let item_schema = if let Some(items_obj) = items {
                parse_schema_from_py(py, items_obj, None)?
            } else {
                Schema::Any
            };
            Schema::Array(Box::new(item_schema))
        }
        "object" => Schema::Object(ObjectSchema {
            fields: Vec::new(),
            allow_additional_fields: true,
        }),
        other => {
            return Err(PyRuntimeError::new_err(format!(
                "Unknown field_type: {}",
                other
            )))
        }
    };

    Ok(schema)
}

fn usage_to_pydict(py: Python<'_>, usage: &Usage) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    dict.set_item("prompt_tokens", usage.prompt_tokens)?;
    dict.set_item("completion_tokens", usage.completion_tokens)?;
    dict.set_item("total_tokens", usage.total_tokens)?;
    Ok(dict.into())
}

/// Parse malformed JSON-ish text into proper JSON with healing metadata.
#[pyfunction]
#[pyo3(signature = (text, config=None))]
fn heal_json(
    py: Python<'_>,
    text: &str,
    config: Option<&Bound<'_, PyDict>>,
) -> PyResult<ParseResult> {
    let parser_config = if let Some(_cfg) = config {
        ParserConfig::default()
    } else {
        ParserConfig::default()
    };

    let parser = JsonishParser::with_config(parser_config);
    let result = parser
        .parse(text)
        .map_err(|e| PyRuntimeError::new_err(format!("Parsing failed: {}", e)))?;

    let py_value = pythonize::pythonize(py, &result.value)
        .map_err(|e| PyRuntimeError::new_err(format!("Conversion failed: {}", e)))?;

    let was_healed = !result.flags.is_empty();
    let flags: Vec<String> = result.flags.iter().map(|f| f.description()).collect();

    Ok(ParseResult {
        value: py_value.into(),
        confidence: result.confidence,
        was_healed,
        flags,
    })
}

/// Coerce data to match a JSON schema with healing metadata.
#[pyfunction]
#[pyo3(signature = (data, schema, config=None))]
fn coerce_to_schema(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyDict>>,
) -> PyResult<CoercionResult> {
    let value: serde_json::Value = pythonize::depythonize(data)
        .map_err(|e| PyRuntimeError::new_err(format!("Invalid data: {}", e)))?;

    let schema_obj = if let Ok(schema_ref) = schema.extract::<PyRef<PySchema>>() {
        schema_ref.schema.clone()
    } else {
        let schema_dict: &Bound<'_, PyDict> = schema
            .downcast()
            .map_err(|_| PyRuntimeError::new_err("schema must be a dict or Schema".to_string()))?;
        let schema_value: serde_json::Value = pythonize::depythonize(schema_dict)
            .map_err(|e| PyRuntimeError::new_err(format!("Invalid schema: {}", e)))?;

        match schema_value {
            serde_json::Value::Object(map) => {
                let mut fields = Vec::new();
                if let Some(props) = map.get("properties") {
                    if let serde_json::Value::Object(props_obj) = props {
                        let required_fields = map.get("required").and_then(|r| {
                            if let serde_json::Value::Array(arr) = r {
                                Some(arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                            } else {
                                None
                            }
                        });

                        for (key, val) in props_obj {
                            let is_required = required_fields
                                .as_ref()
                                .map(|req| req.contains(&key.as_str()))
                                .unwrap_or(false);
                            let schema = Schema::from_json_schema_value(val).map_err(py_err)?;
                            fields.push((key.clone(), schema, is_required));
                        }
                    }
                }
                Schema::object(fields)
            }
            serde_json::Value::Array(arr) if !arr.is_empty() => {
                let schema = Schema::from_json_schema_value(&arr[0]).map_err(py_err)?;
                Schema::Array(Box::new(schema))
            }
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "Unsupported schema format: {}",
                    schema_value
                )))
            }
        }
    };

    let coercion_config = if let Some(_cfg) = config {
        CoercionConfig::default()
    } else {
        CoercionConfig::default()
    };

    let engine = CoercionEngine::with_config(coercion_config);
    let result = engine
        .coerce(&value, &schema_obj)
        .map_err(|e| PyRuntimeError::new_err(format!("Coercion failed: {}", e)))?;

    let py_value = pythonize::pythonize(py, &result.value)
        .map_err(|e| PyRuntimeError::new_err(format!("Conversion failed: {}", e)))?;

    let was_coerced = !result.flags.is_empty();
    let flags: Vec<String> = result.flags.iter().map(|f| f.description()).collect();

    Ok(CoercionResult {
        value: py_value.into(),
        confidence: result.confidence,
        was_coerced,
        flags,
    })
}

trait SchemaExt {
    fn from_json_schema_value(
        value: &serde_json::Value,
    ) -> std::result::Result<Self, SimpleAgentsError>
    where
        Self: Sized;
}

impl SchemaExt for Schema {
    fn from_json_schema_value(
        value: &serde_json::Value,
    ) -> std::result::Result<Self, SimpleAgentsError> {
        match value {
            serde_json::Value::String(type_name) => match type_name.as_str() {
                "string" => Ok(Schema::String),
                "number" => Ok(Schema::Float),
                "integer" => Ok(Schema::Int),
                "boolean" => Ok(Schema::Bool),
                _ => Ok(Schema::Any),
            },
            serde_json::Value::Object(map) => {
                if let Some(type_val) = map.get("type") {
                    if let serde_json::Value::String(type_name) = type_val {
                        match type_name.as_str() {
                            "object" => {
                                let mut fields = Vec::new();
                                if let Some(props) = map.get("properties") {
                                    if let serde_json::Value::Object(props_obj) = props {
                                        let required_fields = map.get("required").and_then(|r| {
                                            if let serde_json::Value::Array(arr) = r {
                                                Some(
                                                    arr.iter()
                                                        .filter_map(|v| v.as_str())
                                                        .collect::<Vec<_>>(),
                                                )
                                            } else {
                                                None
                                            }
                                        });

                                        for (key, val) in props_obj {
                                            let is_required = required_fields
                                                .as_ref()
                                                .map(|req| req.contains(&key.as_str()))
                                                .unwrap_or(false);
                                            let schema = Schema::from_json_schema_value(val)?;
                                            fields.push(simple_agents_healing::schema::Field {
                                                name: key.clone(),
                                                schema,
                                                required: is_required,
                                                aliases: Vec::new(),
                                                default: None,
                                                description: None,
                                                stream_annotation: simple_agents_healing::schema::StreamAnnotation::Normal,
                                            });
                                        }
                                    }
                                }
                                Ok(Schema::Object(
                                    simple_agents_healing::schema::ObjectSchema {
                                        fields,
                                        allow_additional_fields: true,
                                    },
                                ))
                            }
                            "array" => {
                                if let Some(items) = map.get("items") {
                                    Ok(Schema::Array(Box::new(Schema::from_json_schema_value(
                                        items,
                                    )?)))
                                } else {
                                    Ok(Schema::Array(Box::new(Schema::Any)))
                                }
                            }
                            "string" => Ok(Schema::String),
                            "number" => Ok(Schema::Float),
                            "integer" => Ok(Schema::Int),
                            "boolean" => Ok(Schema::Bool),
                            "null" => Ok(Schema::Null),
                            _ => Ok(Schema::Any),
                        }
                    } else {
                        Ok(Schema::Any)
                    }
                } else {
                    Ok(Schema::Any)
                }
            }
            _ => Ok(Schema::Any),
        }
    }
}

#[pymethods]
#[allow(clippy::useless_conversion)]
impl Client {
    #[new]
    #[pyo3(signature = (provider, api_key=None, api_base=None))]
    fn new(provider: &str, api_key: Option<String>, api_base: Option<String>) -> PyResult<Self> {
        let provider = provider_from_params(provider, api_key.as_deref(), api_base.as_deref())
            .map_err(py_err)?;
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(provider)
            .build()
            .map_err(py_err)?;
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            runtime: Mutex::new(runtime),
            client,
        })
    }

    #[pyo3(signature = (model, prompt, max_tokens=None, temperature=None))]
    fn complete(
        &self,
        model: &str,
        prompt: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> PyResult<String> {
        let request = build_request(model, prompt, max_tokens, temperature).map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }

    #[pyo3(signature = (model, prompt, max_tokens=None, temperature=None))]
    fn complete_with_metadata(
        &self,
        py: Python<'_>,
        model: &str,
        prompt: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> PyResult<ResponseWithMetadata> {
        let request = build_request(model, prompt, max_tokens, temperature).map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let start = Instant::now();
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;
        let latency_ms = start.elapsed().as_millis() as u64;
        let usage = usage_to_pydict(py, &response.usage)?;
        let content = response.content().unwrap_or_default().to_string();
        let finish_reason = response
            .choices
            .first()
            .map(|c| format!("{:?}", c.finish_reason))
            .unwrap_or_else(|| "unknown".to_string());

        let (was_healed, healing_confidence, healing_error, flags) =
            if let Some(meta) = &response.healing_metadata {
                (
                    true,
                    Some(meta.confidence),
                    Some(meta.original_error.clone()),
                    meta.flags.iter().map(|f| f.description()).collect(),
                )
            } else {
                (false, None, None, Vec::new())
            };

        let tool_calls_obj = pythonize::pythonize(py, &Vec::<ToolCall>::new())
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

    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn complete_messages(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<String> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            None,
            None,
            None,
            None,
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }

    #[pyo3(signature = (model, messages, tools, tool_choice=None, max_tokens=None, temperature=None, top_p=None))]
    fn complete_with_tools(
        &self,
        py: Python<'_>,
        model: &str,
        messages: &Bound<'_, PyAny>,
        tools: &Bound<'_, PyAny>,
        tool_choice: Option<&Bound<'_, PyAny>>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<ResponseWithMetadata> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let tools = parse_tools(tools).map_err(py_err)?;
        let tool_choice = match tool_choice {
            Some(choice) => Some(parse_tool_choice(choice).map_err(py_err)?),
            None => None,
        };
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            None,
            Some(tools),
            tool_choice,
            None,
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let start = Instant::now();
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;
        let latency_ms = start.elapsed().as_millis() as u64;
        let usage = usage_to_pydict(py, &response.usage)?;
        let content = response.content().unwrap_or_default().to_string();
        let finish_reason = response
            .choices
            .first()
            .map(|c| format!("{:?}", c.finish_reason))
            .unwrap_or_else(|| "unknown".to_string());

        let (was_healed, healing_confidence, healing_error, flags) =
            if let Some(meta) = &response.healing_metadata {
                (
                    true,
                    Some(meta.confidence),
                    Some(meta.original_error.clone()),
                    meta.flags.iter().map(|f| f.description()).collect(),
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

    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn complete_messages_with_metadata(
        &self,
        py: Python<'_>,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<ResponseWithMetadata> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            None,
            None,
            None,
            None,
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let start = Instant::now();
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;
        let latency_ms = start.elapsed().as_millis() as u64;
        let usage = usage_to_pydict(py, &response.usage)?;
        let content = response.content().unwrap_or_default().to_string();
        let finish_reason = response
            .choices
            .first()
            .map(|c| format!("{:?}", c.finish_reason))
            .unwrap_or_else(|| "unknown".to_string());

        let (was_healed, healing_confidence, healing_error, flags) =
            if let Some(meta) = &response.healing_metadata {
                (
                    true,
                    Some(meta.confidence),
                    Some(meta.original_error.clone()),
                    meta.flags.iter().map(|f| f.description()).collect(),
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

    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn complete_json(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<String> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            Some(ResponseFormat::JsonObject),
            None,
            None,
            None,
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }

    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn complete_json_healed(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<HealedJsonResult> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            Some(ResponseFormat::JsonObject),
            None,
            None,
            None,
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;

        let healed_response: HealedJsonResponse = runtime
            .block_on(self.client.complete_json(&request))
            .map_err(py_err)?;

        let content = healed_response
            .response
            .content()
            .unwrap_or_default()
            .to_string();
        let confidence = healed_response.parsed.confidence;
        let was_healed = !healed_response.parsed.flags.is_empty();
        let flags = healed_response
            .parsed
            .flags
            .iter()
            .map(|f| f.description())
            .collect();

        Ok(HealedJsonResult {
            content,
            confidence,
            was_healed,
            flags,
        })
    }

    #[pyo3(signature = (model, messages, schema, schema_name, max_tokens=None, temperature=None, top_p=None, strict=true))]
    fn complete_json_schema(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        schema_name: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        strict: bool,
    ) -> PyResult<String> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let schema_value: serde_json::Value = pythonize::depythonize(schema).map_err(|_| {
            py_err(SimpleAgentsError::Config(
                "schema must be JSON-serializable".to_string(),
            ))
        })?;
        let response_format = ResponseFormat::JsonSchema {
            json_schema: JsonSchemaFormat {
                name: schema_name.to_string(),
                schema: schema_value,
                strict: Some(strict),
            },
        };
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            Some(response_format),
            None,
            None,
            None,
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }

    /// Stream a completion response.
    ///
    /// Returns an iterator that yields StreamChunk objects as they arrive.
    ///
    /// # Arguments
    ///
    /// * `model` - Model identifier (e.g., "gpt-4o-mini")
    /// * `messages` - List of message dicts with "role" and "content"
    /// * `max_tokens` - Optional maximum tokens to generate
    /// * `temperature` - Optional sampling temperature (0.0-2.0)
    /// * `top_p` - Optional top-p sampling parameter
    ///
    /// # Returns
    ///
    /// PyStreamIterator that yields StreamChunk objects.
    ///
    /// # Example
    ///
    /// ```python
    /// client = Client("openai")
    /// messages = [{"role": "user", "content": "Hello!"}]
    /// for chunk in client.stream("gpt-4o-mini", messages):
    ///     print(chunk.content, end="", flush=True)
    /// ```
    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn stream<'py>(
        &self,
        py: Python<'py>,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Create the PyStreamIterator
        let iterator =
            PyStreamIterator::new(self, model, messages, max_tokens, temperature, top_p)?;

        // Return it as a Python object (it implements __iter__/__next__)
        Ok(Bound::new(py, iterator)?.into_any())
    }

    /// Stream a structured completion response.
    ///
    /// Returns an iterator that yields StructuredEvent objects with
    /// progressive parsing and final complete value.
    ///
    /// # Arguments
    ///
    /// * `model` - Model identifier (e.g., "gpt-4o-mini")
    /// * `messages` - List of message dicts with "role" and "content"
    /// * `schema` - JSON schema dict for validation
    /// * `max_tokens` - Optional maximum tokens to generate
    /// * `temperature` - Optional sampling temperature (0.0-2.0)
    /// * `top_p` - Optional top-p sampling parameter
    ///
    /// # Returns
    ///
    /// StructuredStreamIterator that yields StructuredEvent objects.
    ///
    /// # Example
    ///
    /// ```python
    /// client = Client("openai")
    /// schema = {
    ///     "type": "object",
    ///     "properties": {
    ///         "name": {"type": "string"},
    ///         "age": {"type": "number"}
    ///     }
    /// }
    /// messages = [{"role": "user", "content": "Extract info from: John is 30 years old"}]
    /// for event in client.stream_structured("gpt-4o-mini", messages, schema):
    ///     if event.is_partial:
    ///         print(f"Partial: {event.partial_value}")
    ///     else:
    ///         print(f"Complete: {event.value}")
    /// ```
    #[pyo3(signature = (model, messages, schema, max_tokens=None, temperature=None, top_p=None))]
    fn stream_structured<'py>(
        &self,
        py: Python<'py>,
        model: &str,
        messages: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Parse schema
        let schema_value: Value = pythonize::depythonize(schema)
            .map_err(|_| PyRuntimeError::new_err("schema must be JSON-serializable".to_string()))?;

        // Validate schema is an object
        if !schema_value.is_object() {
            return Err(PyRuntimeError::new_err("schema must be a dict/object".to_string()));
        }

        // Parse messages and build request
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            Some(ResponseFormat::JsonObject),
            None,
            None,
            Some(true),
        )
        .map_err(py_err)?;

        // Get runtime and create stream
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;

        // Get the raw stream from the client
        let raw_stream = runtime
            .block_on(self.client.stream(&request))
            .map_err(py_err)?;

        let healing = HealingIntegration::new(HealingConfig::lenient());

        // Create structured stream
        let structured_stream: StructuredStream<_, Value> =
            StructuredStream::new(raw_stream, schema_value, Some(healing));

        // Create Python iterator
        let iterator = StructuredStreamIterator {
            stream: Some(Box::pin(structured_stream)),
        };

        Ok(Bound::new(py, iterator)?.into_any())
    }
}

#[pymodule]
fn simple_agents_py(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Client>()?;
    module.add_class::<ClientBuilder>()?;
    module.add_class::<HealedJsonResult>()?;
    module.add_class::<ParseResult>()?;
    module.add_class::<CoercionResult>()?;
    module.add_class::<PySchema>()?;
    module.add_class::<SchemaBuilder>()?;
    module.add_class::<StreamingParser>()?;
    module.add_class::<StreamChunk>()?;
    module.add_class::<PyStreamIterator>()?;
    module.add_class::<PyStructuredEvent>()?;
    module.add_class::<StructuredStreamIterator>()?;
    module.add_class::<ResponseWithMetadata>()?;
    module.add_function(wrap_pyfunction!(heal_json, module)?)?;
    module.add_function(wrap_pyfunction!(coerce_to_schema, module)?)?;
    Ok(())
}
