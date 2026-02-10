//! C-compatible FFI bindings for SimpleAgents.

use serde::Serialize;
use serde_json::Value as JsonValue;
use simple_agent_type::coercion::{CoercionFlag, CoercionResult};
use simple_agent_type::message::{Message, Role};
use simple_agent_type::prelude::{ApiKey, CompletionRequest, Provider, Result, SimpleAgentsError};
use simple_agent_type::response::{CompletionResponse, FinishReason, Usage};
use simple_agent_type::tool::{ToolCall, ToolType};
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, HealedJsonResponse, HealedSchemaResponse,
    SimpleAgentsClient, SimpleAgentsClientBuilder,
};
use simple_agents_healing::schema::{Field as SchemaField, ObjectSchema, Schema, StreamAnnotation};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

// Keep runtime ownership in the FFI layer so each client is self-contained.
type Runtime = tokio::runtime::Runtime;

struct FfiClient {
    runtime: Mutex<Runtime>,
    client: SimpleAgentsClient,
}

#[repr(C)]
pub struct SAClient {
    inner: FfiClient,
}

#[repr(C)]
pub struct SAMessage {
    pub role: *const c_char,
    pub content: *const c_char,
    pub name: *const c_char,
    pub tool_call_id: *const c_char,
}

#[derive(Serialize)]
struct FfiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct FfiToolCall {
    id: String,
    tool_type: String,
    function: FfiToolCallFunction,
}

#[derive(Serialize)]
struct FfiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize)]
struct FfiHealingData {
    value: JsonValue,
    flags: Vec<CoercionFlag>,
    confidence: f32,
}

#[derive(Serialize)]
struct FfiCompletionResult {
    id: String,
    model: String,
    role: String,
    content: Option<String>,
    tool_calls: Option<Vec<FfiToolCall>>,
    finish_reason: Option<String>,
    usage: FfiUsage,
    raw: Option<String>,
    healed: Option<FfiHealingData>,
    coerced: Option<FfiHealingData>,
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

fn build_client(provider: Arc<dyn Provider>) -> Result<SimpleAgentsClient> {
    SimpleAgentsClientBuilder::new()
        .with_provider(provider)
        .build()
}

fn build_request_from_messages(
    model: &str,
    messages: Vec<Message>,
    max_tokens: i32,
    temperature: f32,
    top_p: f32,
) -> Result<CompletionRequest> {
    let mut builder = CompletionRequest::builder().model(model).messages(messages);

    if max_tokens > 0 {
        builder = builder.max_tokens(max_tokens as u32);
    }

    if temperature >= 0.0 {
        builder = builder.temperature(temperature);
    }

    if top_p >= 0.0 {
        builder = builder.top_p(top_p);
    }

    builder.build()
}

fn build_request(
    model: &str,
    prompt: &str,
    max_tokens: i32,
    temperature: f32,
) -> Result<CompletionRequest> {
    build_request_from_messages(
        model,
        vec![Message::user(prompt)],
        max_tokens,
        temperature,
        -1.0,
    )
}

fn schema_aliases(value: Option<&JsonValue>) -> Vec<String> {
    value
        .and_then(JsonValue::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_schema_field(value: &JsonValue) -> Result<SchemaField> {
    let name = value
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| SimpleAgentsError::Config("schema field missing `name`".to_string()))?;
    let schema_value = value.get("schema").ok_or_else(|| {
        SimpleAgentsError::Config(format!("schema field `{name}` missing `schema`"))
    })?;

    Ok(SchemaField {
        name: name.to_string(),
        schema: parse_schema(schema_value)?,
        required: value
            .get("required")
            .and_then(JsonValue::as_bool)
            .unwrap_or(true),
        aliases: schema_aliases(value.get("aliases")),
        default: None,
        description: None,
        stream_annotation: StreamAnnotation::Normal,
    })
}

fn parse_schema(value: &JsonValue) -> Result<Schema> {
    let kind = value
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| SimpleAgentsError::Config("schema requires `kind`".to_string()))?
        .to_lowercase();

    match kind.as_str() {
        "string" => Ok(Schema::String),
        "int" => Ok(Schema::Int),
        "uint" => Ok(Schema::UInt),
        "float" => Ok(Schema::Float),
        "bool" => Ok(Schema::Bool),
        "null" => Ok(Schema::Null),
        "any" => Ok(Schema::Any),
        "array" => {
            let elements = value.get("elements").ok_or_else(|| {
                SimpleAgentsError::Config("array schema requires `elements`".to_string())
            })?;
            Ok(Schema::array(parse_schema(elements)?))
        }
        "union" => {
            let variants = value
                .get("variants")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| {
                    SimpleAgentsError::Config("union schema requires `variants` array".to_string())
                })?;
            let schemas = variants
                .iter()
                .map(parse_schema)
                .collect::<Result<Vec<_>>>()?;
            Ok(Schema::union(schemas))
        }
        "object" => {
            let fields = value
                .get("fields")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| {
                    SimpleAgentsError::Config("object schema requires `fields` array".to_string())
                })?;
            let converted = fields
                .iter()
                .map(parse_schema_field)
                .collect::<Result<Vec<_>>>()?;
            Ok(Schema::Object(ObjectSchema {
                fields: converted,
                allow_additional_fields: value
                    .get("allow_additional_fields")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false),
            }))
        }
        other => Err(SimpleAgentsError::Config(format!(
            "unsupported schema kind `{other}`"
        ))),
    }
}

fn completion_options(mode: Option<&str>, schema_json: Option<&str>) -> Result<CompletionOptions> {
    let mode = match mode.map(|m| m.to_ascii_lowercase()) {
        None => CompletionMode::Standard,
        Some(m) if m.is_empty() || m == "standard" => CompletionMode::Standard,
        Some(m) if m == "healed_json" => CompletionMode::HealedJson,
        Some(m) if m == "schema" => {
            let raw_schema = schema_json.ok_or_else(|| {
                SimpleAgentsError::Config("mode `schema` requires `schema_json`".to_string())
            })?;
            let value: JsonValue = serde_json::from_str(raw_schema)
                .map_err(|e| SimpleAgentsError::Config(format!("invalid `schema_json`: {e}")))?;
            CompletionMode::CoercedSchema(parse_schema(&value)?)
        }
        Some(other) => {
            return Err(SimpleAgentsError::Config(format!(
                "unknown mode `{other}` (expected standard|healed_json|schema)"
            )))
        }
    };

    Ok(CompletionOptions { mode })
}

fn role_to_string(role: Role) -> String {
    match role {
        Role::User => "user".to_string(),
        Role::Assistant => "assistant".to_string(),
        Role::System => "system".to_string(),
        Role::Tool => "tool".to_string(),
    }
}

fn finish_reason_to_string(finish_reason: FinishReason) -> String {
    match finish_reason {
        FinishReason::Stop => "stop".to_string(),
        FinishReason::Length => "length".to_string(),
        FinishReason::ContentFilter => "content_filter".to_string(),
        FinishReason::ToolCalls => "tool_calls".to_string(),
    }
}

fn tool_type_to_string(tool_type: ToolType) -> String {
    match tool_type {
        ToolType::Function => "function".to_string(),
    }
}

fn usage_to_ffi(usage: Usage) -> FfiUsage {
    FfiUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
    }
}

fn map_tool_calls(tool_calls: Option<Vec<ToolCall>>) -> Option<Vec<FfiToolCall>> {
    tool_calls.map(|calls| {
        calls
            .into_iter()
            .map(|call| FfiToolCall {
                id: call.id,
                tool_type: tool_type_to_string(call.tool_type),
                function: FfiToolCallFunction {
                    name: call.function.name,
                    arguments: call.function.arguments,
                },
            })
            .collect()
    })
}

fn healing_data_from(result: CoercionResult<JsonValue>) -> FfiHealingData {
    FfiHealingData {
        value: result.value,
        flags: result.flags,
        confidence: result.confidence,
    }
}

fn completion_result_from_response(
    response: CompletionResponse,
    healed: Option<FfiHealingData>,
    coerced: Option<FfiHealingData>,
) -> FfiCompletionResult {
    let content = response.content().map(str::to_string);
    let choice = response.choices.first();
    let role = choice
        .map(|c| role_to_string(c.message.role))
        .unwrap_or_else(|| "assistant".to_string());
    let finish_reason = choice.map(|c| finish_reason_to_string(c.finish_reason));
    let tool_calls = choice.and_then(|c| c.message.tool_calls.clone());
    let usage = response.usage;

    FfiCompletionResult {
        id: response.id.clone(),
        model: response.model.clone(),
        role: role.clone(),
        content: content.clone(),
        tool_calls: map_tool_calls(tool_calls),
        finish_reason,
        usage: usage_to_ffi(usage),
        raw: content,
        healed,
        coerced,
    }
}

fn parse_messages(messages: *const SAMessage, messages_len: usize) -> Result<Vec<Message>> {
    if messages.is_null() {
        return Err(SimpleAgentsError::Config(
            "messages cannot be null".to_string(),
        ));
    }
    if messages_len == 0 {
        return Err(SimpleAgentsError::Config(
            "messages cannot be empty".to_string(),
        ));
    }

    let input = unsafe { std::slice::from_raw_parts(messages, messages_len) };
    input
        .iter()
        .enumerate()
        .map(|(idx, msg)| {
            let role = unsafe { cstr_to_string(msg.role, &format!("messages[{idx}].role"))? }
                .to_ascii_lowercase();
            let content =
                unsafe { cstr_to_string(msg.content, &format!("messages[{idx}].content"))? };
            let name =
                unsafe { cstr_to_optional_string(msg.name, &format!("messages[{idx}].name"))? };
            let tool_call_id = unsafe {
                cstr_to_optional_string(msg.tool_call_id, &format!("messages[{idx}].tool_call_id"))?
            };

            let parsed = match role.as_str() {
                "user" => Message::user(content),
                "assistant" => Message::assistant(content),
                "system" => Message::system(content),
                "tool" => {
                    let call_id = tool_call_id.ok_or_else(|| {
                        SimpleAgentsError::Config(format!(
                            "messages[{idx}].tool_call_id is required for tool role"
                        ))
                    })?;
                    Message::tool(content, call_id)
                }
                _ => {
                    return Err(SimpleAgentsError::Config(format!(
                        "messages[{idx}].role must be one of user|assistant|system|tool"
                    )))
                }
            };

            Ok(match name {
                Some(name) => parsed.with_name(name),
                None => parsed,
            })
        })
        .collect()
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

/// Execute a completion request with full message input and return a structured JSON payload.
///
/// Use `max_tokens <= 0`, `temperature < 0.0`, or `top_p < 0.0` to omit those options.
/// `mode` supports `standard`, `healed_json`, and `schema`; when mode is `schema`, `schema_json`
/// must be a JSON object with the internal schema shape.
///
/// # Safety
///
/// - `client` must be a pointer returned by `sa_client_new_from_env`.
/// - `model` must be a valid null-terminated C string.
/// - `messages` must point to `messages_len` valid `SAMessage` values.
/// - Returned string must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_complete_messages_json(
    client: *mut SAClient,
    model: *const c_char,
    messages: *const SAMessage,
    messages_len: usize,
    max_tokens: i32,
    temperature: f32,
    top_p: f32,
    mode: *const c_char,
    schema_json: *const c_char,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null".to_string());
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let model = cstr_to_string(model, "model")?;
        let messages = parse_messages(messages, messages_len)?;
        let request =
            build_request_from_messages(&model, messages, max_tokens, temperature, top_p)?;

        let mode = cstr_to_optional_string(mode, "mode")?;
        let schema_json = cstr_to_optional_string(schema_json, "schema_json")?;
        let options = completion_options(mode.as_deref(), schema_json.as_deref())?;

        let client = &(*client).inner;
        let runtime = client
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;
        let outcome = runtime.block_on(client.client.complete(&request, options))?;

        let payload = match outcome {
            CompletionOutcome::Response(response) => {
                completion_result_from_response(response, None, None)
            }
            CompletionOutcome::HealedJson(HealedJsonResponse { response, parsed }) => {
                completion_result_from_response(response, Some(healing_data_from(parsed)), None)
            }
            CompletionOutcome::CoercedSchema(HealedSchemaResponse {
                response,
                parsed,
                coerced,
            }) => completion_result_from_response(
                response,
                Some(healing_data_from(parsed)),
                Some(healing_data_from(coerced)),
            ),
            CompletionOutcome::Stream(_) => {
                return Err(SimpleAgentsError::Config(
                    "streaming mode is not supported via sa_complete_messages_json".to_string(),
                ))
            }
        };

        serde_json::to_string(&payload)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize result: {e}")))
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
