//! C-compatible FFI bindings for SimpleAgents.

use simple_agent_type::message::Message;
use simple_agent_type::prelude::{ApiKey, CompletionRequest, Provider, Result, SimpleAgentsError};
use simple_agents_core::{
    CompletionOptions, CompletionOutcome, SimpleAgentsClient, SimpleAgentsClientBuilder,
};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

type Runtime = tokio::runtime::Runtime;

struct FfiClient {
    runtime: Mutex<Runtime>,
    client: SimpleAgentsClient,
}

#[repr(C)]
pub struct SAClient {
    inner: FfiClient,
}

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

fn build_runtime() -> Result<Runtime> {
    Runtime::new().map_err(|e| SimpleAgentsError::Config(format!("Failed to build runtime: {e}")))
}

fn provider_from_env(provider_name: &str) -> Result<Arc<dyn Provider>> {
    match provider_name {
        "openai" => Ok(Arc::new(OpenAIProvider::from_env()?)),
        "anthropic" => Ok(Arc::new(AnthropicProvider::from_env()?)),
        "openrouter" => Ok(Arc::new(openrouter_from_env()?)),
        _ => Err(SimpleAgentsError::Config(format!(
            "Unknown provider '{provider_name}'"
        ))),
    }
}

fn openrouter_from_env() -> Result<OpenRouterProvider> {
    let api_key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
        SimpleAgentsError::Config("OPENROUTER_API_KEY environment variable is required".to_string())
    })?;
    let api_key = ApiKey::new(api_key)?;
    let base_url = std::env::var("OPENROUTER_API_BASE")
        .unwrap_or_else(|_| OpenRouterProvider::DEFAULT_BASE_URL.to_string());
    OpenRouterProvider::with_base_url(api_key, base_url)
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

fn build_client(provider: Arc<dyn Provider>) -> Result<SimpleAgentsClient> {
    SimpleAgentsClientBuilder::new()
        .with_provider(provider)
        .build()
}

fn build_request(
    model: &str,
    prompt: &str,
    max_tokens: i32,
    temperature: f32,
) -> Result<CompletionRequest> {
    let mut builder = CompletionRequest::builder()
        .model(model)
        .message(Message::user(prompt));

    if max_tokens > 0 {
        builder = builder.max_tokens(max_tokens as u32);
    }

    if temperature >= 0.0 {
        builder = builder.temperature(temperature);
    }

    builder.build()
}

fn ffi_result_string(result: Result<String>) -> *mut c_char {
    match result {
        Ok(value) => match CString::new(value) {
            Ok(c_string) => {
                clear_last_error();
                c_string.into_raw()
            }
            Err(_) => {
                set_last_error("Response contained an interior null byte".to_string());
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
            set_last_error("Panic occurred in FFI call".to_string());
            std::ptr::null_mut()
        }
    }
}

/// Create a client from environment variables for a provider.
///
/// `provider_name` must be one of: "openai", "anthropic", "openrouter".
///
/// # Safety
///
/// The `provider_name` pointer must be a valid null-terminated C string or null.
/// The returned pointer must be freed with `sa_client_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_client_new_from_env(provider_name: *const c_char) -> *mut SAClient {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<Box<SAClient>> {
        let provider = cstr_to_string(provider_name, "provider_name")?;
        let provider = provider_from_env(&provider)?;
        let client = build_client(provider)?;
        let runtime = build_runtime()?;

        Ok(Box::new(SAClient {
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
            set_last_error("Panic occurred in sa_client_new_from_env".to_string());
            std::ptr::null_mut()
        }
    }
}

/// Free a client created by `sa_client_new_from_env`.
///
/// # Safety
///
/// The `client` pointer must be null or a valid pointer returned by `sa_client_new_from_env`.
/// After calling this function, the pointer is no longer valid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn sa_client_free(client: *mut SAClient) {
    if client.is_null() {
        return;
    }

    drop(Box::from_raw(client));
}

/// Execute a completion request with a single user prompt.
///
/// Use `max_tokens <= 0` to omit, and `temperature < 0.0` to omit.
///
/// # Safety
///
/// The `client` pointer must be a valid pointer returned by `sa_client_new_from_env`.
/// The `model` and `prompt` pointers must be valid null-terminated C strings.
/// The returned pointer must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_complete(
    client: *mut SAClient,
    model: *const c_char,
    prompt: *const c_char,
    max_tokens: i32,
    temperature: f32,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null".to_string());
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let model = cstr_to_string(model, "model")?;
        let prompt = cstr_to_string(prompt, "prompt")?;
        let request = build_request(&model, &prompt, max_tokens, temperature)?;

        let client = &(*client).inner;
        let runtime = client
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;
        let outcome = runtime.block_on(
            client
                .client
                .complete(&request, CompletionOptions::default()),
        )?;
        let response = match outcome {
            CompletionOutcome::Response(response) => response,
            CompletionOutcome::Stream(_) => {
                return Err(SimpleAgentsError::Config(
                    "streaming response returned from complete".to_string(),
                ))
            }
            CompletionOutcome::HealedJson(_) => {
                return Err(SimpleAgentsError::Config(
                    "healed json response returned from complete".to_string(),
                ))
            }
            CompletionOutcome::CoercedSchema(_) => {
                return Err(SimpleAgentsError::Config(
                    "schema response returned from complete".to_string(),
                ))
            }
        };

        Ok(response.content().unwrap_or_default().to_string())
    })
}

/// Get the last error message for the current thread.
///
/// Returns null if there is no error. Caller must free the string.
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

/// Free a string returned by SimpleAgents FFI.
///
/// # Safety
///
/// The `value` pointer must be null or a valid pointer returned by a SimpleAgents FFI function.
/// After calling this function, the pointer is no longer valid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn sa_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    drop(CString::from_raw(value));
}
