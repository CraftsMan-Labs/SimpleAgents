//! Node.js bindings for SimpleAgents using napi-rs.

use futures_util::StreamExt;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
    ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi_derive::napi;
use serde::Serialize;
use serde_json::Value as JsonValue;
use simple_agent_type::coercion::CoercionFlag;
use simple_agent_type::message::{Message, Role};
use simple_agent_type::prelude::{
    ApiKey, CompletionRequest, Provider, Result as SaResult, SimpleAgentsError,
};
use simple_agent_type::response::{CompletionChunk, CompletionResponse, FinishReason, Usage};
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
    workflow_execution, YamlWorkflowEvent, YamlWorkflowEventSink, YamlWorkflowExecutionFlags,
    YamlWorkflowExecutionRequest, YamlWorkflowExecutorBinding, YamlWorkflowRunOptions,
    YamlWorkflowSource,
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod workflow_helpers;
use workflow_helpers::{
    build_workflow_input_with_messages_envelope, parse_workflow_options, validate_workflow_request,
};

type Runtime = tokio::runtime::Runtime;

fn provider_from_env(provider_name: &str) -> SaResult<Arc<dyn Provider>> {
    build_provider_arc(provider_name, None, None)
}

/// Builds a provider from explicit credentials and/or environment variables.
///
/// When `api_key` is `None`, provider-specific environment variables are used
/// (same as [`provider_from_env`]).
fn build_provider_arc(
    provider_name: &str,
    api_key: Option<&str>,
    api_base: Option<&str>,
) -> SaResult<Arc<dyn Provider>> {
    match provider_name {
        "openai" => {
            let provider = match api_key {
                Some(key) => {
                    let key = ApiKey::new(key)?;
                    match api_base {
                        Some(base) => OpenAIProvider::with_base_url(key, base.to_string())?,
                        None => OpenAIProvider::new(key)?,
                    }
                }
                None => OpenAIProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        "anthropic" => {
            let provider = match api_key {
                Some(key) => {
                    let key = ApiKey::new(key)?;
                    match api_base {
                        Some(base) => AnthropicProvider::with_base_url(key, base.to_string())?,
                        None => AnthropicProvider::new(key)?,
                    }
                }
                None => AnthropicProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        "openrouter" => {
            let provider = match api_key {
                Some(key) => {
                    let key = ApiKey::new(key)?;
                    match api_base {
                        Some(base) => OpenRouterProvider::with_base_url(key, base.to_string())?,
                        None => OpenRouterProvider::new(key)?,
                    }
                }
                None => OpenRouterProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        _ => Err(SimpleAgentsError::Config(format!(
            "Unknown provider '{provider_name}'"
        ))),
    }
}

fn napi_err(error: SimpleAgentsError) -> Error {
    Error::from_reason(error.to_string())
}

fn config_err(msg: impl Into<String>) -> SimpleAgentsError {
    SimpleAgentsError::Config(msg.into())
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

fn parse_schema_field(value: &JsonValue) -> SaResult<SchemaField> {
    let name = value
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| config_err("schema field missing `name`"))?;
    let schema_value = value
        .get("schema")
        .ok_or_else(|| config_err(format!("schema field `{name}` missing `schema`")))?;

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

fn parse_schema(value: &JsonValue) -> SaResult<Schema> {
    let kind = value
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| config_err("schema requires `kind`"))?
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
            let elements = value
                .get("elements")
                .ok_or_else(|| config_err("array schema requires `elements`"))?;
            Ok(Schema::array(parse_schema(elements)?))
        }
        "union" => {
            let variants = value
                .get("variants")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| config_err("union schema requires `variants` array"))?;
            let schemas = variants
                .iter()
                .map(parse_schema)
                .collect::<SaResult<Vec<_>>>()?;
            Ok(Schema::union(schemas))
        }
        "object" => {
            let fields = value
                .get("fields")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| config_err("object schema requires `fields` array"))?;
            let converted = fields
                .iter()
                .map(parse_schema_field)
                .collect::<SaResult<Vec<_>>>()?;
            Ok(Schema::Object(ObjectSchema {
                fields: converted,
                allow_additional_fields: value
                    .get("allow_additional_fields")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false),
            }))
        }
        other => Err(config_err(format!("unsupported schema kind `{other}`"))),
    }
}

fn completion_options(opts: &CompleteOptions) -> SaResult<CompletionOptions> {
    let mode_str = opts.mode.as_ref().map(|m| m.to_lowercase());
    let mode = match mode_str.as_deref() {
        None | Some("standard") => CompletionMode::Standard,
        Some("healed_json") => CompletionMode::HealedJson,
        Some("schema") => {
            let schema = opts
                .schema
                .as_ref()
                .ok_or_else(|| config_err("mode `schema` requires `schema` field"))?;
            CompletionMode::CoercedSchema(parse_schema(schema)?)
        }
        Some(other) => {
            return Err(config_err(format!(
                "unknown completion mode `{other}` (expected standard|healed_json|schema)"
            )))
        }
    };

    Ok(CompletionOptions { mode })
}

#[napi(object)]
pub struct ClientProviderConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub api_base: Option<String>,
}

#[napi(object)]
pub struct WorkflowYamlRunFlags {
    pub healing: bool,
    pub workflow_streaming: bool,
    pub node_llm_streaming: bool,
}

#[napi(object)]
pub struct ParsedWorkflowYamlExecutionRequest {
    pub workflow_path: String,
    #[napi(ts_type = "Record<string, unknown>")]
    pub workflow_input: JsonValue,
    pub healing: bool,
    pub workflow_streaming: bool,
    pub node_llm_streaming: bool,
    #[napi(ts_type = "Record<string, unknown>")]
    pub workflow_options: JsonValue,
}

#[napi(object)]
pub struct WorkflowYamlRunRequest {
    pub workflow_path: String,
    pub messages: Vec<MessageInput>,
    pub healing: bool,
    pub workflow_streaming: bool,
    pub node_llm_streaming: bool,
    /// When true, emit split thinking/output stream events (OR with env `SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW`).
    pub split_stream_deltas: Option<bool>,
    #[napi(ts_type = "Record<string, unknown>")]
    pub extra_workflow_input: Option<JsonValue>,
    #[napi(ts_type = "Record<string, unknown>")]
    pub workflow_options: Option<JsonValue>,
}

#[napi(object)]
#[derive(Default)]
pub struct CompleteOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stream: Option<bool>,
    /// "standard" | "healed_json" | "schema"
    pub mode: Option<String>,
    #[napi(ts_type = "unknown")]
    pub schema: Option<JsonValue>,
}

#[napi(object)]
pub struct MessageInput {
    #[napi(ts_type = "'system' | 'user' | 'assistant' | 'tool'")]
    pub role: String,
    pub content: String,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<Vec<JsToolCall>>,
}

#[napi(object)]
pub struct JsToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[napi(object)]
pub struct ToolCallResultFunction {
    pub name: String,
    pub arguments: String,
}

#[napi(object)]
pub struct ToolCallResult {
    pub id: String,
    pub tool_type: String,
    pub function: ToolCallResultFunction,
}

#[napi(object)]
pub struct JsToolCall {
    pub id: String,
    pub tool_type: String,
    pub function: JsToolCallFunction,
}

#[napi(object)]
pub struct CompletionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[napi(object)]
#[derive(Serialize)]
pub struct HealingData {
    #[napi(ts_type = "unknown")]
    pub value: Option<JsonValue>,
    pub flags: Vec<String>,
    pub confidence: f64,
}

#[napi(object)]
pub struct CompletionResult {
    pub id: String,
    pub model: String,
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallResult>>,
    pub finish_reason: Option<String>,
    pub usage: CompletionUsage,
    pub usage_available: bool,
    pub latency_ms: u32,
    pub raw: Option<String>,
    pub healed: Option<HealingData>,
    pub coerced: Option<HealingData>,
}

#[napi(object)]
#[derive(Serialize)]
pub struct StreamChunk {
    pub id: String,
    pub model: String,
    pub content: Option<String>,
    pub finish_reason: Option<String>,
    pub error: Option<String>,
    pub raw: Option<String>,
}

#[napi(object)]
#[derive(Serialize)]
pub struct StreamDelta {
    pub id: String,
    pub model: String,
    pub index: u32,
    pub role: Option<String>,
    pub content: Option<String>,
    pub finish_reason: Option<String>,
    pub raw: Option<String>,
}

#[napi(object)]
#[derive(Serialize)]
pub struct StreamErrorEvent {
    pub message: String,
}

#[napi(object)]
#[derive(Serialize)]
pub struct StreamEvent {
    pub event_type: String,
    pub delta: Option<StreamDelta>,
    pub error: Option<StreamErrorEvent>,
}

fn build_messages(input: Either<String, Vec<MessageInput>>) -> SaResult<Vec<Message>> {
    match input {
        Either::A(prompt) => {
            if prompt.is_empty() {
                return Err(SimpleAgentsError::Config(
                    "prompt cannot be empty".to_string(),
                ));
            }
            Ok(vec![Message::user(prompt)])
        }
        Either::B(messages) => {
            if messages.is_empty() {
                return Err(SimpleAgentsError::Config(
                    "messages cannot be empty".to_string(),
                ));
            }
            messages.into_iter().map(parse_message).collect()
        }
    }
}

pub(crate) fn parse_message(input: MessageInput) -> SaResult<Message> {
    let parsed_role = input.role.parse::<Role>().map_err(|_| {
        SimpleAgentsError::Config("role must be one of: user, assistant, system, tool".to_string())
    })?;

    let mut message = match parsed_role {
        Role::User => Message::user(input.content),
        Role::Assistant => {
            let mut msg = Message::assistant(input.content);
            if let Some(tool_calls) = input.tool_calls {
                let calls = tool_calls
                    .into_iter()
                    .map(ToolCall::from)
                    .collect::<Vec<_>>();
                if !calls.is_empty() {
                    msg = msg.with_tool_calls(calls);
                }
            }
            msg
        }
        Role::System => Message::system(input.content),
        Role::Tool => {
            let tool_call_id = input.tool_call_id.ok_or_else(|| {
                SimpleAgentsError::Config("tool role requires tool_call_id".to_string())
            })?;
            Message::tool(input.content, tool_call_id)
        }
    };

    if let Some(name) = input.name {
        if !name.is_empty() {
            message = message.with_name(name);
        }
    }

    Ok(message)
}

fn build_request(
    model: &str,
    messages: Vec<Message>,
    options: &CompleteOptions,
) -> SaResult<CompletionRequest> {
    if model.is_empty() {
        return Err(SimpleAgentsError::Config(
            "model cannot be empty".to_string(),
        ));
    }

    let mut builder = CompletionRequest::builder().model(model);
    for message in messages {
        builder = builder.message(message);
    }

    if let Some(max_tokens) = options.max_tokens {
        builder = builder.max_tokens(max_tokens);
    }
    if let Some(temperature) = options.temperature {
        builder = builder.temperature(temperature as f32);
    }
    if let Some(top_p) = options.top_p {
        builder = builder.top_p(top_p as f32);
    }
    if let Some(stream) = options.stream {
        builder = builder.stream(stream);
    }

    builder.build()
}

fn tool_type_to_str(tool_type: ToolType) -> &'static str {
    match tool_type {
        ToolType::Function => "function",
    }
}

fn finish_reason_to_str(finish_reason: FinishReason) -> &'static str {
    finish_reason.as_str()
}

fn role_to_str(role: Role) -> &'static str {
    role.as_str()
}

impl From<ToolCall> for ToolCallResult {
    fn from(value: ToolCall) -> Self {
        Self {
            id: value.id,
            tool_type: tool_type_to_str(value.tool_type).to_string(),
            function: ToolCallResultFunction {
                name: value.function.name,
                arguments: value.function.arguments,
            },
        }
    }
}

impl From<JsToolCall> for ToolCall {
    fn from(value: JsToolCall) -> Self {
        ToolCall {
            id: value.id,
            tool_type: ToolType::Function,
            function: simple_agent_type::tool::ToolCallFunction {
                name: value.function.name,
                arguments: value.function.arguments,
            },
        }
    }
}

impl From<Usage> for CompletionUsage {
    fn from(value: Usage) -> Self {
        Self {
            prompt_tokens: value.prompt_tokens,
            completion_tokens: value.completion_tokens,
            total_tokens: value.total_tokens,
        }
    }
}

impl CompletionResult {
    fn from_response(response: CompletionResponse, latency_ms: u64) -> Self {
        let choice = response.choices.first();
        let role = choice
            .map(|c| role_to_str(c.message.role).to_string())
            .unwrap_or_else(|| "assistant".to_string());
        let content = choice.map(|c| c.message.content.clone());
        let tool_calls = choice
            .and_then(|c| c.message.tool_calls.clone())
            .map(|calls| calls.into_iter().map(ToolCallResult::from).collect());
        let finish_reason = choice.map(|c| finish_reason_to_str(c.finish_reason).to_string());
        let usage = CompletionUsage::from(response.usage);
        let raw = serde_json::to_string(&response).ok();

        Self {
            id: response.id,
            model: response.model,
            role,
            content,
            tool_calls,
            finish_reason,
            usage,
            usage_available: true,
            latency_ms: latency_ms as u32,
            raw,
            healed: None,
            coerced: None,
        }
    }

    fn from_healed_json(healed: HealedJsonResponse, latency_ms: u64) -> Self {
        let mut base = Self::from_response(healed.response, latency_ms);
        base.healed = Some(healing_data(
            healed.parsed.value,
            healed.parsed.flags,
            healed.parsed.confidence,
        ));
        base
    }

    fn from_schema(healed: HealedSchemaResponse, latency_ms: u64) -> Self {
        let mut base = Self::from_response(healed.response, latency_ms);
        base.healed = Some(healing_data(
            healed.parsed.value,
            healed.parsed.flags,
            healed.parsed.confidence,
        ));
        base.coerced = Some(healing_data(
            healed.coerced.value,
            healed.coerced.flags,
            healed.coerced.confidence,
        ));
        base
    }
}

fn chunk_to_stream_chunk(chunk: CompletionChunk, error: Option<String>) -> StreamChunk {
    let raw = serde_json::to_string(&chunk).ok();
    let choice = chunk.choices.first();
    let content = choice.and_then(|c| c.delta.content.clone());
    let finish_reason = choice
        .and_then(|c| c.finish_reason)
        .map(|fr| finish_reason_to_str(fr).to_string());

    StreamChunk {
        id: chunk.id.clone(),
        model: chunk.model.clone(),
        content,
        finish_reason,
        error,
        raw,
    }
}

fn chunk_to_stream_delta(chunk: CompletionChunk) -> StreamDelta {
    let raw = serde_json::to_string(&chunk).ok();
    let choice = chunk.choices.first();
    let content = choice.and_then(|c| c.delta.content.clone());
    let finish_reason = choice
        .and_then(|c| c.finish_reason)
        .map(|fr| finish_reason_to_str(fr).to_string());
    let role = choice
        .and_then(|c| c.delta.role)
        .map(|role| role_to_str(role).to_string());
    let index = choice.map(|c| c.index).unwrap_or(0);

    StreamDelta {
        id: chunk.id,
        model: chunk.model,
        index,
        role,
        content,
        finish_reason,
        raw,
    }
}

fn flags_to_strings(flags: &[CoercionFlag]) -> Vec<String> {
    flags
        .iter()
        .map(|flag| serde_json::to_string(flag).unwrap_or_else(|_| flag.description()))
        .collect()
}

fn healing_data(value: JsonValue, flags: Vec<CoercionFlag>, confidence: f32) -> HealingData {
    HealingData {
        value: Some(value),
        flags: flags_to_strings(&flags),
        confidence: confidence as f64,
    }
}

pub struct CompleteTask {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
    request: CompletionRequest,
    completion_options: CompletionOptions,
}

impl Task for CompleteTask {
    type Output = CompletionResult;
    type JsValue = CompletionResult;

    fn compute(&mut self) -> Result<Self::Output> {
        let started = Instant::now();
        let outcome = self
            .runtime
            .block_on(
                self.client
                    .complete(&self.request, self.completion_options.clone()),
            )
            .map_err(napi_err)?;
        let latency_ms = started.elapsed().as_millis() as u64;
        match outcome {
            CompletionOutcome::Response(response) => {
                Ok(CompletionResult::from_response(response, latency_ms))
            }
            CompletionOutcome::Stream(_) => Err(Error::from_reason(
                "use stream() for streaming responses".to_string(),
            )),
            CompletionOutcome::HealedJson(healed) => {
                Ok(CompletionResult::from_healed_json(healed, latency_ms))
            }
            CompletionOutcome::CoercedSchema(healed) => {
                Ok(CompletionResult::from_schema(healed, latency_ms))
            }
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

pub struct StreamTask {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
    request: CompletionRequest,
    completion_options: CompletionOptions,
    on_chunk: ThreadsafeFunction<StreamChunk>,
}

pub struct StreamEventsTask {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
    request: CompletionRequest,
    completion_options: CompletionOptions,
    on_event: ThreadsafeFunction<StreamEvent>,
}

pub struct WorkflowStreamTask {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
    workflow_path: String,
    workflow_input: JsonValue,
    workflow_options: YamlWorkflowRunOptions,
    workflow_flags: YamlWorkflowExecutionFlags,
    on_event: ThreadsafeFunction<String>,
}

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
            .map_err(|_| Error::from_reason("workflow event sink lock poisoned".to_string()))?
            .clone();
        let events_value = serde_json::to_value(events)
            .map_err(|error| Error::from_reason(format!("failed to serialize events: {error}")))?;
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

struct NodeWorkflowEventSink {
    callback: ThreadsafeFunction<String>,
}

impl YamlWorkflowEventSink for NodeWorkflowEventSink {
    fn emit(&self, event: &YamlWorkflowEvent) {
        let payload = match serde_json::to_string(event) {
            Ok(value) => value,
            Err(_) => return,
        };
        self.callback
            .call(Ok(payload), ThreadsafeFunctionCallMode::Blocking);
    }
}

impl Task for StreamTask {
    type Output = CompletionResult;
    type JsValue = CompletionResult;

    fn compute(&mut self) -> Result<Self::Output> {
        let started = Instant::now();
        let outcome = self
            .runtime
            .block_on(
                self.client
                    .complete(&self.request, self.completion_options.clone()),
            )
            .map_err(napi_err)?;

        let mut aggregated = String::new();
        let mut response_id = String::new();
        let mut model = self.request.model.clone();
        match outcome {
            CompletionOutcome::Stream(mut stream) => {
                while let Some(item) = self.runtime.block_on(stream.next()) {
                    match item {
                        Ok(chunk) => {
                            if response_id.is_empty() {
                                response_id = chunk.id.clone();
                            }
                            if !chunk.model.is_empty() {
                                model = chunk.model.clone();
                            }
                            if let Some(ref content) =
                                chunk.choices.first().and_then(|c| c.delta.content.clone())
                            {
                                aggregated.push_str(content);
                            }
                            let payload = chunk_to_stream_chunk(chunk, None);
                            self.on_chunk
                                .call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
                        }
                        Err(e) => {
                            let payload = StreamChunk {
                                id: "error".to_string(),
                                model: "".to_string(),
                                content: None,
                                finish_reason: None,
                                error: Some(e.to_string()),
                                raw: None,
                            };
                            self.on_chunk
                                .call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
                            return Err(napi_err(e));
                        }
                    }
                }
            }
            CompletionOutcome::Response(response) => {
                return Ok(CompletionResult::from_response(
                    response,
                    started.elapsed().as_millis() as u64,
                ))
            }
            CompletionOutcome::HealedJson(_) => {
                return Err(Error::from_reason(
                    "healed JSON responses are not yet supported in Node bindings".to_string(),
                ))
            }
            CompletionOutcome::CoercedSchema(_) => {
                return Err(Error::from_reason(
                    "schema responses are not yet supported in Node bindings".to_string(),
                ))
            }
        }

        let latency_ms = started.elapsed().as_millis() as u64;
        let usage = CompletionUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };

        Ok(CompletionResult {
            id: response_id,
            model,
            role: "assistant".to_string(),
            content: Some(aggregated),
            tool_calls: None,
            finish_reason: None,
            usage,
            usage_available: false,
            latency_ms: latency_ms as u32,
            raw: None,
            healed: None,
            coerced: None,
        })
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

impl Task for StreamEventsTask {
    type Output = CompletionResult;
    type JsValue = CompletionResult;

    fn compute(&mut self) -> Result<Self::Output> {
        let started = Instant::now();
        let outcome = self
            .runtime
            .block_on(
                self.client
                    .complete(&self.request, self.completion_options.clone()),
            )
            .map_err(napi_err)?;

        let mut aggregated = String::new();
        let mut response_id = String::new();
        let mut model = self.request.model.clone();
        match outcome {
            CompletionOutcome::Stream(mut stream) => {
                while let Some(item) = self.runtime.block_on(stream.next()) {
                    match item {
                        Ok(chunk) => {
                            if response_id.is_empty() {
                                response_id = chunk.id.clone();
                            }
                            if !chunk.model.is_empty() {
                                model = chunk.model.clone();
                            }
                            if let Some(ref content) =
                                chunk.choices.first().and_then(|c| c.delta.content.clone())
                            {
                                aggregated.push_str(content);
                            }
                            let payload = StreamEvent {
                                event_type: "delta".to_string(),
                                delta: Some(chunk_to_stream_delta(chunk)),
                                error: None,
                            };
                            self.on_event
                                .call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
                        }
                        Err(e) => {
                            let payload = StreamEvent {
                                event_type: "error".to_string(),
                                delta: None,
                                error: Some(StreamErrorEvent {
                                    message: e.to_string(),
                                }),
                            };
                            self.on_event
                                .call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
                            return Err(napi_err(e));
                        }
                    }
                }

                self.on_event.call(
                    Ok(StreamEvent {
                        event_type: "done".to_string(),
                        delta: None,
                        error: None,
                    }),
                    ThreadsafeFunctionCallMode::NonBlocking,
                );
            }
            CompletionOutcome::Response(response) => {
                return Ok(CompletionResult::from_response(
                    response,
                    started.elapsed().as_millis() as u64,
                ))
            }
            CompletionOutcome::HealedJson(_) => {
                return Err(Error::from_reason(
                    "healed JSON responses are not yet supported in Node bindings".to_string(),
                ))
            }
            CompletionOutcome::CoercedSchema(_) => {
                return Err(Error::from_reason(
                    "schema responses are not yet supported in Node bindings".to_string(),
                ))
            }
        }

        let latency_ms = started.elapsed().as_millis() as u64;
        let usage = CompletionUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };

        Ok(CompletionResult {
            id: response_id,
            model,
            role: "assistant".to_string(),
            content: Some(aggregated),
            tool_calls: None,
            finish_reason: None,
            usage,
            usage_available: false,
            latency_ms: latency_ms as u32,
            raw: None,
            healed: None,
            coerced: None,
        })
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

impl Task for WorkflowStreamTask {
    type Output = JsonValue;
    type JsValue = napi::JsUnknown;

    fn compute(&mut self) -> Result<Self::Output> {
        let event_sink = NodeWorkflowEventSink {
            callback: self.on_event.clone(),
        };

        let request = YamlWorkflowExecutionRequest {
            source: YamlWorkflowSource::File(Path::new(self.workflow_path.as_str())),
            workflow_input: &self.workflow_input,
            executor: YamlWorkflowExecutorBinding::Client(self.client.as_ref()),
            custom_worker: None,
            options: &self.workflow_options,
            flags: self.workflow_flags,
        };

        let output = self
            .runtime
            .block_on(workflow_execution::stream(request, &event_sink))
            .map_err(|error| Error::from_reason(error.to_string()))?;

        serde_json::to_value(output)
            .map_err(|error| Error::from_reason(format!("failed to serialize output: {error}")))
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.to_js_value(&output)
    }
}

fn blocking_workflow_to_json(
    runtime: &Runtime,
    client: &Arc<SimpleAgentsClient>,
    workflow_path: &str,
    workflow_input: &JsonValue,
    options: &YamlWorkflowRunOptions,
    flags: YamlWorkflowExecutionFlags,
    event_sink: Option<&dyn YamlWorkflowEventSink>,
) -> Result<JsonValue> {
    let request = YamlWorkflowExecutionRequest {
        source: YamlWorkflowSource::File(Path::new(workflow_path)),
        workflow_input,
        executor: YamlWorkflowExecutorBinding::Client(client.as_ref()),
        custom_worker: None,
        options,
        flags,
    };

    let output = if let Some(sink) = event_sink {
        runtime.block_on(workflow_execution::stream(request, sink))
    } else {
        runtime.block_on(workflow_execution::run(request))
    }
    .map_err(|error| Error::from_reason(error.to_string()))?;

    serde_json::to_value(output)
        .map_err(|error| Error::from_reason(format!("failed to serialize output: {error}")))
}

#[napi(
    js_name = "parseWorkflowYamlExecutionRequest",
    ts_args_type = "workflowPath: string, messages: Array<MessageInput>, flags: WorkflowYamlRunFlags, extraWorkflowInput?: Record<string, unknown>, workflowOptions?: Record<string, unknown>"
)]
pub fn parse_workflow_yaml_execution_request(
    workflow_path: String,
    messages: Vec<MessageInput>,
    flags: WorkflowYamlRunFlags,
    extra_workflow_input: Option<JsonValue>,
    workflow_options: Option<JsonValue>,
) -> Result<ParsedWorkflowYamlExecutionRequest> {
    if workflow_path.trim().is_empty() {
        return Err(Error::from_reason(
            "workflow_path cannot be empty".to_string(),
        ));
    }
    let workflow_input =
        build_workflow_input_with_messages_envelope(messages, extra_workflow_input.as_ref())?;
    validate_workflow_request(workflow_path.as_str(), &workflow_input)?;
    let opts = parse_workflow_options(workflow_options.clone())?;
    let workflow_options_value = serde_json::to_value(&opts).map_err(|error| {
        Error::from_reason(format!("failed to serialize workflow options: {error}"))
    })?;
    Ok(ParsedWorkflowYamlExecutionRequest {
        workflow_path,
        workflow_input,
        healing: flags.healing,
        workflow_streaming: flags.workflow_streaming,
        node_llm_streaming: flags.node_llm_streaming,
        workflow_options: workflow_options_value,
    })
}

#[napi]
pub struct Client {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
}

#[napi]
impl Client {
    #[napi(constructor)]
    pub fn new(provider: String) -> Result<Self> {
        let provider = provider_from_env(&provider).map_err(napi_err)?;
        Self::from_provider_arc(provider)
    }

    #[napi(factory)]
    pub fn with_provider_config(config: ClientProviderConfig) -> Result<Self> {
        let provider = build_provider_arc(
            &config.provider,
            config.api_key.as_deref(),
            config.api_base.as_deref(),
        )
        .map_err(napi_err)?;
        Self::from_provider_arc(provider)
    }

    fn from_provider_arc(provider: Arc<dyn Provider>) -> Result<Self> {
        let client = Arc::new(
            SimpleAgentsClientBuilder::new()
                .with_provider(provider)
                .build()
                .map_err(napi_err)?,
        );
        let runtime = Arc::new(Runtime::new().map_err(|e| Error::from_reason(e.to_string()))?);
        Ok(Self { runtime, client })
    }

    #[napi(
        ts_args_type = "model: string, promptOrMessages: string | MessageInput[], options?: CompleteOptions",
        ts_return_type = "Promise<CompletionResult>"
    )]
    pub fn complete(
        &self,
        model: String,
        prompt_or_messages: Either<String, Vec<MessageInput>>,
        options: Option<CompleteOptions>,
    ) -> Result<AsyncTask<CompleteTask>> {
        let opts = options.unwrap_or_default();
        let completion_options = completion_options(&opts).map_err(napi_err)?;
        let messages = build_messages(prompt_or_messages).map_err(napi_err)?;
        let request = build_request(&model, messages, &opts).map_err(napi_err)?;
        let task = CompleteTask {
            runtime: self.runtime.clone(),
            client: self.client.clone(),
            request,
            completion_options,
        };
        Ok(AsyncTask::new(task))
    }

    #[napi(
        ts_args_type = "model: string, promptOrMessages: string | MessageInput[], onChunk: (chunk: StreamChunk) => void, options?: CompleteOptions",
        ts_return_type = "Promise<CompletionResult>"
    )]
    pub fn stream(
        &self,
        model: String,
        prompt_or_messages: Either<String, Vec<MessageInput>>,
        on_chunk: JsFunction,
        options: Option<CompleteOptions>,
    ) -> Result<AsyncTask<StreamTask>> {
        let messages = build_messages(prompt_or_messages).map_err(napi_err)?;
        let mut opts = options.unwrap_or_default();
        opts.stream = Some(true);
        let completion_options = completion_options(&opts).map_err(napi_err)?;
        if !matches!(completion_options.mode, CompletionMode::Standard) {
            return Err(Error::from_reason(
                "healed_json and schema modes are not supported with stream() yet".to_string(),
            ));
        }
        let request = build_request(&model, messages, &opts).map_err(napi_err)?;

        let tsfn: ThreadsafeFunction<StreamChunk> =
            on_chunk.create_threadsafe_function(0, |ctx: ThreadSafeCallContext<StreamChunk>| {
                ctx.env.to_js_value(&ctx.value).map(|v| vec![v])
            })?;

        let task = StreamTask {
            runtime: self.runtime.clone(),
            client: self.client.clone(),
            request,
            completion_options,
            on_chunk: tsfn,
        };

        Ok(AsyncTask::new(task))
    }

    #[napi(
        ts_args_type = "model: string, promptOrMessages: string | MessageInput[], onEvent: (event: StreamEvent) => void, options?: CompleteOptions",
        ts_return_type = "Promise<CompletionResult>"
    )]
    pub fn stream_events(
        &self,
        model: String,
        prompt_or_messages: Either<String, Vec<MessageInput>>,
        on_event: JsFunction,
        options: Option<CompleteOptions>,
    ) -> Result<AsyncTask<StreamEventsTask>> {
        let messages = build_messages(prompt_or_messages).map_err(napi_err)?;
        let mut opts = options.unwrap_or_default();
        opts.stream = Some(true);
        let completion_options = completion_options(&opts).map_err(napi_err)?;
        if !matches!(completion_options.mode, CompletionMode::Standard) {
            return Err(Error::from_reason(
                "healed_json and schema modes are not supported with stream_events() yet"
                    .to_string(),
            ));
        }
        let request = build_request(&model, messages, &opts).map_err(napi_err)?;

        let tsfn: ThreadsafeFunction<StreamEvent> =
            on_event.create_threadsafe_function(0, |ctx: ThreadSafeCallContext<StreamEvent>| {
                ctx.env.to_js_value(&ctx.value).map(|v| vec![v])
            })?;

        let task = StreamEventsTask {
            runtime: self.runtime.clone(),
            client: self.client.clone(),
            request,
            completion_options,
            on_event: tsfn,
        };

        Ok(AsyncTask::new(task))
    }

    #[napi(
        ts_args_type = "workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }",
        ts_return_type = "{ workflow_id: string; entry_node: string; trace: Array<string>; outputs: Record<string, unknown>; terminal_node: string; terminal_output?: unknown; step_timings: Array<unknown>; llm_node_metrics: Record<string, unknown>; llm_node_models: Record<string, string>; total_elapsed_ms: number; ttft_ms?: number; total_input_tokens: number; total_output_tokens: number; total_tokens: number; total_reasoning_tokens?: number; tokens_per_second: number; trace_id?: string; metadata?: unknown; events?: Array<unknown> }"
    )]
    pub fn run_workflow_yaml(
        &self,
        workflow_path: String,
        workflow_input: JsonValue,
    ) -> Result<JsonValue> {
        self.run_workflow_yaml_with_options(workflow_path, workflow_input, None)
    }

    #[napi(
        ts_args_type = "workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown> }",
        ts_return_type = "{ workflow_id: string; entry_node: string; trace: Array<string>; outputs: Record<string, unknown>; terminal_node: string; terminal_output?: unknown; step_timings: Array<unknown>; llm_node_metrics: Record<string, unknown>; llm_node_models: Record<string, string>; total_elapsed_ms: number; ttft_ms?: number; total_input_tokens: number; total_output_tokens: number; total_tokens: number; total_reasoning_tokens?: number; tokens_per_second: number; trace_id?: string; metadata?: unknown; events?: Array<unknown> }"
    )]
    pub fn run_workflow_yaml_with_events(
        &self,
        workflow_path: String,
        workflow_input: JsonValue,
        workflow_options: Option<JsonValue>,
    ) -> Result<JsonValue> {
        validate_workflow_request(workflow_path.as_str(), &workflow_input)?;
        let options = parse_workflow_options(workflow_options)?;

        let event_sink = RecordingWorkflowEventSink::new();
        let stream_flags = YamlWorkflowExecutionFlags {
            workflow_streaming: true,
            ..YamlWorkflowExecutionFlags::default()
        };
        let mut output_value = blocking_workflow_to_json(
            &self.runtime,
            &self.client,
            workflow_path.as_str(),
            &workflow_input,
            &options,
            stream_flags,
            Some(&event_sink),
        )?;
        event_sink.attach_to_output(&mut output_value)?;
        Ok(output_value)
    }

    #[napi(
        ts_args_type = "workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, onEvent: (err: unknown, eventJson: string) => void, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown> }",
        ts_return_type = "Promise<{ workflow_id: string; entry_node: string; trace: Array<string>; outputs: Record<string, unknown>; terminal_node: string; terminal_output?: unknown; step_timings: Array<unknown>; llm_node_metrics: Record<string, unknown>; llm_node_models: Record<string, string>; total_elapsed_ms: number; ttft_ms?: number; total_input_tokens: number; total_output_tokens: number; total_tokens: number; total_reasoning_tokens?: number; tokens_per_second: number; trace_id?: string; metadata?: unknown; events?: Array<unknown> }>"
    )]
    pub fn run_workflow_yaml_stream(
        &self,
        workflow_path: String,
        workflow_input: JsonValue,
        on_event: JsFunction,
        workflow_options: Option<JsonValue>,
    ) -> Result<AsyncTask<WorkflowStreamTask>> {
        validate_workflow_request(workflow_path.as_str(), &workflow_input)?;
        let options = parse_workflow_options(workflow_options)?;

        let tsfn: ThreadsafeFunction<String> =
            on_event.create_threadsafe_function(0, |ctx: ThreadSafeCallContext<String>| {
                let event_json = ctx.env.create_string_from_std(ctx.value)?.into_unknown();
                Ok(vec![event_json])
            })?;

        let task = WorkflowStreamTask {
            runtime: self.runtime.clone(),
            client: self.client.clone(),
            workflow_path,
            workflow_input,
            workflow_options: options,
            workflow_flags: YamlWorkflowExecutionFlags {
                workflow_streaming: true,
                ..YamlWorkflowExecutionFlags::default()
            },
            on_event: tsfn,
        };

        Ok(AsyncTask::new(task))
    }

    #[napi(
        ts_args_type = "workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown> }",
        ts_return_type = "{ workflow_id: string; entry_node: string; trace: Array<string>; outputs: Record<string, unknown>; terminal_node: string; terminal_output?: unknown; step_timings: Array<unknown>; llm_node_metrics: Record<string, unknown>; llm_node_models: Record<string, string>; total_elapsed_ms: number; ttft_ms?: number; total_input_tokens: number; total_output_tokens: number; total_tokens: number; total_reasoning_tokens?: number; tokens_per_second: number; trace_id?: string; metadata?: unknown; events?: Array<unknown> }"
    )]
    pub fn run_workflow_yaml_with_options(
        &self,
        workflow_path: String,
        workflow_input: JsonValue,
        workflow_options: Option<JsonValue>,
    ) -> Result<JsonValue> {
        validate_workflow_request(workflow_path.as_str(), &workflow_input)?;
        let options = parse_workflow_options(workflow_options)?;
        blocking_workflow_to_json(
            &self.runtime,
            &self.client,
            workflow_path.as_str(),
            &workflow_input,
            &options,
            YamlWorkflowExecutionFlags::default(),
            None,
        )
    }

    #[napi(
        ts_args_type = "request: WorkflowYamlRunRequest",
        ts_return_type = "{ workflow_id: string; entry_node: string; trace: Array<string>; outputs: Record<string, unknown>; terminal_node: string; terminal_output?: unknown; step_timings: Array<unknown>; llm_node_metrics: Record<string, unknown>; llm_node_models: Record<string, string>; total_elapsed_ms: number; ttft_ms?: number; total_input_tokens: number; total_output_tokens: number; total_tokens: number; total_reasoning_tokens?: number; tokens_per_second: number; trace_id?: string; metadata?: unknown; events?: Array<unknown> }"
    )]
    pub fn execute_workflow_yaml(&self, request: WorkflowYamlRunRequest) -> Result<JsonValue> {
        let workflow_input = build_workflow_input_with_messages_envelope(
            request.messages,
            request.extra_workflow_input.as_ref(),
        )?;
        validate_workflow_request(request.workflow_path.as_str(), &workflow_input)?;
        let options = parse_workflow_options(request.workflow_options.clone())?;
        let flags = YamlWorkflowExecutionFlags {
            healing: request.healing,
            workflow_streaming: request.workflow_streaming,
            node_llm_streaming: request.node_llm_streaming,
            split_stream_deltas: request.split_stream_deltas.unwrap_or(false),
        };
        blocking_workflow_to_json(
            &self.runtime,
            &self.client,
            request.workflow_path.as_str(),
            &workflow_input,
            &options,
            flags,
            None,
        )
    }

    #[napi(
        ts_args_type = "request: WorkflowYamlRunRequest, onEvent: (err: unknown, eventJson: string) => void",
        ts_return_type = "Promise<{ workflow_id: string; entry_node: string; trace: Array<string>; outputs: Record<string, unknown>; terminal_node: string; terminal_output?: unknown; step_timings: Array<unknown>; llm_node_metrics: Record<string, unknown>; llm_node_models: Record<string, string>; total_elapsed_ms: number; ttft_ms?: number; total_input_tokens: number; total_output_tokens: number; total_tokens: number; total_reasoning_tokens?: number; tokens_per_second: number; trace_id?: string; metadata?: unknown; events?: Array<unknown> }>"
    )]
    pub fn execute_workflow_yaml_stream(
        &self,
        request: WorkflowYamlRunRequest,
        on_event: JsFunction,
    ) -> Result<AsyncTask<WorkflowStreamTask>> {
        let workflow_input = build_workflow_input_with_messages_envelope(
            request.messages,
            request.extra_workflow_input.as_ref(),
        )?;
        validate_workflow_request(request.workflow_path.as_str(), &workflow_input)?;
        let options = parse_workflow_options(request.workflow_options.clone())?;
        let workflow_flags = YamlWorkflowExecutionFlags {
            healing: request.healing,
            workflow_streaming: request.workflow_streaming,
            node_llm_streaming: request.node_llm_streaming,
            split_stream_deltas: request.split_stream_deltas.unwrap_or(false),
        };

        let tsfn: ThreadsafeFunction<String> =
            on_event.create_threadsafe_function(0, |ctx: ThreadSafeCallContext<String>| {
                let event_json = ctx.env.create_string_from_std(ctx.value)?.into_unknown();
                Ok(vec![event_json])
            })?;

        let task = WorkflowStreamTask {
            runtime: self.runtime.clone(),
            client: self.client.clone(),
            workflow_path: request.workflow_path,
            workflow_input,
            workflow_options: options,
            workflow_flags,
            on_event: tsfn,
        };

        Ok(AsyncTask::new(task))
    }
}
