use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Map;
use serde_json::Value;
use simple_agents_workflow::yaml_runner::{
    YamlWorkflowCustomWorkerExecutor, YamlWorkflowEvent, YamlWorkflowEventSink,
    YamlWorkflowExecutionFlags, YamlWorkflowRunOptions,
};
use std::sync::Mutex;

pub(crate) fn workflow_root_path(workflow_path: &std::path::Path) -> std::path::PathBuf {
    workflow_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

pub(crate) struct PythonCustomWorkerExecutor {
    pub(crate) workflow_root: std::path::PathBuf,
}

#[async_trait::async_trait]
impl YamlWorkflowCustomWorkerExecutor for PythonCustomWorkerExecutor {
    async fn execute(
        &self,
        handler: &str,
        handler_file: Option<&str>,
        payload: &Value,
        context: &Value,
    ) -> std::result::Result<Value, String> {
        Python::with_gil(|py| {
            let configured_handler_file = handler_file.unwrap_or("handlers.py");
            let configured_path = std::path::PathBuf::from(configured_handler_file);
            let handlers_path = if configured_path.is_absolute() {
                configured_path
            } else {
                self.workflow_root.join(configured_path)
            };

            if !handlers_path.exists() {
                return Err(format!(
                    "custom worker handlers file not found: {}",
                    handlers_path.display()
                ));
            }

            let importlib_util = py
                .import_bound("importlib.util")
                .map_err(|error| error.to_string())?;
            let module_path = handlers_path.to_string_lossy().to_string();
            let spec = importlib_util
                .call_method1(
                    "spec_from_file_location",
                    ("simple_agents_workflow_handlers", module_path.as_str()),
                )
                .map_err(|error| error.to_string())?;
            if spec.is_none() {
                return Err(format!(
                    "failed to load module spec from {}",
                    handlers_path.display()
                ));
            }

            let module = importlib_util
                .call_method1("module_from_spec", (&spec,))
                .map_err(|error| error.to_string())?;
            let loader = spec.getattr("loader").map_err(|error| error.to_string())?;
            if loader.is_none() {
                return Err("module loader is missing for handlers.py".to_string());
            }
            loader
                .call_method1("exec_module", (&module,))
                .map_err(|error| error.to_string())?;

            let function = module.getattr(handler).map_err(|error| error.to_string())?;
            let kwargs = pyo3::types::PyDict::new_bound(py);
            let context_obj =
                pythonize::pythonize(py, context).map_err(|error| error.to_string())?;
            let payload_obj =
                pythonize::pythonize(py, payload).map_err(|error| error.to_string())?;
            kwargs
                .set_item("context", context_obj)
                .map_err(|error| error.to_string())?;
            kwargs
                .set_item("payload", payload_obj)
                .map_err(|error| error.to_string())?;

            let result = function.call((), Some(&kwargs)).map_err(|error| {
                format!(
                    "custom worker handler '{}' in '{}' failed: {}",
                    handler,
                    handlers_path.display(),
                    error
                )
            })?;
            pythonize::depythonize::<Value>(&result).map_err(|error| error.to_string())
        })
    }
}

pub(crate) struct RecordingWorkflowEventSink {
    events: Mutex<Vec<YamlWorkflowEvent>>,
}

impl RecordingWorkflowEventSink {
    pub(crate) fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn events_value(&self) -> PyResult<Value> {
        let events = match self.events.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => {
                eprintln!(
                    "[simple-agents-py] event sink lock poisoned while collecting events; recovering"
                );
                poisoned.into_inner().clone()
            }
        };
        serde_json::to_value(events).map_err(|error| {
            PyRuntimeError::new_err(format!("event serialization failed: {error}"))
        })
    }
}

impl YamlWorkflowEventSink for RecordingWorkflowEventSink {
    fn emit(&self, event: &YamlWorkflowEvent) {
        match self.events.lock() {
            Ok(mut events) => events.push(event.clone()),
            Err(poisoned) => {
                eprintln!(
                    "[simple-agents-py] event sink lock poisoned while recording event; recovering"
                );
                let mut events = poisoned.into_inner();
                events.push(event.clone());
            }
        }
    }
}

pub(crate) fn attach_workflow_events(
    value: &mut Value,
    event_sink: &RecordingWorkflowEventSink,
) -> PyResult<()> {
    let events_value = event_sink.events_value()?;
    match value {
        Value::Object(object) => {
            object.insert("events".to_string(), events_value);
            Ok(())
        }
        _ => Err(PyRuntimeError::new_err(
            "workflow output must be an object when include_events=true".to_string(),
        )),
    }
}

pub(crate) struct PythonWorkflowExecutionRequest {
    pub workflow_path: String,
    pub messages: Vec<Value>,
    pub context: Option<Value>,
    pub media: Option<Value>,
    pub input: Option<Value>,
    pub execution: PythonWorkflowExecutionOptions,
    pub workflow_options: Option<YamlWorkflowRunOptions>,
}

#[derive(Default)]
pub(crate) struct PythonWorkflowExecutionOptions {
    pub model: Option<String>,
    pub healing: bool,
    pub workflow_streaming: bool,
    pub node_llm_streaming: bool,
    pub split_stream_deltas: bool,
    pub debug_stream_parse: bool,
}

const fn default_true() -> bool {
    true
}

pub(crate) fn parse_workflow_execution_request(
    value: &Bound<'_, PyAny>,
) -> PyResult<PythonWorkflowExecutionRequest> {
    let raw: Value = pythonize::depythonize(value).map_err(|error| {
        PyRuntimeError::new_err(format!(
            "invalid workflow execution request: {error}. expected keys: workflow_path, messages, context?, media?, input?, execution?, workflow_options?"
        ))
    })?;
    let object = raw.as_object().ok_or_else(|| {
        PyRuntimeError::new_err("workflow execution request must be a dict/object".to_string())
    })?;
    let allowed_keys = [
        "workflow_path",
        "messages",
        "context",
        "media",
        "input",
        "execution",
        "workflow_options",
    ];
    for key in object.keys() {
        if !allowed_keys.contains(&key.as_str()) {
            return Err(PyRuntimeError::new_err(format!(
                "unknown workflow execution request key '{key}'; expected keys: workflow_path, messages, context?, media?, input?, execution?, workflow_options?"
            )));
        }
    }
    let workflow_path = object
        .get("workflow_path")
        .and_then(Value::as_str)
        .ok_or_else(|| PyRuntimeError::new_err("workflow_path is required".to_string()))?
        .to_string();
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| PyRuntimeError::new_err("messages is required".to_string()))?
        .clone();
    let context = object.get("context").cloned();
    let media = object.get("media").cloned();
    let input = object.get("input").cloned();
    let workflow_options = object
        .get("workflow_options")
        .map(|options| {
            serde_json::from_value::<YamlWorkflowRunOptions>(options.clone()).map_err(|error| {
                PyRuntimeError::new_err(format!("invalid workflow_options: {error}"))
            })
        })
        .transpose()?;
    let execution = if let Some(execution_value) = object.get("execution") {
        let execution_object = execution_value.as_object().ok_or_else(|| {
            PyRuntimeError::new_err("execution must be a dict/object".to_string())
        })?;
        let allowed_execution_keys = [
            "model",
            "healing",
            "workflow_streaming",
            "node_llm_streaming",
            "split_stream_deltas",
            "debug_stream_parse",
        ];
        for key in execution_object.keys() {
            if !allowed_execution_keys.contains(&key.as_str()) {
                return Err(PyRuntimeError::new_err(format!(
                    "unknown execution key '{key}'; expected keys: model?, healing?, workflow_streaming?, node_llm_streaming?, split_stream_deltas?, debug_stream_parse?"
                )));
            }
        }
        let model = execution_object
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string);
        let healing = execution_object
            .get("healing")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let workflow_streaming = execution_object
            .get("workflow_streaming")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let node_llm_streaming = execution_object
            .get("node_llm_streaming")
            .and_then(Value::as_bool)
            .unwrap_or(default_true());
        let split_stream_deltas = execution_object
            .get("split_stream_deltas")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let debug_stream_parse = execution_object
            .get("debug_stream_parse")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        PythonWorkflowExecutionOptions {
            model,
            healing,
            workflow_streaming,
            node_llm_streaming,
            split_stream_deltas,
            debug_stream_parse,
        }
    } else {
        PythonWorkflowExecutionOptions {
            node_llm_streaming: default_true(),
            ..PythonWorkflowExecutionOptions::default()
        }
    };
    let request = PythonWorkflowExecutionRequest {
        workflow_path,
        messages,
        context,
        media,
        input,
        execution,
        workflow_options,
    };
    if request.workflow_path.trim().is_empty() {
        return Err(PyRuntimeError::new_err(
            "workflow_path cannot be empty".to_string(),
        ));
    }
    if request.messages.is_empty() {
        return Err(PyRuntimeError::new_err(
            "messages must contain at least one message".to_string(),
        ));
    }
    Ok(request)
}

pub(crate) fn workflow_execution_flags(
    options: &PythonWorkflowExecutionOptions,
) -> YamlWorkflowExecutionFlags {
    YamlWorkflowExecutionFlags {
        healing: options.healing,
        workflow_streaming: options.workflow_streaming,
        node_llm_streaming: options.node_llm_streaming,
        split_stream_deltas: options.split_stream_deltas,
        debug_stream_parse: options.debug_stream_parse,
    }
}

pub(crate) fn workflow_execution_options(
    request: &PythonWorkflowExecutionRequest,
) -> YamlWorkflowRunOptions {
    let mut options = request.workflow_options.clone().unwrap_or_default();
    if let Some(model) = request.execution.model.clone() {
        options.model = Some(model);
    }
    options
}

pub(crate) fn build_workflow_input_from_execution_request(
    request: &PythonWorkflowExecutionRequest,
) -> PyResult<Value> {
    let mut object = Map::new();

    let extra_input = request
        .input
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in extra_input {
        object.insert(key, value);
    }

    object.insert(
        "messages".to_string(),
        Value::Array(request.messages.clone()),
    );
    if let Some(context) = request.context.clone() {
        object.insert("context".to_string(), context);
    }
    if let Some(media) = request.media.clone() {
        object.insert("media".to_string(), media);
    }

    Ok(Value::Object(object))
}

pub(crate) struct PythonWorkflowEventSink {
    pub(crate) callback: Option<Py<PyAny>>,
    pub(crate) callback_error: Mutex<Option<String>>,
}

unsafe impl Send for PythonWorkflowEventSink {}
unsafe impl Sync for PythonWorkflowEventSink {}

impl YamlWorkflowEventSink for PythonWorkflowEventSink {
    fn is_cancelled(&self) -> bool {
        self.callback_error
            .lock()
            .map(|error| error.is_some())
            .unwrap_or(true)
    }

    fn emit(&self, event: &YamlWorkflowEvent) {
        if self.is_cancelled() {
            return;
        }
        let Some(callback) = self.callback.as_ref() else {
            return;
        };

        Python::with_gil(|py| {
            let event_value = match serde_json::to_value(event) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("[simple-agents-py] failed to serialize workflow event: {error}");
                    return;
                }
            };
            let py_event = match pythonize::pythonize(py, &event_value) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "[simple-agents-py] failed to convert workflow event for callback: {error}"
                    );
                    return;
                }
            };
            if let Err(error) = callback.bind(py).call1((py_event,)) {
                if let Ok(mut callback_error) = self.callback_error.lock() {
                    *callback_error = Some(error.to_string());
                }
            }
        });
    }
}

pub(crate) struct CombinedWorkflowEventSink {
    events: Mutex<Vec<YamlWorkflowEvent>>,
    pub(crate) callback: Option<Py<PyAny>>,
    callback_error: Mutex<Option<String>>,
    record: bool,
}

unsafe impl Send for CombinedWorkflowEventSink {}
unsafe impl Sync for CombinedWorkflowEventSink {}

impl CombinedWorkflowEventSink {
    pub(crate) fn new(record: bool, callback: Option<Py<PyAny>>) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            callback,
            callback_error: Mutex::new(None),
            record,
        }
    }

    pub(crate) fn attach_to_output(&self, value: &mut Value) -> PyResult<()> {
        if !self.record {
            return Ok(());
        }
        let events = match self.events.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => {
                eprintln!(
                    "[simple-agents-py] event sink lock poisoned while collecting events; recovering"
                );
                poisoned.into_inner().clone()
            }
        };
        let events_value = serde_json::to_value(events).map_err(|error| {
            PyRuntimeError::new_err(format!("event serialization failed: {error}"))
        })?;
        match value {
            Value::Object(object) => {
                object.insert("events".to_string(), events_value);
                Ok(())
            }
            _ => Err(PyRuntimeError::new_err(
                "workflow output must be an object when include_events=true".to_string(),
            )),
        }
    }
}

impl YamlWorkflowEventSink for CombinedWorkflowEventSink {
    fn is_cancelled(&self) -> bool {
        self.callback_error
            .lock()
            .map(|error| error.is_some())
            .unwrap_or(true)
    }

    fn emit(&self, event: &YamlWorkflowEvent) {
        if self.is_cancelled() {
            return;
        }
        if self.record {
            match self.events.lock() {
                Ok(mut events) => events.push(event.clone()),
                Err(poisoned) => {
                    eprintln!(
                        "[simple-agents-py] event sink lock poisoned while recording event; recovering"
                    );
                    let mut events = poisoned.into_inner();
                    events.push(event.clone());
                }
            }
        }

        let Some(callback) = self.callback.as_ref() else {
            return;
        };

        Python::with_gil(|py| {
            let event_value = match serde_json::to_value(event) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("[simple-agents-py] failed to serialize workflow event: {error}");
                    return;
                }
            };
            let py_event = match pythonize::pythonize(py, &event_value) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "[simple-agents-py] failed to convert workflow event for callback: {error}"
                    );
                    return;
                }
            };
            if let Err(error) = callback.bind(py).call1((py_event,)) {
                if let Ok(mut callback_error) = self.callback_error.lock() {
                    *callback_error = Some(error.to_string());
                }
            }
        });
    }
}
