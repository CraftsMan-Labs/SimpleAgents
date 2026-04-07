//! Node.js bindings for SimpleAgents using napi-rs.

use futures_util::StreamExt;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
    ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::JsObject;
use napi_derive::napi;
use serde::Serialize;
use serde_json::Value as JsonValue;
use simple_agent_type::coercion::CoercionFlag;
use simple_agent_type::message::{ContentPart, Message, MessageContent, Role};
use simple_agent_type::prelude::{
    ApiKey, CompletionRequest, Provider, Result as SaResult, SimpleAgentsError,
};
use simple_agent_type::response::{CompletionChunk, CompletionResponse, FinishReason, Usage};
use simple_agent_type::tool::{ToolCall, ToolType};
use simple_agents_core::{
    CompletionMode, CompletionOptions, CompletionOutcome, HealedJsonResponse, HealedSchemaResponse,
    SimpleAgentsClient,
};
use simple_agents_healing::{
    schema::{Field as SchemaField, ObjectSchema, Schema},
    CoercionEngine, JsonishParser,
};
use simple_agents_providers::openai::OpenAiCompatProvider;
use simple_agents_workflow::yaml_runner::{
    validate_custom_worker_executor_for_file, workflow_execution, YamlWorkflowCustomWorkerExecutor,
    YamlWorkflowEvent, YamlWorkflowEventSink, YamlWorkflowExecutionFlags,
    YamlWorkflowExecutionRequest, YamlWorkflowExecutorBinding, YamlWorkflowRunOptions,
    YamlWorkflowSource,
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

mod workflow_custom_worker;
mod workflow_helpers;
mod workflow_options_napi;
use workflow_helpers::{
    apply_workflow_execution_flags_patch, build_workflow_input_with_messages_envelope,
    normalize_workflow_input_messages, parse_workflow_execution_flags_patch,
    parse_workflow_options, parse_workflow_request_options, validate_workflow_request,
};
pub use workflow_options_napi::{
    WorkflowRunOptionsNapi, WorkflowTelemetryConfigNapi, WorkflowTraceConfigNapi,
    WorkflowTraceContextNapi, WorkflowTraceTenantNapi,
};

type Runtime = tokio::runtime::Runtime;

type ClientOptsFromJs = (
    Option<JsonValue>,
    Option<Arc<dyn YamlWorkflowCustomWorkerExecutor>>,
);

fn client_opts_from_js_object(opts: Option<&JsObject>) -> Result<ClientOptsFromJs> {
    let Some(opts) = opts else {
        return Ok((None, None));
    };
    let workflow_options: Option<JsonValue> = opts.get("workflowOptions")?;
    let custom_worker = match opts.get("customWorker")? {
        Some(f) => Some(workflow_custom_worker::build_executor(&f)?),
        None => None,
    };
    Ok((workflow_options, custom_worker))
}

fn build_provider_arc(
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> SaResult<Arc<dyn Provider>> {
    let provider = match api_key {
        Some(key) => {
            let key = ApiKey::new(key)?;
            match base_url {
                Some(base) => OpenAiCompatProvider::with_base_url(key, base.to_string())?,
                None => OpenAiCompatProvider::new(key)?,
            }
        }
        None => OpenAiCompatProvider::from_env()?,
    };
    Ok(Arc::new(provider))
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

// ---------------------------------------------------------------------------
// NAPI object types
// ---------------------------------------------------------------------------

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
    pub split_stream_deltas: Option<bool>,
    #[napi(ts_type = "Record<string, unknown>")]
    pub extra_workflow_input: Option<JsonValue>,
    pub workflow_options: Option<WorkflowRunOptionsNapi>,
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
    pub send_schema: Option<bool>,
}

#[napi(object)]
pub struct ContentPartInput {
    #[napi(ts_type = "'text' | 'image' | 'audio' | 'video'")]
    pub r#type: String,
    pub text: Option<String>,
    pub media_type: Option<String>,
    pub data: Option<String>,
}

#[napi(object)]
pub struct MessageInput {
    #[napi(ts_type = "'system' | 'user' | 'assistant' | 'tool'")]
    pub role: String,
    #[napi(ts_type = "string | Array<ContentPartInput>")]
    pub content: Either<String, Vec<ContentPartInput>>,
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
    #[napi(ts_type = "unknown")]
    pub snapshot: Option<JsonValue>,
    pub snapshot_confidence: Option<f64>,
    #[napi(ts_type = "unknown")]
    pub coerced_snapshot: Option<JsonValue>,
    pub coerced_confidence: Option<f64>,
    pub is_complete: Option<bool>,
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

// ---------------------------------------------------------------------------
// Message / request builders
// ---------------------------------------------------------------------------

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

fn message_content_from_parts(parts: Vec<ContentPartInput>) -> SaResult<MessageContent> {
    if parts.is_empty() {
        return Err(SimpleAgentsError::Config(
            "content parts cannot be empty".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(parts.len());
    for p in parts {
        match p.r#type.as_str() {
            "text" => {
                let t = p.text.ok_or_else(|| {
                    SimpleAgentsError::Config("text part requires `text`".to_string())
                })?;
                out.push(ContentPart::text(t));
            }
            "image" => {
                let mt = p.media_type.ok_or_else(|| {
                    SimpleAgentsError::Config("image part requires `media_type`".to_string())
                })?;
                let d = p.data.ok_or_else(|| {
                    SimpleAgentsError::Config("image part requires `data`".to_string())
                })?;
                out.push(ContentPart::image(mt, d));
            }
            "audio" => {
                let mt = p.media_type.ok_or_else(|| {
                    SimpleAgentsError::Config("audio part requires `media_type`".to_string())
                })?;
                let d = p.data.ok_or_else(|| {
                    SimpleAgentsError::Config("audio part requires `data`".to_string())
                })?;
                out.push(ContentPart::audio(mt, d));
            }
            "video" => {
                let mt = p.media_type.ok_or_else(|| {
                    SimpleAgentsError::Config("video part requires `media_type`".to_string())
                })?;
                let d = p.data.ok_or_else(|| {
                    SimpleAgentsError::Config("video part requires `data`".to_string())
                })?;
                out.push(ContentPart::video(mt, d));
            }
            other => {
                return Err(SimpleAgentsError::Config(format!(
                    "unknown content part type `{other}` (expected: text|image|audio|video)"
                )));
            }
        }
    }
    Ok(MessageContent::Parts(out))
}

pub(crate) fn parse_message(input: MessageInput) -> SaResult<Message> {
    let MessageInput {
        role,
        content,
        name,
        tool_call_id,
        tool_calls,
    } = input;

    let parsed_role = role.parse::<Role>().map_err(|_| {
        SimpleAgentsError::Config("role must be one of: user, assistant, system, tool".to_string())
    })?;

    let message_content: MessageContent = match content {
        Either::A(s) => {
            if s.is_empty() {
                return Err(SimpleAgentsError::Config(
                    "content cannot be empty".to_string(),
                ));
            }
            MessageContent::Text(s)
        }
        Either::B(parts) => message_content_from_parts(parts)?,
    };

    let mut message = match parsed_role {
        Role::User => Message {
            role: Role::User,
            content: message_content,
            name: None,
            tool_call_id: None,
            tool_calls: None,
        },
        Role::Assistant => {
            let mut msg = Message {
                role: Role::Assistant,
                content: message_content,
                name: None,
                tool_call_id: None,
                tool_calls: None,
            };
            if let Some(calls_in) = tool_calls {
                let calls = calls_in.into_iter().map(ToolCall::from).collect::<Vec<_>>();
                if !calls.is_empty() {
                    msg = msg.with_tool_calls(calls);
                }
            }
            msg
        }
        Role::System => Message {
            role: Role::System,
            content: message_content,
            name: None,
            tool_call_id: None,
            tool_calls: None,
        },
        Role::Tool => {
            let call_id = tool_call_id.ok_or_else(|| {
                SimpleAgentsError::Config("tool role requires tool_call_id".to_string())
            })?;
            Message {
                role: Role::Tool,
                content: message_content,
                name: None,
                tool_call_id: Some(call_id),
                tool_calls: None,
            }
        }
    };

    if let Some(n) = name {
        if !n.is_empty() {
            message = message.with_name(n);
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

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

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
        let content = choice.map(|c| c.message.content_text().to_string());
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
        snapshot: None,
        snapshot_confidence: None,
        coerced_snapshot: None,
        coerced_confidence: None,
        is_complete: Some(false),
        error,
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

// ---------------------------------------------------------------------------
// Async tasks
// ---------------------------------------------------------------------------

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
                let mode = self.completion_options.mode.clone();
                let mut last_healed: Option<simple_agents_healing::ParserResult> = None;
                let mut last_coerced: Option<
                    simple_agent_type::coercion::CoercionResult<JsonValue>,
                > = None;
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
                            let mut payload = chunk_to_stream_chunk(chunk.clone(), None);
                            if matches!(
                                mode,
                                CompletionMode::HealedJson | CompletionMode::CoercedSchema(_)
                            ) {
                                if let Ok(parsed) = JsonishParser::new().parse(aggregated.as_str())
                                {
                                    payload.snapshot = Some(parsed.value.clone());
                                    payload.snapshot_confidence = Some(parsed.confidence as f64);
                                    last_healed = Some(parsed.clone());
                                    if let CompletionMode::CoercedSchema(schema) = &mode {
                                        if let Ok(coerced) =
                                            CoercionEngine::new().coerce(&parsed.value, schema)
                                        {
                                            payload.coerced_snapshot = Some(coerced.value.clone());
                                            payload.coerced_confidence =
                                                Some(coerced.confidence as f64);
                                            last_coerced = Some(coerced);
                                        }
                                    }
                                }
                            }
                            self.on_chunk
                                .call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
                        }
                        Err(e) => {
                            let payload = StreamChunk {
                                id: "error".to_string(),
                                model: "".to_string(),
                                content: None,
                                finish_reason: None,
                                snapshot: None,
                                snapshot_confidence: None,
                                coerced_snapshot: None,
                                coerced_confidence: None,
                                is_complete: Some(true),
                                error: Some(e.to_string()),
                                raw: None,
                            };
                            self.on_chunk
                                .call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
                            return Err(napi_err(e));
                        }
                    }
                }
                // Emit a final completion marker with best-known snapshots.
                let final_payload = StreamChunk {
                    id: if response_id.is_empty() {
                        "final".to_string()
                    } else {
                        response_id.clone()
                    },
                    model: model.clone(),
                    content: None,
                    finish_reason: Some("stop".to_string()),
                    snapshot: last_healed.as_ref().map(|parsed| parsed.value.clone()),
                    snapshot_confidence: last_healed
                        .as_ref()
                        .map(|parsed| parsed.confidence as f64),
                    coerced_snapshot: last_coerced.as_ref().map(|coerced| coerced.value.clone()),
                    coerced_confidence: last_coerced
                        .as_ref()
                        .map(|coerced| coerced.confidence as f64),
                    is_complete: Some(true),
                    error: None,
                    raw: None,
                };
                self.on_chunk
                    .call(Ok(final_payload), ThreadsafeFunctionCallMode::NonBlocking);

                let healed_data = last_healed
                    .map(|parsed| healing_data(parsed.value, parsed.flags, parsed.confidence));
                let coerced_data = last_coerced
                    .map(|coerced| healing_data(coerced.value, coerced.flags, coerced.confidence));

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
                    healed: healed_data,
                    coerced: coerced_data,
                })
            }
            CompletionOutcome::Response(response) => Ok(CompletionResult::from_response(
                response,
                started.elapsed().as_millis() as u64,
            )),
            CompletionOutcome::HealedJson(_) | CompletionOutcome::CoercedSchema(_) => Err(
                Error::from_reason("unexpected non-streaming outcome".to_string()),
            ),
        }
        // all match arms return
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

pub struct WorkflowStreamTask {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
    workflow_path: String,
    workflow_input: JsonValue,
    workflow_options: YamlWorkflowRunOptions,
    workflow_flags: YamlWorkflowExecutionFlags,
    include_events: bool,
    on_event: ThreadsafeFunction<String>,
    custom_worker: Option<Arc<dyn YamlWorkflowCustomWorkerExecutor>>,
}

/// Runs `blocking_workflow_to_json` off the JavaScript main thread so
/// [`ThreadsafeFunction`] callbacks for [`YamlWorkflowCustomWorkerExecutor`]
/// can execute (main thread must not be stuck in `Runtime::block_on`).
pub struct RunWorkflowTask {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
    workflow_path: String,
    workflow_input: JsonValue,
    workflow_options: YamlWorkflowRunOptions,
    custom_worker: Arc<dyn YamlWorkflowCustomWorkerExecutor>,
    /// When true, matches legacy `runWorkflow` with `include_events: true` (record + attach).
    record_events: bool,
}

impl Task for RunWorkflowTask {
    type Output = JsonValue;
    type JsValue = napi::JsUnknown;

    fn compute(&mut self) -> Result<Self::Output> {
        if self.record_events {
            let event_sink = RecordingWorkflowEventSink::new();
            let stream_flags = YamlWorkflowExecutionFlags {
                workflow_streaming: true,
                ..YamlWorkflowExecutionFlags::default()
            };
            let mut output_value = blocking_workflow_to_json(BlockingWorkflowParams {
                runtime: &self.runtime,
                client: &self.client,
                workflow_path: self.workflow_path.as_str(),
                workflow_input: &self.workflow_input,
                options: &self.workflow_options,
                flags: stream_flags,
                event_sink: Some(&event_sink),
                custom_worker: Some(self.custom_worker.as_ref()),
            })?;
            event_sink.attach_to_output(&mut output_value)?;
            Ok(output_value)
        } else {
            blocking_workflow_to_json(BlockingWorkflowParams {
                runtime: &self.runtime,
                client: &self.client,
                workflow_path: self.workflow_path.as_str(),
                workflow_input: &self.workflow_input,
                options: &self.workflow_options,
                flags: YamlWorkflowExecutionFlags::default(),
                event_sink: None,
                custom_worker: Some(self.custom_worker.as_ref()),
            })
        }
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.to_js_value(&output)
    }
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

struct NodeCombinedWorkflowEventSink {
    events: Mutex<Vec<YamlWorkflowEvent>>,
    callback: ThreadsafeFunction<String>,
}

impl NodeCombinedWorkflowEventSink {
    fn new(callback: ThreadsafeFunction<String>) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            callback,
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

impl YamlWorkflowEventSink for NodeCombinedWorkflowEventSink {
    fn emit(&self, event: &YamlWorkflowEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event.clone());
        }
        let payload = match serde_json::to_string(event) {
            Ok(value) => value,
            Err(_) => return,
        };
        self.callback
            .call(Ok(payload), ThreadsafeFunctionCallMode::Blocking);
    }
}

impl Task for WorkflowStreamTask {
    type Output = JsonValue;
    type JsValue = napi::JsUnknown;

    fn compute(&mut self) -> Result<Self::Output> {
        if self.include_events {
            let event_sink = NodeCombinedWorkflowEventSink::new(self.on_event.clone());
            let request = YamlWorkflowExecutionRequest {
                source: YamlWorkflowSource::File(Path::new(self.workflow_path.as_str())),
                workflow_input: &self.workflow_input,
                executor: YamlWorkflowExecutorBinding::Client(self.client.as_ref()),
                custom_worker: self.custom_worker.as_deref(),
                options: &self.workflow_options,
                flags: self.workflow_flags,
            };
            let output = self
                .runtime
                .block_on(workflow_execution::stream(request, &event_sink))
                .map_err(|error| Error::from_reason(error.to_string()))?;
            let mut value = serde_json::to_value(output).map_err(|error| {
                Error::from_reason(format!("failed to serialize output: {error}"))
            })?;
            event_sink.attach_to_output(&mut value)?;
            Ok(value)
        } else {
            let event_sink = NodeWorkflowEventSink {
                callback: self.on_event.clone(),
            };
            let request = YamlWorkflowExecutionRequest {
                source: YamlWorkflowSource::File(Path::new(self.workflow_path.as_str())),
                workflow_input: &self.workflow_input,
                executor: YamlWorkflowExecutorBinding::Client(self.client.as_ref()),
                custom_worker: self.custom_worker.as_deref(),
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
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.to_js_value(&output)
    }
}

struct BlockingWorkflowParams<'a> {
    runtime: &'a Runtime,
    client: &'a Arc<SimpleAgentsClient>,
    workflow_path: &'a str,
    workflow_input: &'a JsonValue,
    options: &'a YamlWorkflowRunOptions,
    flags: YamlWorkflowExecutionFlags,
    event_sink: Option<&'a dyn YamlWorkflowEventSink>,
    custom_worker: Option<&'a dyn YamlWorkflowCustomWorkerExecutor>,
}

fn blocking_workflow_to_json(p: BlockingWorkflowParams<'_>) -> Result<JsonValue> {
    let request = YamlWorkflowExecutionRequest {
        source: YamlWorkflowSource::File(Path::new(p.workflow_path)),
        workflow_input: p.workflow_input,
        executor: YamlWorkflowExecutorBinding::Client(p.client.as_ref()),
        custom_worker: p.custom_worker,
        options: p.options,
        flags: p.flags,
    };

    let output = if let Some(sink) = p.event_sink {
        p.runtime
            .block_on(workflow_execution::stream(request, sink))
    } else {
        p.runtime.block_on(workflow_execution::run(request))
    }
    .map_err(|error| Error::from_reason(error.to_string()))?;

    serde_json::to_value(output)
        .map_err(|error| Error::from_reason(format!("failed to serialize output: {error}")))
}

// ---------------------------------------------------------------------------
// Top-level helpers
// ---------------------------------------------------------------------------

#[napi(
    js_name = "parseWorkflowYamlExecutionRequest",
    ts_args_type = "workflowPath: string, messages: Array<MessageInput>, flags: WorkflowYamlRunFlags, extraWorkflowInput?: Record<string, unknown>, workflowOptions?: WorkflowRunOptionsNapi"
)]
pub fn parse_workflow_yaml_execution_request(
    workflow_path: String,
    messages: Vec<MessageInput>,
    flags: WorkflowYamlRunFlags,
    extra_workflow_input: Option<JsonValue>,
    workflow_options: Option<WorkflowRunOptionsNapi>,
) -> Result<ParsedWorkflowYamlExecutionRequest> {
    if workflow_path.trim().is_empty() {
        return Err(Error::from_reason(
            "workflow_path cannot be empty".to_string(),
        ));
    }
    let workflow_input =
        build_workflow_input_with_messages_envelope(messages, extra_workflow_input.as_ref())?;
    validate_workflow_request(workflow_path.as_str(), &workflow_input)?;
    let opts_json = workflow_options_napi::workflow_run_options_napi_to_json(workflow_options)?;
    let opts = parse_workflow_options(opts_json)?;
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

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

#[napi]
pub struct Client {
    runtime: Arc<Runtime>,
    client: Arc<SimpleAgentsClient>,
}

#[napi]
impl Client {
    /// Create a client from an API key.
    ///
    /// Uses `OpenAiCompatProvider` under the hood; pass `baseUrl` to override
    /// the endpoint.
    #[napi(constructor)]
    pub fn new(api_key: String, base_url: Option<String>) -> Result<Self> {
        let provider =
            build_provider_arc(Some(api_key.as_str()), base_url.as_deref()).map_err(napi_err)?;
        let client = Arc::new(SimpleAgentsClient::new(provider));
        let runtime = Arc::new(Runtime::new().map_err(|e| Error::from_reason(e.to_string()))?);
        Ok(Self { runtime, client })
    }

    /// Create a client using environment variables for the API key.
    #[napi(factory)]
    pub fn from_env() -> Result<Self> {
        let provider = build_provider_arc(None, None).map_err(napi_err)?;
        let client = Arc::new(SimpleAgentsClient::new(provider));
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
        js_name = "streamComplete",
        ts_args_type = "model: string, promptOrMessages: string | MessageInput[], onChunk: (chunk: StreamChunk) => void, options?: CompleteOptions",
        ts_return_type = "Promise<CompletionResult>"
    )]
    pub fn stream_complete(
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
        js_name = "runWorkflow",
        ts_args_type = "workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown>; include_events?: boolean }, workflowExecution?: { healing?: boolean; workflowStreaming?: boolean; nodeLlmStreaming?: boolean; splitStreamDeltas?: boolean }, customWorkerDispatch?: (req: { handler: string; handlerFile?: string; payload: unknown; context: unknown }) => unknown",
        ts_return_type = "Record<string, unknown> | Promise<Record<string, unknown>>"
    )]
    pub fn run_workflow(
        &self,
        workflow_path: String,
        workflow_input: JsonValue,
        workflow_options: Option<JsonValue>,
        workflow_execution: Option<JsonValue>,
        custom_worker_dispatch: Option<JsFunction>,
    ) -> Result<Either<JsonValue, AsyncTask<RunWorkflowTask>>> {
        let execution_patch = parse_workflow_execution_flags_patch(workflow_execution)?;
        let workflow_input = normalize_workflow_input_messages(&workflow_input)?;
        validate_workflow_request(workflow_path.as_str(), &workflow_input)?;
        let request_options = parse_workflow_request_options(workflow_options)?;
        let custom_worker = match custom_worker_dispatch {
            Some(ref f) => Some(workflow_custom_worker::build_executor(f)?),
            None => None,
        };
        if let Some(cw) = custom_worker {
            return Ok(Either::B(AsyncTask::new(RunWorkflowTask {
                runtime: self.runtime.clone(),
                client: self.client.clone(),
                workflow_path,
                workflow_input,
                workflow_options: request_options.run_options,
                custom_worker: cw,
                record_events: request_options.include_events,
            })));
        }
        let cw = None;
        if request_options.include_events {
            let event_sink = RecordingWorkflowEventSink::new();
            let stream_flags = apply_workflow_execution_flags_patch(
                YamlWorkflowExecutionFlags {
                    workflow_streaming: true,
                    ..YamlWorkflowExecutionFlags::default()
                },
                &execution_patch,
            );
            let mut output_value = blocking_workflow_to_json(BlockingWorkflowParams {
                runtime: &self.runtime,
                client: &self.client,
                workflow_path: workflow_path.as_str(),
                workflow_input: &workflow_input,
                options: &request_options.run_options,
                flags: stream_flags,
                event_sink: Some(&event_sink),
                custom_worker: cw,
            })?;
            event_sink.attach_to_output(&mut output_value)?;
            Ok(Either::A(output_value))
        } else {
            Ok(Either::A(blocking_workflow_to_json(
                BlockingWorkflowParams {
                    runtime: &self.runtime,
                    client: &self.client,
                    workflow_path: workflow_path.as_str(),
                    workflow_input: &workflow_input,
                    options: &request_options.run_options,
                    flags: apply_workflow_execution_flags_patch(
                        YamlWorkflowExecutionFlags::default(),
                        &execution_patch,
                    ),
                    event_sink: None,
                    custom_worker: cw,
                },
            )?))
        }
    }

    #[napi(
        js_name = "streamWorkflow",
        ts_args_type = "workflowPath: string, workflowInput: { messages?: MessageInput[]; [key: string]: unknown }, onEvent: (err: unknown, eventJson: string) => void, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown>; include_events?: boolean }, workflowExecution?: { healing?: boolean; workflowStreaming?: boolean; nodeLlmStreaming?: boolean; splitStreamDeltas?: boolean }, customWorkerDispatch?: (req: { handler: string; handlerFile?: string; payload: unknown; context: unknown }) => unknown",
        ts_return_type = "Promise<Record<string, unknown>>"
    )]
    pub fn stream_workflow(
        &self,
        workflow_path: String,
        workflow_input: JsonValue,
        on_event: JsFunction,
        workflow_options: Option<JsonValue>,
        workflow_execution: Option<JsonValue>,
        custom_worker_dispatch: Option<JsFunction>,
    ) -> Result<AsyncTask<WorkflowStreamTask>> {
        let execution_patch = parse_workflow_execution_flags_patch(workflow_execution)?;
        let workflow_input = normalize_workflow_input_messages(&workflow_input)?;
        validate_workflow_request(workflow_path.as_str(), &workflow_input)?;
        let request_options = parse_workflow_request_options(workflow_options)?;

        let custom_worker = match custom_worker_dispatch {
            Some(f) => Some(workflow_custom_worker::build_executor(&f)?),
            None => None,
        };

        if custom_worker.is_none() {
            validate_custom_worker_executor_for_file(Path::new(workflow_path.as_str()), None)
                .map_err(|error| Error::from_reason(error.to_string()))?;
        }

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
            workflow_options: request_options.run_options,
            workflow_flags: apply_workflow_execution_flags_patch(
                YamlWorkflowExecutionFlags {
                    workflow_streaming: true,
                    ..YamlWorkflowExecutionFlags::default()
                },
                &execution_patch,
            ),
            include_events: request_options.include_events,
            on_event: tsfn,
            custom_worker,
        };

        Ok(AsyncTask::new(task))
    }

    /// Resume a workflow from a checkpoint.
    ///
    /// ```ts
    /// const result = await client.resume(checkpoint);
    /// ```
    #[napi(
        js_name = "resume",
        ts_args_type = "checkpoint: Record<string, unknown>, opts?: { workflowOptions?: Record<string, unknown>; customWorker?: (req: { handler: string; handlerFile?: string; payload: unknown; context: unknown }) => unknown }",
        ts_return_type = "Record<string, unknown> | Promise<Record<string, unknown>>"
    )]
    pub fn resume(
        &self,
        checkpoint: JsonValue,
        opts: Option<JsObject>,
    ) -> Result<Either<JsonValue, AsyncTask<RunWorkflowTask>>> {
        let workflow_path = checkpoint
            .get("workflow_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::from_reason("checkpoint must have workflow_path".to_string()))?
            .to_string();

        let messages_val = checkpoint
            .get("original_messages")
            .cloned()
            .unwrap_or(serde_json::json!([]));

        let workflow_input = serde_json::json!({ "messages": messages_val });
        let (workflow_options, custom_worker) = client_opts_from_js_object(opts.as_ref())?;
        let request_options = parse_workflow_request_options(workflow_options)?;

        if let Some(cw) = custom_worker {
            return Ok(Either::B(AsyncTask::new(RunWorkflowTask {
                runtime: self.runtime.clone(),
                client: self.client.clone(),
                workflow_path,
                workflow_input,
                workflow_options: request_options.run_options,
                custom_worker: cw,
                record_events: false,
            })));
        }

        Ok(Either::A(blocking_workflow_to_json(
            BlockingWorkflowParams {
                runtime: &self.runtime,
                client: &self.client,
                workflow_path: workflow_path.as_str(),
                workflow_input: &workflow_input,
                options: &request_options.run_options,
                flags: YamlWorkflowExecutionFlags::default(),
                event_sink: None,
                custom_worker: None,
            },
        )?))
    }
}

/// Copies OTLP-related variables into the Rust process environment (`std::env`).
///
/// Bun and some Node setups update `process.env` in JavaScript without updating the
/// OS environment that `std::env::var` reads in native code. Call this after setting
/// `SIMPLE_AGENTS_TRACING_ENABLED` / `OTEL_EXPORTER_OTLP_*` in JS and **before** the
/// first workflow run so workflow tracing initializes with OTLP enabled.
#[napi(js_name = "syncOtelEnvFromProcess")]
pub fn sync_otel_env_from_process(
    tracing_enabled: String,
    otlp_protocol: String,
    otlp_endpoint: String,
    otlp_headers: String,
    otel_service_name: Option<String>,
) {
    std::env::set_var("SIMPLE_AGENTS_TRACING_ENABLED", tracing_enabled);
    std::env::set_var("OTEL_EXPORTER_OTLP_PROTOCOL", otlp_protocol);
    std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", otlp_endpoint);
    std::env::set_var("OTEL_EXPORTER_OTLP_HEADERS", otlp_headers);
    if let Some(name) = otel_service_name {
        if !name.is_empty() {
            std::env::set_var("OTEL_SERVICE_NAME", name);
        }
    }
}
