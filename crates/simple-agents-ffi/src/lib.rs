//! C-compatible FFI bindings for SimpleAgents.

use futures_util::StreamExt;
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
use simple_agents_workflow::{
    run_email_workflow_yaml_file_with_client,
    run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options, YamlWorkflowEvent,
    YamlWorkflowEventSink, YamlWorkflowRunOptions,
};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
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

type SAStreamCallback =
    Option<extern "C" fn(event_json: *const c_char, user_data: *mut c_void) -> i32>;

type SAWorkflowEventCallback =
    Option<extern "C" fn(event_json: *const c_char, user_data: *mut c_void) -> i32>;

struct RecordingWorkflowEventSink {
    events: Mutex<Vec<YamlWorkflowEvent>>,
}

impl RecordingWorkflowEventSink {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    fn attach_to_output(&self, output: &mut JsonValue) -> Result<()> {
        let events = self
            .events
            .lock()
            .map_err(|_| {
                SimpleAgentsError::Config("workflow event sink lock poisoned".to_string())
            })?
            .clone();
        let events_value = serde_json::to_value(events)
            .map_err(|e| SimpleAgentsError::Config(format!("serialize workflow events: {e}")))?;
        if let JsonValue::Object(object) = output {
            object.insert("events".to_string(), events_value);
        }
        Ok(())
    }
}

impl YamlWorkflowEventSink for RecordingWorkflowEventSink {
    fn emit(&self, event: &YamlWorkflowEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event.clone());
        }
    }
}

struct CallbackWorkflowEventSink {
    callback: extern "C" fn(event_json: *const c_char, user_data: *mut c_void) -> i32,
    user_data: *mut c_void,
    callback_failed: Mutex<bool>,
}

impl CallbackWorkflowEventSink {
    fn new(
        callback: extern "C" fn(event_json: *const c_char, user_data: *mut c_void) -> i32,
        user_data: *mut c_void,
    ) -> Self {
        Self {
            callback,
            user_data,
            callback_failed: Mutex::new(false),
        }
    }

    fn callback_failed(&self) -> bool {
        self.callback_failed
            .lock()
            .map(|flag| *flag)
            .unwrap_or(true)
    }
}

// Safe because callback/user_data ownership belongs to the caller; this sink only forwards events.
unsafe impl Send for CallbackWorkflowEventSink {}
unsafe impl Sync for CallbackWorkflowEventSink {}

impl YamlWorkflowEventSink for CallbackWorkflowEventSink {
    fn emit(&self, event: &YamlWorkflowEvent) {
        let payload = match serde_json::to_string(event) {
            Ok(value) => value,
            Err(_) => {
                if let Ok(mut failed) = self.callback_failed.lock() {
                    *failed = true;
                }
                return;
            }
        };
        let payload = match CString::new(payload) {
            Ok(value) => value,
            Err(_) => {
                if let Ok(mut failed) = self.callback_failed.lock() {
                    *failed = true;
                }
                return;
            }
        };
        let status = (self.callback)(payload.as_ptr(), self.user_data);
        if status != 0 {
            if let Ok(mut failed) = self.callback_failed.lock() {
                *failed = true;
            }
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FfiStreamEvent {
    Chunk {
        chunk: simple_agent_type::response::CompletionChunk,
    },
    Error {
        message: String,
    },
    Done,
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

fn parse_workflow_run_options(raw_json: Option<String>) -> Result<YamlWorkflowRunOptions> {
    match raw_json {
        None => Ok(YamlWorkflowRunOptions::default()),
        Some(value) => {
            if value.trim().is_empty() {
                return Ok(YamlWorkflowRunOptions::default());
            }
            serde_json::from_str::<YamlWorkflowRunOptions>(&value).map_err(|error| {
                SimpleAgentsError::Config(format!(
                    "workflow_options_json must be valid JSON: {error}"
                ))
            })
        }
    }
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
    role.as_str().to_string()
}

fn finish_reason_to_string(finish_reason: FinishReason) -> String {
    finish_reason.as_str().to_string()
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

            let parsed_role = role.parse::<Role>().map_err(|_| {
                SimpleAgentsError::Config(format!(
                    "messages[{idx}].role must be one of user|assistant|system|tool"
                ))
            })?;

            let parsed = match parsed_role {
                Role::User => Message::user(content),
                Role::Assistant => Message::assistant(content),
                Role::System => Message::system(content),
                Role::Tool => {
                    let call_id = tool_call_id.ok_or_else(|| {
                        SimpleAgentsError::Config(format!(
                            "messages[{idx}].tool_call_id is required for tool role"
                        ))
                    })?;
                    Message::tool(content, call_id)
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
            set_last_error("Panic occurred in FFI call".to_string());
            -1
        }
    }
}

fn emit_stream_event(
    callback: extern "C" fn(*const c_char, *mut c_void) -> i32,
    user_data: *mut c_void,
    event: FfiStreamEvent,
) -> Result<()> {
    let payload = serde_json::to_string(&event)
        .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize stream event: {e}")))?;
    let payload = CString::new(payload).map_err(|_| {
        SimpleAgentsError::Config("stream event contains interior null byte".to_string())
    })?;

    let callback_status = callback(payload.as_ptr(), user_data);
    if callback_status == 0 {
        Ok(())
    } else {
        Err(SimpleAgentsError::Config(
            "stream cancelled by callback".to_string(),
        ))
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

/// Execute a message-based completion request in streaming mode and emit JSON events to a callback.
///
/// Returns `0` on success and non-zero on failure. On failure call `sa_last_error_message`.
///
/// # Safety
///
/// - `client` must be a pointer returned by `sa_client_new_from_env`.
/// - `model` must be a valid null-terminated C string.
/// - `messages` must point to `messages_len` valid `SAMessage` values.
/// - `callback` must point to a valid C function for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn sa_stream_messages(
    client: *mut SAClient,
    model: *const c_char,
    messages: *const SAMessage,
    messages_len: usize,
    max_tokens: i32,
    temperature: f32,
    top_p: f32,
    callback: SAStreamCallback,
    user_data: *mut c_void,
) -> i32 {
    if client.is_null() {
        set_last_error("client cannot be null".to_string());
        return -1;
    }

    let Some(callback) = callback else {
        set_last_error("callback cannot be null".to_string());
        return -1;
    };

    ffi_guard_status(|| {
        let model = cstr_to_string(model, "model")?;
        let messages = parse_messages(messages, messages_len)?;

        let mut builder = CompletionRequest::builder()
            .model(&model)
            .messages(messages);
        if max_tokens > 0 {
            builder = builder.max_tokens(max_tokens as u32);
        }
        if temperature >= 0.0 {
            builder = builder.temperature(temperature);
        }
        if top_p >= 0.0 {
            builder = builder.top_p(top_p);
        }
        builder = builder.stream(true);
        let request = builder.build()?;

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
        let mut stream = match outcome {
            CompletionOutcome::Stream(stream) => stream,
            CompletionOutcome::Response(_) => {
                return Err(SimpleAgentsError::Config(
                    "non-streaming response returned from sa_stream_messages".to_string(),
                ))
            }
            CompletionOutcome::HealedJson(_) => {
                return Err(SimpleAgentsError::Config(
                    "healed json response returned from sa_stream_messages".to_string(),
                ))
            }
            CompletionOutcome::CoercedSchema(_) => {
                return Err(SimpleAgentsError::Config(
                    "schema response returned from sa_stream_messages".to_string(),
                ))
            }
        };

        runtime.block_on(async {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        emit_stream_event(callback, user_data, FfiStreamEvent::Chunk { chunk })?;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        let _ = emit_stream_event(
                            callback,
                            user_data,
                            FfiStreamEvent::Error {
                                message: message.clone(),
                            },
                        );
                        return Err(error);
                    }
                }
            }

            emit_stream_event(callback, user_data, FfiStreamEvent::Done)
        })
    })
}

/// Execute workflow email YAML through the Rust workflow runner and return JSON output.
///
/// # Safety
///
/// - `client` must be a pointer returned by `sa_client_new_from_env`.
/// - `workflow_path` and `email_text` must be valid null-terminated UTF-8 strings.
/// - Returned string must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_run_email_workflow_yaml(
    client: *mut SAClient,
    workflow_path: *const c_char,
    email_text: *const c_char,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null".to_string());
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let workflow_path = cstr_to_string(workflow_path, "workflow_path")?;
        let email_text = cstr_to_string(email_text, "email_text")?;

        let client = &(*client).inner;
        let runtime = client
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let output = runtime
            .block_on(run_email_workflow_yaml_file_with_client(
                std::path::Path::new(workflow_path.as_str()),
                email_text.as_str(),
                &client.client,
            ))
            .map_err(|error| {
                SimpleAgentsError::Config(format!("failed to run workflow yaml: {error}"))
            })?;

        serde_json::to_string(&output)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize result: {e}")))
    })
}

/// Execute workflow YAML with arbitrary workflow input JSON and return JSON output.
///
/// # Safety
///
/// - `client` must be a pointer returned by `sa_client_new_from_env`.
/// - `workflow_path` and `workflow_input_json` must be valid null-terminated UTF-8 strings.
/// - `workflow_input_json` must be a valid JSON object string.
/// - Returned string must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_run_workflow_yaml(
    client: *mut SAClient,
    workflow_path: *const c_char,
    workflow_input_json: *const c_char,
) -> *mut c_char {
    sa_run_workflow_yaml_with_options(client, workflow_path, workflow_input_json, std::ptr::null())
}

/// Execute workflow YAML with arbitrary input JSON and optional telemetry options JSON.
///
/// `workflow_options_json` accepts a `YamlWorkflowRunOptions` JSON object and may be null.
///
/// # Safety
///
/// - `client` must be a pointer returned by `sa_client_new_from_env` and remain valid for the call.
/// - `workflow_path` and `workflow_input_json` must be valid null-terminated UTF-8 strings.
/// - `workflow_input_json` must be a valid JSON object string.
/// - `workflow_options_json` may be null; when non-null it must be valid null-terminated UTF-8 JSON.
/// - Returned string must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_run_workflow_yaml_with_options(
    client: *mut SAClient,
    workflow_path: *const c_char,
    workflow_input_json: *const c_char,
    workflow_options_json: *const c_char,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null".to_string());
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let workflow_path = cstr_to_string(workflow_path, "workflow_path")?;
        let workflow_input_json = cstr_to_string(workflow_input_json, "workflow_input_json")?;
        let workflow_options_json =
            cstr_to_optional_string(workflow_options_json, "workflow_options_json")?;
        let workflow_input: JsonValue =
            serde_json::from_str(&workflow_input_json).map_err(|e| {
                SimpleAgentsError::Config(format!("workflow_input_json must be valid JSON: {e}"))
            })?;
        if !workflow_input.is_object() {
            return Err(SimpleAgentsError::Config(
                "workflow_input_json must decode to a JSON object".to_string(),
            ));
        }
        let workflow_options = parse_workflow_run_options(workflow_options_json)?;

        let client = &(*client).inner;
        let runtime = client
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let output = runtime
            .block_on(
                run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
                    std::path::Path::new(workflow_path.as_str()),
                    &workflow_input,
                    &client.client,
                    None,
                    None,
                    &workflow_options,
                ),
            )
            .map_err(|error| {
                SimpleAgentsError::Config(format!("failed to run workflow yaml: {error}"))
            })?;

        serde_json::to_string(&output)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize result: {e}")))
    })
}

/// Execute workflow YAML and include collected workflow events in the JSON output under `events`.
///
/// # Safety
///
/// - `client` must be a pointer returned by `sa_client_new_from_env` and remain valid for the call.
/// - `workflow_path` and `workflow_input_json` must be valid null-terminated UTF-8 strings.
/// - `workflow_input_json` must decode to a JSON object.
/// - `workflow_options_json` may be null.
/// - Returned string must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_run_workflow_yaml_with_events(
    client: *mut SAClient,
    workflow_path: *const c_char,
    workflow_input_json: *const c_char,
    workflow_options_json: *const c_char,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null".to_string());
        return std::ptr::null_mut();
    }

    ffi_guard(|| {
        let workflow_path = cstr_to_string(workflow_path, "workflow_path")?;
        let workflow_input_json = cstr_to_string(workflow_input_json, "workflow_input_json")?;
        let workflow_options_json =
            cstr_to_optional_string(workflow_options_json, "workflow_options_json")?;
        let workflow_input: JsonValue =
            serde_json::from_str(&workflow_input_json).map_err(|e| {
                SimpleAgentsError::Config(format!("workflow_input_json must be valid JSON: {e}"))
            })?;
        if !workflow_input.is_object() {
            return Err(SimpleAgentsError::Config(
                "workflow_input_json must decode to a JSON object".to_string(),
            ));
        }
        let workflow_options = parse_workflow_run_options(workflow_options_json)?;

        let client = &(*client).inner;
        let runtime = client
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let event_sink = RecordingWorkflowEventSink::new();
        let output = runtime
            .block_on(
                run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
                    std::path::Path::new(workflow_path.as_str()),
                    &workflow_input,
                    &client.client,
                    None,
                    Some(&event_sink),
                    &workflow_options,
                ),
            )
            .map_err(|error| {
                SimpleAgentsError::Config(format!("failed to run workflow yaml: {error}"))
            })?;

        let mut output_value = serde_json::to_value(output)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize result: {e}")))?;
        event_sink.attach_to_output(&mut output_value)?;
        serde_json::to_string(&output_value)
            .map_err(|e| SimpleAgentsError::Config(format!("failed to serialize result: {e}")))
    })
}

/// Execute workflow YAML and emit live workflow events to a callback while returning final output.
///
/// # Safety
///
/// - `client` must be a pointer returned by `sa_client_new_from_env` and remain valid for the call.
/// - `workflow_path` and `workflow_input_json` must be valid null-terminated UTF-8 strings.
/// - `workflow_input_json` must decode to a JSON object.
/// - `workflow_options_json` may be null.
/// - `callback` must be non-null for the duration of the call.
/// - Returned string must be freed with `sa_string_free`.
#[no_mangle]
pub unsafe extern "C" fn sa_run_workflow_yaml_stream_events(
    client: *mut SAClient,
    workflow_path: *const c_char,
    workflow_input_json: *const c_char,
    workflow_options_json: *const c_char,
    callback: SAWorkflowEventCallback,
    user_data: *mut c_void,
) -> *mut c_char {
    if client.is_null() {
        set_last_error("client cannot be null".to_string());
        return std::ptr::null_mut();
    }

    let Some(callback) = callback else {
        set_last_error("callback cannot be null".to_string());
        return std::ptr::null_mut();
    };

    ffi_guard(|| {
        let workflow_path = cstr_to_string(workflow_path, "workflow_path")?;
        let workflow_input_json = cstr_to_string(workflow_input_json, "workflow_input_json")?;
        let workflow_options_json =
            cstr_to_optional_string(workflow_options_json, "workflow_options_json")?;
        let workflow_input: JsonValue =
            serde_json::from_str(&workflow_input_json).map_err(|e| {
                SimpleAgentsError::Config(format!("workflow_input_json must be valid JSON: {e}"))
            })?;
        if !workflow_input.is_object() {
            return Err(SimpleAgentsError::Config(
                "workflow_input_json must decode to a JSON object".to_string(),
            ));
        }
        let workflow_options = parse_workflow_run_options(workflow_options_json)?;

        let client = &(*client).inner;
        let runtime = client
            .runtime
            .lock()
            .map_err(|_| SimpleAgentsError::Config("runtime lock poisoned".to_string()))?;

        let event_sink = CallbackWorkflowEventSink::new(callback, user_data);
        let output = runtime
            .block_on(
                run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
                    std::path::Path::new(workflow_path.as_str()),
                    &workflow_input,
                    &client.client,
                    None,
                    Some(&event_sink),
                    &workflow_options,
                ),
            )
            .map_err(|error| {
                SimpleAgentsError::Config(format!("failed to run workflow yaml: {error}"))
            })?;

        if event_sink.callback_failed() {
            return Err(SimpleAgentsError::Config(
                "workflow event callback returned non-zero status or failed to serialize payload"
                    .to_string(),
            ));
        }

        serde_json::to_string(&output)
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
