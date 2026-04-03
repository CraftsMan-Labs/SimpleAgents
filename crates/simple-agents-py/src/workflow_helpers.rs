use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;
use simple_agents_workflow::{
    YamlWorkflowCustomWorkerExecutor, YamlWorkflowEvent, YamlWorkflowEventSink,
    YamlWorkflowRunOptions,
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
        input_text: &str,
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
            let Some(topic) = payload
                .get("topic")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            else {
                return Err("custom worker payload.topic must be a non-empty string".to_string());
            };
            let kwargs = pyo3::types::PyDict::new_bound(py);
            let context_obj =
                pythonize::pythonize(py, context).map_err(|error| error.to_string())?;
            let payload_obj =
                pythonize::pythonize(py, payload).map_err(|error| error.to_string())?;
            kwargs
                .set_item("email_text", input_text)
                .map_err(|error| error.to_string())?;
            kwargs
                .set_item("context", context_obj)
                .map_err(|error| error.to_string())?;
            kwargs
                .set_item("payload", payload_obj)
                .map_err(|error| error.to_string())?;

            let result = function.call((topic,), Some(&kwargs)).map_err(|error| {
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

pub(crate) fn parse_workflow_run_options(
    workflow_options: Option<&Bound<'_, PyAny>>,
) -> PyResult<YamlWorkflowRunOptions> {
    workflow_options
        .map(|value| {
            pythonize::depythonize::<YamlWorkflowRunOptions>(value).map_err(|error| {
                PyRuntimeError::new_err(format!("invalid workflow_options: {error}"))
            })
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

pub(crate) fn parse_workflow_input_value(workflow_input: &Bound<'_, PyAny>) -> PyResult<Value> {
    let workflow_input_value: Value = pythonize::depythonize(workflow_input)
        .map_err(|error| PyRuntimeError::new_err(format!("invalid workflow_input: {error}")))?;
    if !workflow_input_value.is_object() {
        return Err(PyRuntimeError::new_err(
            "workflow_input must be a dict/object".to_string(),
        ));
    }
    Ok(workflow_input_value)
}

pub(crate) struct PythonWorkflowEventSink {
    pub(crate) callback: Option<Py<PyAny>>,
}

unsafe impl Send for PythonWorkflowEventSink {}
unsafe impl Sync for PythonWorkflowEventSink {}

impl YamlWorkflowEventSink for PythonWorkflowEventSink {
    fn emit(&self, event: &YamlWorkflowEvent) {
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
                eprintln!("[simple-agents-py] workflow event callback failed: {error}");
            }
        });
    }
}
