//! C-compatible FFI bindings for SimpleAgents.
//!
//! Provides a minimal C API surface:
//! - `sa_client_new` / `sa_client_free` — lifetime management
//! - `sa_complete` — blocking completion (request as JSON)
//! - `sa_stream` — streaming completion with callback
//! - `sa_run_workflow` — execute a YAML workflow
//! - `sa_last_error_message` / `sa_string_free` — error + memory helpers

use futures_util::StreamExt;
use serde_json::Value as JsonValue;
use simple_agent_type::prelude::{ApiKey, CompletionRequest, Provider, Result, SimpleAgentsError};
use simple_agents_core::{CompletionOptions, CompletionOutcome, SimpleAgentsClient};
use simple_agents_providers::openai::OpenAiCompatProvider;
use simple_agents_workflow::yaml_runner::{
    workflow_execution, YamlWorkflowExecutionFlags, YamlWorkflowExecutionRequest,
    YamlWorkflowExecutorBinding, YamlWorkflowRunOptions, YamlWorkflowSource,
};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

type Runtime = tokio::runtime::Runtime;

struct FfiClient {
    runtime: Mutex<Runtime>,
    client: SimpleAgentsClient,
}

#[repr(C)]
pub struct SaClient {
    inner: FfiClient,
}

type SaStreamCallback =
    Option<extern "C" fn(event_json: *const c_char, user_data: *mut c_void) -> i32>;

// ---------------------------------------------------------------------------
// Thread-local error slot
// ---------------------------------------------------------------------------

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_last_error(message: impl Into<String>) {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = Some(message.into());
    });
}

fn clear_last_error() {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

fn take_last_error() -> Option<String> {
    LAST_ERROR.with(|slot| slot.borrow_mut().take())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_runtime() -> Result<Runtime> {
    Runtime::new().map_err(|e| SimpleAgentsError::Config(format!("failed to build runtime: {e}")))
}

unsafe fn cstr_to_string(ptr: *const c_char, field: &str) -> Result<String> {
    if ptr.is_null() {
        return Err(SimpleAgentsError::Config(format!("{field} cannot be null")));
    }
    let c_str = CStr::from_ptr(ptr);
    let value = c_str
        .to_str()
        .map_err(|_| SimpleAgentsError::Config(format!("{field} must be valid UTF-8")))?;
    if value.is_empty() {
        return Err(SimpleAgentsError::Config(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(value.to_string())
}

unsafe fn cstr_to_optional_string(ptr: *const c_char, field: &str) -> Result<Option<String>> {
    if ptr.is_null() {
        return Ok(None);
    }
    let c_str = CStr::from_ptr(ptr);
    let value = c_str
        .to_str()
        .map_err(|_| SimpleAgentsError::Config(format!("{field} must be valid UTF-8")))?;
    if value.is_empty() {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

fn ffi_result_string(result: Result<String>) -> *mut c_char {
    match result {
        Ok(value) => match CString::new(value) {
            Ok(c_string) => {
                clear_last_error();
                c_string.into_raw()
            }
            Err(_) => {
                set_last_error("response contained an interior null byte");
                std::ptr::null_mut()
            }
        },
        Err(error) => {
            set_last_error(error.to_string());
            std::ptr::null_mut()
        }
    }
}

fn ffi_guard<T>(action: impl FnOnce() -> Result<T>) -> *mut c_char
where
    T: Into<String>,
{
    let result = catch_unwind(AssertUnwindSafe(action));
    match result {
        Ok(inner) => ffi_result_string(inner.map(Into::into)),
        Err(_) => {
            set_last_error("panic occurred in FFI call");
            std::ptr::null_mut()
        }
    }
}

fn ffi_guard_status(action: impl FnOnce() -> Result<()>) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(action));
    match result {
        Ok(Ok(())) => {
            clear_last_error();
            0
        }
        Ok(Err(error)) => {
            set_last_error(error.to_string());
            -1
        }
        Err(_) => {
            set_last_error("panic occurred in FFI call");
            -1
        }
    }
}

// ---------------------------------------------------------------------------
// sa_client_new — create a client with explicit credentials
// ---------------------------------------------------------------------------

/// Create an OpenAI-compatible client.
///
/// `api_key` — required, non-empty API key.
/// `model` — informational only; the model is selected per-request.
/// `base_url` — may be null to use the OpenAI default.
///
/// # Safety
///
/// All non-null pointers must be valid null-terminated UTF-8 strings.
/// The returned pointer must be freed with `sa_client_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_client_new(
    api_key: *const c_char,
    _model: *const c_char,
    base_url: *const c_char,
) -> *mut SaClient {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<Box<SaClient>> {
        let key_str = cstr_to_string(api_key, "api_key")?;
        let key = ApiKey::new(key_str)?;
        let base = cstr_to_optional_string(base_url, "base_url")?;

        let provider: Arc<dyn Provider> = match base {
            Some(url) => Arc::new(OpenAiCompatProvider::with_base_url(key, url)?),
            None => Arc::new(OpenAiCompatProvider::new(key)?),
        };

        let client = SimpleAgentsClient::new(provider);
        let runtime = build_runtime()?;

        Ok(Box::new(SaClient {
            inner: FfiClient {
                runtime: Mutex::new(runtime),
                client,
            },
        }))
    }));

    match result {
        Ok(Ok(client)) => {
            clear_last_error();
            Box::into_raw(client)
        }
        Ok(Err(error)) => {
            set_last_error(error.to_string());
            std::ptr::null_mut()
        }
        Err(_) => {
            set_last_error("panic occurred in sa_client_new");
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// sa_client_free
// ---------------------------------------------------------------------------

/// Free a client created by `sa_client_new`.
///
/// # Safety
///
/// `client` must be null or a pointer returned by `sa_client_new`.
#[no_mangle]
pub unsafe extern "C" fn sa_client_free(client: *mut SaClient) {
    if client.is_null() {
        return;
    }
    drop(Box::from_raw(client));
}

// ---------------------------------------------------------------------------
// sa_complete — JSON-in / JSON-out completion
// ---------------------------------------------------------------------------

/// Execute a completion request. `request_json` is a JSON string matching the
/// `CompletionRequest` schema. Returns a JSON string with the full
/// `CompletionResponse`, or null on error (check `sa_last_error_message`).
///
/// # Safety
///
/// `client` must be a live pointer from `sa_client_new`.
/// `request_json` must be a valid null-terminated UTF-8 JSON string.
/// The returned pointer must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_complete(
    client: *mut SaClient,
    request_json: *const c_char,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null");
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let json_str = cstr_to_string(request_json, "request_json")?;
        let request: CompletionRequest = serde_json::from_str(&json_str)
            .map_err(|e| SimpleAgentsError::Config(format!("invalid request JSON: {e}")))?;

        let ffi = &(*client).inner;
        let runtime = ffi
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let outcome =
            runtime.block_on(ffi.client.complete(&request, CompletionOptions::default()))?;

        let response = match outcome {
            CompletionOutcome::Response(r) => r,
            _ => {
                return Err(SimpleAgentsError::Config(
                    "unexpected non-response outcome from complete".to_string(),
                ))
            }
        };

        serde_json::to_string(&response)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize response: {e}")))
    })
}

// ---------------------------------------------------------------------------
// sa_stream — streaming completion with callback
// ---------------------------------------------------------------------------

/// Stream a completion request. Each chunk is delivered as a JSON string to
/// `callback`. Return 0 from the callback to continue, non-zero to cancel.
///
/// Returns 0 on success, -1 on error (check `sa_last_error_message`).
///
/// # Safety
///
/// `client` must be a live pointer from `sa_client_new`.
/// `request_json` must be a valid null-terminated UTF-8 JSON string with
/// `"stream": true`.
/// `callback` must remain valid for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn sa_stream(
    client: *mut SaClient,
    request_json: *const c_char,
    callback: SaStreamCallback,
    user_data: *mut c_void,
) -> i32 {
    if client.is_null() {
        set_last_error("client cannot be null");
        return -1;
    }
    let Some(callback) = callback else {
        set_last_error("callback cannot be null");
        return -1;
    };

    ffi_guard_status(|| {
        let json_str = cstr_to_string(request_json, "request_json")?;
        let request: CompletionRequest = serde_json::from_str(&json_str)
            .map_err(|e| SimpleAgentsError::Config(format!("invalid request JSON: {e}")))?;

        let ffi = &(*client).inner;
        let runtime = ffi
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let outcome =
            runtime.block_on(ffi.client.complete(&request, CompletionOptions::default()))?;

        let mut stream = match outcome {
            CompletionOutcome::Stream(s) => s,
            _ => {
                return Err(SimpleAgentsError::Config(
                    "expected stream outcome; ensure request has stream: true".to_string(),
                ))
            }
        };

        runtime.block_on(async {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let payload = serde_json::to_string(&chunk).map_err(|e| {
                            SimpleAgentsError::Config(format!(
                                "failed to serialize stream chunk: {e}"
                            ))
                        })?;
                        let c_payload = CString::new(payload).map_err(|_| {
                            SimpleAgentsError::Config(
                                "stream chunk contains interior null byte".to_string(),
                            )
                        })?;
                        let status = callback(c_payload.as_ptr(), user_data);
                        if status != 0 {
                            return Err(SimpleAgentsError::Config(
                                "stream cancelled by callback".to_string(),
                            ));
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        })
    })
}

// ---------------------------------------------------------------------------
// sa_run_workflow — execute a YAML workflow
// ---------------------------------------------------------------------------

/// Run a YAML workflow file.
///
/// `yaml_path` — path to a `.yaml` / `.yml` workflow file.
/// `input_json` — JSON object string passed as `workflow_input`.
///
/// Returns a JSON string with the `YamlWorkflowRunOutput`, or null on error.
///
/// # Safety
///
/// `client` must be a live pointer from `sa_client_new`.
/// `yaml_path` and `input_json` must be valid null-terminated UTF-8 strings.
/// The returned pointer must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_run_workflow(
    client: *mut SaClient,
    yaml_path: *const c_char,
    input_json: *const c_char,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null");
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let path = cstr_to_string(yaml_path, "yaml_path")?;
        let input_str = cstr_to_string(input_json, "input_json")?;
        let input: JsonValue = serde_json::from_str(&input_str)
            .map_err(|e| SimpleAgentsError::Config(format!("invalid input JSON: {e}")))?;
        if !input.is_object() {
            return Err(SimpleAgentsError::Config(
                "input_json must be a JSON object".to_string(),
            ));
        }

        let ffi = &(*client).inner;
        let runtime = ffi
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(std::path::Path::new(&path)),
            workflow_input: &input,
            executor: YamlWorkflowExecutorBinding::Client(&ffi.client),
            custom_worker: None,
            options: &YamlWorkflowRunOptions::default(),
            flags: YamlWorkflowExecutionFlags::default(),
        };

        let output = runtime
            .block_on(workflow_execution::run(request))
            .map_err(|e| SimpleAgentsError::Config(format!("workflow execution failed: {e}")))?;

        serde_json::to_string(&output)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize output: {e}")))
    })
}

// ---------------------------------------------------------------------------
// sa_stream_workflow — stream a YAML workflow with C callback for events
// ---------------------------------------------------------------------------

/// Stream a YAML workflow file, delivering each event as a JSON string to
/// `callback`. Return 0 from the callback to continue, non-zero to cancel.
///
/// Returns 0 on success, -1 on error (check `sa_last_error_message`).
///
/// # Safety
///
/// `client` must be a live pointer from `sa_client_new`.
/// `yaml_path` and `input_json` must be valid null-terminated UTF-8 strings.
/// `callback` must remain valid for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn sa_stream_workflow(
    client: *mut SaClient,
    yaml_path: *const c_char,
    input_json: *const c_char,
    callback: SaStreamCallback,
    user_data: *mut c_void,
) -> i32 {
    if client.is_null() {
        set_last_error("client cannot be null");
        return -1;
    }

    let Some(callback) = callback else {
        set_last_error("callback cannot be null");
        return -1;
    };

    ffi_guard_status(|| {
        let path = cstr_to_string(yaml_path, "yaml_path")?;
        let input_str = cstr_to_string(input_json, "input_json")?;
        let input: JsonValue = serde_json::from_str(&input_str)
            .map_err(|e| SimpleAgentsError::Config(format!("invalid input JSON: {e}")))?;

        let ffi = &(*client).inner;
        let runtime = ffi
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        // Bridge: collect events and deliver each one via the C callback.
        use std::cell::Cell;
        struct FfiEventSink {
            callback: extern "C" fn(*const c_char, *mut c_void) -> i32,
            user_data: *mut c_void,
            cancelled: Cell<bool>,
        }

        // Safety: The callback and user_data are valid for the duration of the
        // `block_on` call on this thread. FfiEventSink is not `Send` but we
        // don't move it across threads.
        unsafe impl Send for FfiEventSink {}
        unsafe impl Sync for FfiEventSink {}

        impl simple_agents_workflow::yaml_runner::YamlWorkflowEventSink for FfiEventSink {
            fn emit(&self, event: &simple_agents_workflow::yaml_runner::YamlWorkflowEvent) {
                if self.cancelled.get() {
                    return;
                }
                let payload = match serde_json::to_string(event) {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let c_payload = match std::ffi::CString::new(payload) {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let status = (self.callback)(c_payload.as_ptr(), self.user_data);
                if status != 0 {
                    self.cancelled.set(true);
                }
            }

            fn is_cancelled(&self) -> bool {
                self.cancelled.get()
            }
        }

        let event_sink = FfiEventSink {
            callback,
            user_data,
            cancelled: Cell::new(false),
        };

        let request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(std::path::Path::new(&path)),
            workflow_input: &input,
            executor: YamlWorkflowExecutorBinding::Client(&ffi.client),
            custom_worker: None,
            options: &YamlWorkflowRunOptions::default(),
            flags: YamlWorkflowExecutionFlags {
                workflow_streaming: true,
                ..YamlWorkflowExecutionFlags::default()
            },
        };

        runtime
            .block_on(workflow_execution::stream(request, &event_sink))
            .map_err(|e| SimpleAgentsError::Config(format!("workflow stream failed: {e}")))?;

        Ok(())
    })
}

// ---------------------------------------------------------------------------
// sa_resume — resume from a serialized checkpoint JSON
// ---------------------------------------------------------------------------

/// Resume a workflow from a previously captured checkpoint JSON string.
///
/// `checkpoint_json` — JSON string matching the `WorkflowCheckpoint` schema
/// (returned as part of a failed workflow output).
///
/// Returns a JSON string with `YamlWorkflowRunOutput`, or null on error.
///
/// # Safety
///
/// `client` must be a live pointer from `sa_client_new`.
/// `checkpoint_json` must be a valid null-terminated UTF-8 JSON string.
/// The returned pointer must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_resume(
    client: *mut SaClient,
    checkpoint_json: *const c_char,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null");
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let checkpoint_str = cstr_to_string(checkpoint_json, "checkpoint_json")?;
        let checkpoint: serde_json::Value = serde_json::from_str(&checkpoint_str)
            .map_err(|e| SimpleAgentsError::Config(format!("invalid checkpoint JSON: {e}")))?;

        let workflow_path = checkpoint
            .get("workflow_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                SimpleAgentsError::Config("checkpoint must have workflow_path".to_string())
            })?
            .to_string();

        let messages_val = checkpoint
            .get("original_messages")
            .cloned()
            .unwrap_or(serde_json::json!([]));
        let input = serde_json::json!({ "messages": messages_val });

        let ffi = &(*client).inner;
        let runtime = ffi
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(std::path::Path::new(&workflow_path)),
            workflow_input: &input,
            executor: YamlWorkflowExecutorBinding::Client(&ffi.client),
            custom_worker: None,
            options: &YamlWorkflowRunOptions::default(),
            flags: YamlWorkflowExecutionFlags::default(),
        };

        let output = runtime
            .block_on(workflow_execution::run(request))
            .map_err(|e| SimpleAgentsError::Config(format!("workflow execution failed: {e}")))?;

        serde_json::to_string(&output)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize output: {e}")))
    })
}

// ---------------------------------------------------------------------------
// sa_last_error_message / sa_string_free
// ---------------------------------------------------------------------------

/// Get the last error message for the current thread, or null if none.
/// The returned string must be freed with `sa_string_free`.
#[no_mangle]
pub extern "C" fn sa_last_error_message() -> *mut c_char {
    match take_last_error() {
        Some(message) => match CString::new(message) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Free a string returned by any SimpleAgents FFI function.
///
/// # Safety
///
/// `ptr` must be null or a pointer returned by a SimpleAgents FFI function.
#[no_mangle]
pub unsafe extern "C" fn sa_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    drop(CString::from_raw(ptr));
}
