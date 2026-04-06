use js_sys::{Array, Function, Object, Promise, Reflect};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfig {
    api_key: String,
    base_url: Option<String>,
    headers: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MessageInput {
    role: String,
    /// Plain string or multimodal parts array (OpenAI-compatible JSON).
    content: JsonValue,
    name: Option<String>,
    #[serde(alias = "tool_call_id")]
    tool_call_id: Option<String>,
    #[serde(alias = "tool_calls")]
    tool_calls: Option<Vec<JsToolCall>>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsToolCall {
    id: String,
    #[serde(alias = "type")]
    tool_type: Option<String>,
    function: JsToolCallFunction,
}

#[derive(Deserialize, Serialize, Clone)]
struct JsToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize, Clone)]
struct OpenAiMessageInput {
    role: String,
    /// Plain string or multimodal parts array.
    content: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Serialize, Clone)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiToolCallFunction,
}

#[derive(Serialize, Clone)]
struct OpenAiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct CompleteOptions {
    max_tokens: Option<u32>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    mode: Option<String>,
}

#[derive(Deserialize)]
struct WorkflowDoc {
    model: Option<String>,
    steps: Vec<WorkflowStep>,
}

#[derive(Deserialize, Clone)]
struct GraphWorkflowDoc {
    model: Option<String>,
    entry_node: String,
    nodes: Vec<GraphWorkflowNode>,
    edges: Option<Vec<GraphWorkflowEdge>>,
}

#[derive(Deserialize, Clone)]
struct GraphWorkflowEdge {
    from: String,
    to: String,
}

#[derive(Deserialize, Clone)]
struct GraphWorkflowNode {
    id: String,
    node_type: GraphNodeType,
    config: Option<GraphNodeConfig>,
}

#[derive(Deserialize, Clone)]
struct GraphNodeConfig {
    prompt: Option<String>,
    payload: Option<JsonValue>,
    output_schema: Option<JsonValue>,
}

#[derive(Deserialize, Clone, Default)]
struct GraphNodeType {
    llm_call: Option<GraphLlmCall>,
    switch: Option<GraphSwitch>,
    custom_worker: Option<GraphCustomWorker>,
}

#[derive(Deserialize, Clone)]
struct GraphLlmCall {
    model: Option<String>,
    temperature: Option<f64>,
    messages_path: Option<String>,
    append_prompt_as_user: Option<bool>,
}

#[derive(Deserialize, Clone)]
struct GraphSwitch {
    branches: Option<Vec<GraphSwitchBranch>>,
    default: Option<String>,
}

#[derive(Deserialize, Clone)]
struct GraphSwitchBranch {
    condition: Option<String>,
    target: Option<String>,
}

#[derive(Deserialize, Clone)]
struct GraphCustomWorker {
    handler: Option<String>,
    handler_file: Option<String>,
}

#[derive(Deserialize, Clone)]
struct WorkflowStep {
    id: String,
    #[serde(rename = "type")]
    step_type: String,
    key: Option<String>,
    value: Option<JsonValue>,
    prompt: Option<String>,
    condition: Option<WorkflowCondition>,
    then: Option<String>,
    r#else: Option<String>,
    text: Option<String>,
    function: Option<String>,
    args: Option<JsonValue>,
    next: Option<String>,
    model: Option<String>,
    temperature: Option<f64>,
}

#[derive(Deserialize, Clone)]
struct WorkflowCondition {
    left: JsonValue,
    operator: String,
    right: JsonValue,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallFunctionOut {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallOut {
    id: String,
    tool_type: String,
    function: ToolCallFunctionOut,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionResult {
    id: String,
    model: String,
    role: String,
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallOut>>,
    finish_reason: Option<String>,
    usage: CompletionUsage,
    usage_available: bool,
    latency_ms: u32,
    raw: Option<String>,
    healed: Option<JsonValue>,
    coerced: Option<JsonValue>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowRunEvent {
    step_id: String,
    step_type: String,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowRunResult {
    status: String,
    context: JsonValue,
    output: Option<JsonValue>,
    events: Vec<WorkflowRunEvent>,
    #[serde(rename = "workflow_id", skip_serializing_if = "Option::is_none")]
    workflow_id: Option<String>,
    #[serde(rename = "entry_node", skip_serializing_if = "Option::is_none")]
    entry_node: Option<String>,
    #[serde(rename = "email_text", skip_serializing_if = "Option::is_none")]
    email_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<JsonValue>,
    #[serde(rename = "terminal_node", skip_serializing_if = "Option::is_none")]
    terminal_node: Option<String>,
    #[serde(rename = "terminal_output", skip_serializing_if = "Option::is_none")]
    terminal_output: Option<JsonValue>,
    #[serde(rename = "step_timings", skip_serializing_if = "Option::is_none")]
    step_timings: Option<JsonValue>,
    #[serde(rename = "total_elapsed_ms", skip_serializing_if = "Option::is_none")]
    total_elapsed_ms: Option<u64>,
    #[serde(rename = "ttft_ms", skip_serializing_if = "Option::is_none")]
    ttft_ms: Option<u64>,
    #[serde(rename = "total_input_tokens", skip_serializing_if = "Option::is_none")]
    total_input_tokens: Option<u64>,
    #[serde(rename = "total_output_tokens", skip_serializing_if = "Option::is_none")]
    total_output_tokens: Option<u64>,
    #[serde(rename = "total_tokens", skip_serializing_if = "Option::is_none")]
    total_tokens: Option<u64>,
    #[serde(rename = "total_reasoning_tokens", skip_serializing_if = "Option::is_none")]
    total_reasoning_tokens: Option<u64>,
    #[serde(rename = "tokens_per_second", skip_serializing_if = "Option::is_none")]
    tokens_per_second: Option<f64>,
    #[serde(rename = "trace_id", skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<JsonValue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowRunOptions {
    #[allow(dead_code)]
    telemetry: Option<JsonValue>,
    #[allow(dead_code)]
    trace: Option<JsonValue>,
    #[serde(skip)]
    functions_js: Option<JsValue>,
}

fn js_error(message: impl Into<String>) -> JsValue {
    js_sys::Error::new(&format!(
        "simple-agents-wasm runtime error: {}",
        message.into()
    ))
    .into()
}

fn config_error(message: impl Into<String>) -> JsValue {
    js_sys::Error::new(&format!(
        "simple-agents-wasm config error: {}",
        message.into()
    ))
    .into()
}

fn now_millis() -> f64 {
    let global = js_sys::global();
    let performance = Reflect::get(&global, &JsValue::from_str("performance")).ok();
    if let Some(perf) = performance {
        if let Ok(now) = Reflect::get(&perf, &JsValue::from_str("now")) {
            if let Some(now_fn) = now.dyn_ref::<Function>() {
                if let Ok(v) = now_fn.call0(&perf) {
                    return v.as_f64().unwrap_or(0.0);
                }
            }
        }
    }
    0.0
}

fn to_messages(prompt_or_messages: JsValue) -> Result<Vec<MessageInput>, JsValue> {
    if let Some(prompt) = prompt_or_messages.as_string() {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return Err(config_error("prompt cannot be empty"));
        }
        return Ok(vec![MessageInput {
            role: "user".to_string(),
            content: JsonValue::String(trimmed.to_string()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }]);
    }

    let messages: Vec<MessageInput> = serde_wasm_bindgen::from_value(prompt_or_messages)
        .map_err(|_| config_error("messages must be a non-empty array"))?;
    if messages.is_empty() {
        return Err(config_error("messages must be a non-empty array"));
    }
    Ok(messages)
}

/// Reads `media_type` or `mediaType` from a JSON object (JS often uses camelCase).
fn media_type_field(obj: &JsonMap<String, JsonValue>) -> Option<&str> {
    obj.get("media_type")
        .or_else(|| obj.get("mediaType"))
        .and_then(|v| v.as_str())
}

/// Converts simplified multimodal parts (`type`: image|audio|video + data) to OpenAI chat
/// wire format. Passes through strings, native OpenAI parts, and unknown objects unchanged.
fn normalize_message_content(content: JsonValue) -> JsonValue {
    match content {
        JsonValue::Array(parts) => JsonValue::Array(
            parts.into_iter().map(normalize_content_part).collect(),
        ),
        other => other,
    }
}

fn normalize_content_part(part: JsonValue) -> JsonValue {
    let Some(obj) = part.as_object() else {
        return part;
    };
    let ty = obj.get("type").and_then(|v| v.as_str());
    match ty {
        Some("image") => {
            let mt = media_type_field(obj).unwrap_or("");
            let data = obj.get("data").and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{mt};base64,{data}") }
            })
        }
        Some("audio") => {
            let mt = media_type_field(obj).unwrap_or("");
            let data = obj.get("data").and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "type": "input_audio",
                "input_audio": { "media_type": mt, "data": data }
            })
        }
        Some("video") => {
            let mt = media_type_field(obj).unwrap_or("");
            let data = obj.get("data").and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "type": "video",
                "video": { "media_type": mt, "data": data }
            })
        }
        _ => part,
    }
}

fn to_openai_messages(messages: Vec<MessageInput>) -> Vec<OpenAiMessageInput> {
    messages
        .into_iter()
        .map(|message| OpenAiMessageInput {
            role: message.role,
            content: normalize_message_content(message.content),
            name: message.name,
            tool_call_id: message.tool_call_id,
            tool_calls: message.tool_calls.map(|tool_calls| {
                tool_calls
                    .into_iter()
                    .map(|tool_call| OpenAiToolCall {
                        id: tool_call.id,
                        tool_type: tool_call
                            .tool_type
                            .unwrap_or_else(|| "function".to_string()),
                        function: OpenAiToolCallFunction {
                            name: tool_call.function.name,
                            arguments: tool_call.function.arguments,
                        },
                    })
                    .collect()
            }),
        })
        .collect()
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn default_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("https://api.openai.com/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        _ => None,
    }
}

async fn call_method0(target: &JsValue, method: &str) -> Result<JsValue, JsValue> {
    let method_value = Reflect::get(target, &JsValue::from_str(method))
        .map_err(|_| js_error(format!("missing method: {method}")))?;
    let method_fn = method_value
        .dyn_into::<Function>()
        .map_err(|_| js_error(format!("method is not callable: {method}")))?;
    let out = method_fn
        .call0(target)
        .map_err(|_| js_error(format!("failed to call method: {method}")))?;
    let promise = out
        .dyn_into::<Promise>()
        .map_err(|_| js_error(format!("method did not return Promise: {method}")))?;
    JsFuture::from(promise)
        .await
        .map_err(|_| js_error(format!("await failed for method: {method}")))
}

fn call_method0_sync(target: &JsValue, method: &str) -> Result<JsValue, JsValue> {
    let method_value = Reflect::get(target, &JsValue::from_str(method))
        .map_err(|_| js_error(format!("missing method: {method}")))?;
    let method_fn = method_value
        .dyn_into::<Function>()
        .map_err(|_| js_error(format!("method is not callable: {method}")))?;
    method_fn
        .call0(target)
        .map_err(|_| js_error(format!("failed to call method: {method}")))
}

fn call_method2_sync(
    target: &JsValue,
    method: &str,
    arg1: &JsValue,
    arg2: &JsValue,
) -> Result<JsValue, JsValue> {
    let method_value = Reflect::get(target, &JsValue::from_str(method))
        .map_err(|_| js_error(format!("missing method: {method}")))?;
    let method_fn = method_value
        .dyn_into::<Function>()
        .map_err(|_| js_error(format!("method is not callable: {method}")))?;
    method_fn
        .call2(target, arg1, arg2)
        .map_err(|_| js_error(format!("failed to call method: {method}")))
}

async fn js_fetch(
    url: &str,
    body: &JsonValue,
    headers: &HashMap<String, String>,
) -> Result<JsValue, JsValue> {
    let global = js_sys::global();
    let fetch_value = Reflect::get(&global, &JsValue::from_str("fetch"))
        .map_err(|_| js_error("global fetch is unavailable"))?;
    let fetch_fn = fetch_value
        .dyn_into::<Function>()
        .map_err(|_| js_error("global fetch is not callable"))?;

    let options = Object::new();
    Reflect::set(
        &options,
        &JsValue::from_str("method"),
        &JsValue::from_str("POST"),
    )
    .map_err(|_| js_error("failed to set request method"))?;

    let headers_obj = Object::new();
    Reflect::set(
        &headers_obj,
        &JsValue::from_str("Content-Type"),
        &JsValue::from_str("application/json"),
    )
    .map_err(|_| js_error("failed to set content-type header"))?;
    for (key, value) in headers {
        Reflect::set(
            &headers_obj,
            &JsValue::from_str(key),
            &JsValue::from_str(value),
        )
        .map_err(|_| js_error("failed to set custom header"))?;
    }

    Reflect::set(&options, &JsValue::from_str("headers"), &headers_obj)
        .map_err(|_| js_error("failed to set request headers"))?;
    let body_str =
        serde_json::to_string(body).map_err(|_| js_error("failed to serialize request body"))?;
    Reflect::set(
        &options,
        &JsValue::from_str("body"),
        &JsValue::from_str(&body_str),
    )
    .map_err(|_| js_error("failed to set request body"))?;

    let response_promise = fetch_fn
        .call2(&global, &JsValue::from_str(url), &options)
        .map_err(|_| js_error("fetch call failed"))?
        .dyn_into::<Promise>()
        .map_err(|_| js_error("fetch did not return Promise"))?;
    JsFuture::from(response_promise)
        .await
        .map_err(|_| js_error("fetch await failed"))
}

fn interpolate_string(input: &str, context: &JsonMap<String, JsonValue>) -> String {
    let mut output = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("}}") {
            let key = after_start[..end].trim();
            let replacement = context
                .get(key)
                .map(|value| match value {
                    JsonValue::String(s) => s.clone(),
                    _ => serde_json::to_string(value).unwrap_or_default(),
                })
                .unwrap_or_default();
            output.push_str(&replacement);
            rest = &after_start[end + 2..];
        } else {
            output.push_str(&rest[start..]);
            rest = "";
        }
    }

    output.push_str(rest);
    output
}

fn interpolate_json(value: &JsonValue, context: &JsonMap<String, JsonValue>) -> JsonValue {
    match value {
        JsonValue::String(s) => JsonValue::String(interpolate_string(s, context)),
        JsonValue::Array(arr) => JsonValue::Array(
            arr.iter()
                .map(|item| interpolate_json(item, context))
                .collect(),
        ),
        JsonValue::Object(obj) => {
            let mapped = obj
                .iter()
                .map(|(k, v)| (k.clone(), interpolate_json(v, context)))
                .collect::<JsonMap<String, JsonValue>>();
            JsonValue::Object(mapped)
        }
        _ => value.clone(),
    }
}

fn evaluate_condition(condition: &WorkflowCondition, context: &JsonMap<String, JsonValue>) -> bool {
    let left = interpolate_json(&condition.left, context);
    let right = interpolate_json(&condition.right, context);

    match condition.operator.as_str() {
        "eq" => left == right,
        "ne" => left != right,
        "contains" => {
            let l = match left {
                JsonValue::String(s) => s,
                _ => serde_json::to_string(&left).unwrap_or_default(),
            };
            let r = match right {
                JsonValue::String(s) => s,
                _ => serde_json::to_string(&right).unwrap_or_default(),
            };
            l.contains(&r)
        }
        _ => false,
    }
}

fn get_path_value<'a>(root: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    let normalized = path.trim().strip_prefix("$.").unwrap_or(path.trim());
    let mut current = root;
    for token in normalized.split('.') {
        if token.is_empty() {
            continue;
        }
        current = current.get(token)?;
    }
    Some(current)
}

fn interpolate_graph_prompt(input: &str, context: &JsonValue) -> String {
    let mut output = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("}}") {
            let key = after_start[..end].trim();
            let replacement = get_path_value(context, key)
                .map(|value| match value {
                    JsonValue::String(s) => s.clone(),
                    _ => serde_json::to_string(value).unwrap_or_default(),
                })
                .unwrap_or_default();
            output.push_str(&replacement);
            rest = &after_start[end + 2..];
        } else {
            output.push_str(&rest[start..]);
            rest = "";
        }
    }

    output.push_str(rest);
    output
}

fn interpolate_graph_json(value: &JsonValue, context: &JsonValue) -> JsonValue {
    match value {
        JsonValue::String(s) => JsonValue::String(interpolate_graph_prompt(s, context)),
        JsonValue::Array(items) => JsonValue::Array(
            items
                .iter()
                .map(|item| interpolate_graph_json(item, context))
                .collect(),
        ),
        JsonValue::Object(map) => JsonValue::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), interpolate_graph_json(value, context)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn parse_json_from_text(value: &str) -> JsonValue {
    if let Ok(parsed) = serde_json::from_str::<JsonValue>(value) {
        return parsed;
    }

    if let (Some(start), Some(end)) = (value.find('{'), value.rfind('}')) {
        if end > start {
            let candidate = &value[start..=end];
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(candidate) {
                return parsed;
            }
        }
    }

    JsonValue::String(value.to_string())
}

fn evaluate_switch_condition(condition: &str, context: &JsonValue) -> bool {
    let trimmed = condition.trim();

    if let Some((left, right)) = trimmed.split_once("==") {
        let left_path = left.trim();
        let right_value = right.trim().trim_matches('"');
        let left_value = get_path_value(context, left_path)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string())
            })
            .unwrap_or_default();
        return left_value == right_value;
    }

    if let Some((left, right)) = trimmed.split_once("!=") {
        let left_path = left.trim();
        let right_value = right.trim().trim_matches('"');
        let left_value = get_path_value(context, left_path)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string())
            })
            .unwrap_or_default();
        return left_value != right_value;
    }

    false
}

fn graph_llm_output_schema(config: Option<&GraphNodeConfig>) -> JsonValue {
    if let Some(schema) = config.and_then(|cfg| cfg.output_schema.clone()) {
        return schema;
    }
    json!({
        "type": "object",
        "additionalProperties": true
    })
}

fn matches_json_schema_type(expected: &str, value: &JsonValue) -> bool {
    match expected {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "string" => value.is_string(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

fn json_value_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn validate_schema_instance(
    schema: &JsonValue,
    value: &JsonValue,
    path: &str,
) -> Result<(), String> {
    let Some(schema_obj) = schema.as_object() else {
        return Ok(());
    };

    if let Some(any_of) = schema_obj.get("anyOf").and_then(JsonValue::as_array) {
        if !any_of.is_empty()
            && !any_of
                .iter()
                .any(|nested| validate_schema_instance(nested, value, path).is_ok())
        {
            return Err(format!("{path} did not satisfy anyOf"));
        }
    }

    if let Some(one_of) = schema_obj.get("oneOf").and_then(JsonValue::as_array) {
        if !one_of.is_empty() {
            let matched = one_of
                .iter()
                .filter(|nested| validate_schema_instance(nested, value, path).is_ok())
                .count();
            if matched != 1 {
                return Err(format!("{path} must satisfy exactly one oneOf schema"));
            }
        }
    }

    if let Some(all_of) = schema_obj.get("allOf").and_then(JsonValue::as_array) {
        for nested in all_of {
            validate_schema_instance(nested, value, path)?;
        }
    }

    if let Some(enum_values) = schema_obj.get("enum").and_then(JsonValue::as_array) {
        if !enum_values.is_empty() && !enum_values.iter().any(|candidate| candidate == value) {
            return Err(format!("{path} must be one of enum values"));
        }
    }

    if let Some(schema_type) = schema_obj.get("type") {
        let matches = match schema_type {
            JsonValue::String(t) => matches_json_schema_type(t, value),
            JsonValue::Array(types) => types.iter().any(|item| {
                item.as_str()
                    .map(|t| matches_json_schema_type(t, value))
                    .unwrap_or(false)
            }),
            _ => true,
        };
        if !matches {
            return Err(format!(
                "{path} expected type {schema_type}, got {}",
                json_value_type_name(value)
            ));
        }
    }

    if let Some(text) = value.as_str() {
        if let Some(min_length) = schema_obj.get("minLength").and_then(JsonValue::as_u64) {
            if text.chars().count() < min_length as usize {
                return Err(format!("{path} must have minLength {min_length}"));
            }
        }
        if let Some(max_length) = schema_obj.get("maxLength").and_then(JsonValue::as_u64) {
            if text.chars().count() > max_length as usize {
                return Err(format!("{path} must have maxLength {max_length}"));
            }
        }
    }

    if let Some(number) = value.as_f64() {
        if let Some(minimum) = schema_obj.get("minimum").and_then(JsonValue::as_f64) {
            if number < minimum {
                return Err(format!("{path} must be >= {minimum}"));
            }
        }
        if let Some(maximum) = schema_obj.get("maximum").and_then(JsonValue::as_f64) {
            if number > maximum {
                return Err(format!("{path} must be <= {maximum}"));
            }
        }
    }

    if let Some(items) = value.as_array() {
        if let Some(min_items) = schema_obj.get("minItems").and_then(JsonValue::as_u64) {
            if items.len() < min_items as usize {
                return Err(format!("{path} must have at least {min_items} items"));
            }
        }
        if let Some(max_items) = schema_obj.get("maxItems").and_then(JsonValue::as_u64) {
            if items.len() > max_items as usize {
                return Err(format!("{path} must have at most {max_items} items"));
            }
        }
        if let Some(item_schema) = schema_obj.get("items") {
            for (index, item) in items.iter().enumerate() {
                validate_schema_instance(item_schema, item, &format!("{path}[{index}]"))?;
            }
        }
    }

    if let Some(object) = value.as_object() {
        let properties = schema_obj
            .get("properties")
            .and_then(JsonValue::as_object)
            .cloned()
            .unwrap_or_default();

        if let Some(required) = schema_obj.get("required").and_then(JsonValue::as_array) {
            for key in required.iter().filter_map(JsonValue::as_str) {
                if !object.contains_key(key) {
                    return Err(format!("{path}.{key} is required"));
                }
            }
        }

        for (key, property_schema) in &properties {
            if let Some(property_value) = object.get(key) {
                validate_schema_instance(
                    property_schema,
                    property_value,
                    &format!("{path}.{key}"),
                )?;
            }
        }

        if let Some(additional) = schema_obj.get("additionalProperties") {
            if additional == &JsonValue::Bool(false) {
                for key in object.keys() {
                    if !properties.contains_key(key) {
                        return Err(format!("{path}.{key} is not allowed"));
                    }
                }
            } else if additional != &JsonValue::Bool(true) {
                for (key, property_value) in object {
                    if properties.contains_key(key) {
                        continue;
                    }
                    validate_schema_instance(additional, property_value, &format!("{path}.{key}"))?;
                }
            }
        }
    }

    Ok(())
}

fn normalize_sse_text(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn pop_next_sse_block(buffer: &mut String) -> Option<String> {
    let delimiter_index = buffer.find("\n\n")?;
    let block = buffer[..delimiter_index].trim().to_string();
    let remaining = buffer[delimiter_index + 2..].to_string();
    *buffer = remaining;
    Some(block)
}

async fn consume_sse_blocks<F>(response: &JsValue, mut on_block: F) -> Result<(), JsValue>
where
    F: FnMut(String) -> Result<bool, JsValue>,
{
    let body = Reflect::get(response, &JsValue::from_str("body"))
        .map_err(|_| js_error("stream response body is unavailable"))?;
    if body.is_null() || body.is_undefined() {
        return Err(js_error("stream response had no body"));
    }

    let reader = call_method0_sync(&body, "getReader")?;

    let global = js_sys::global();
    let text_decoder_ctor = Reflect::get(&global, &JsValue::from_str("TextDecoder"))
        .map_err(|_| js_error("TextDecoder is unavailable"))?;
    let text_decoder_fn = text_decoder_ctor
        .dyn_into::<Function>()
        .map_err(|_| js_error("TextDecoder constructor is not callable"))?;
    let decoder = js_sys::Reflect::construct(&text_decoder_fn, &Array::new())
        .map_err(|_| js_error("failed to construct TextDecoder"))?;

    let mut buffer = String::new();

    loop {
        let read_result = call_method0(&reader, "read").await?;
        let done = Reflect::get(&read_result, &JsValue::from_str("done"))
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if done {
            break;
        }

        let value = Reflect::get(&read_result, &JsValue::from_str("value"))
            .map_err(|_| js_error("stream chunk missing value"))?;

        let decode_options = Object::new();
        Reflect::set(
            &decode_options,
            &JsValue::from_str("stream"),
            &JsValue::TRUE,
        )
        .map_err(|_| js_error("failed to configure stream decoder"))?;

        let chunk_js = call_method2_sync(&decoder, "decode", &value, &decode_options)?;
        let chunk = chunk_js.as_string().unwrap_or_default();
        buffer.push_str(&normalize_sse_text(&chunk));

        while let Some(block) = pop_next_sse_block(&mut buffer) {
            if block.is_empty() {
                continue;
            }
            let should_continue = on_block(block)?;
            if !should_continue {
                return Ok(());
            }
        }
    }

    let trailing_js = call_method0_sync(&decoder, "decode")?;
    let trailing = trailing_js.as_string().unwrap_or_default();
    buffer.push_str(&normalize_sse_text(&trailing));

    let trailing_block = buffer.trim().to_string();
    if !trailing_block.is_empty() {
        let _ = on_block(trailing_block)?;
    }

    Ok(())
}

fn parse_sse_data_line(block: &str) -> Option<String> {
    let mut data_lines = Vec::new();
    for line in block.lines() {
        let normalized = line.trim_end_matches('\r');
        if let Some(rest) = normalized.strip_prefix("data:") {
            data_lines.push(rest.trim_start().to_string());
        }
    }
    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

#[wasm_bindgen]
pub struct WasmClient {
    provider: String,
    base_url: String,
    api_key: String,
    headers: HashMap<String, String>,
}

#[wasm_bindgen]
impl WasmClient {
    #[wasm_bindgen(constructor)]
    pub fn new(provider: String, config: JsValue) -> Result<WasmClient, JsValue> {
        if provider != "openai" && provider != "openrouter" {
            return Err(config_error(
                "provider must be 'openai' or 'openrouter' in wasm mode",
            ));
        }

        let parsed: ClientConfig = serde_wasm_bindgen::from_value(config)
            .map_err(|_| config_error("invalid client config object"))?;
        if parsed.api_key.trim().is_empty() {
            return Err(config_error("config.apiKey is required"));
        }

        let base = parsed
            .base_url
            .or_else(|| default_base_url(&provider).map(str::to_string))
            .ok_or_else(|| config_error("baseUrl is required"))?;

        Ok(Self {
            provider,
            base_url: normalize_base_url(&base),
            api_key: parsed.api_key,
            headers: parsed.headers.unwrap_or_default(),
        })
    }

    #[wasm_bindgen(js_name = complete)]
    pub async fn complete(
        &self,
        model: String,
        prompt_or_messages: JsValue,
        options: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        if model.trim().is_empty() {
            return Err(config_error("model cannot be empty"));
        }

        let opts = if let Some(value) = options {
            serde_wasm_bindgen::from_value::<CompleteOptions>(value)
                .map_err(|_| config_error("invalid options object"))?
        } else {
            CompleteOptions::default()
        };

        if let Some(mode) = opts.mode.as_deref() {
            if mode == "healed_json" || mode == "schema" {
                return Err(js_error(
                    "healed_json and schema modes are not supported in simple-agents-wasm yet",
                ));
            }
        }

        let messages = to_openai_messages(to_messages(prompt_or_messages)?);
        let messages_value = serde_json::to_value(messages)
            .map_err(|_| js_error("failed to serialize request messages"))?;
        let body = json!({
            "model": model,
            "messages": messages_value,
            "max_tokens": opts.max_tokens,
            "temperature": opts.temperature,
            "top_p": opts.top_p,
            "stream": false
        });

        let started = now_millis();
        let mut headers = self.headers.clone();
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", self.api_key),
        );
        if self.provider == "openrouter" {
            headers
                .entry("HTTP-Referer".to_string())
                .or_insert_with(|| "https://simpleagents.dev".to_string());
        }

        let response = js_fetch(
            &format!("{}/chat/completions", self.base_url),
            &body,
            &headers,
        )
        .await?;

        let ok = Reflect::get(&response, &JsValue::from_str("ok"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !ok {
            let status = Reflect::get(&response, &JsValue::from_str("status"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as u16;
            let text_js = call_method0(&response, "text").await?;
            let text = text_js.as_string().unwrap_or_default();
            return Err(js_error(format!(
                "request failed ({status}): {}",
                text.chars().take(500).collect::<String>()
            )));
        }

        let json_js = call_method0(&response, "json").await?;
        let payload: JsonValue = serde_wasm_bindgen::from_value(json_js)
            .map_err(|_| js_error("failed to parse response JSON"))?;

        let choice = payload
            .get("choices")
            .and_then(JsonValue::as_array)
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or(JsonValue::Null);

        let content = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(JsonValue::as_str)
            .map(str::to_string);

        let tool_calls = choice
            .get("message")
            .and_then(|m| m.get("tool_calls"))
            .and_then(JsonValue::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|call| ToolCallOut {
                        id: call
                            .get("id")
                            .and_then(JsonValue::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        tool_type: call
                            .get("type")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("function")
                            .to_string(),
                        function: ToolCallFunctionOut {
                            name: call
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(JsonValue::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            arguments: call
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(JsonValue::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        },
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty());

        let usage = payload.get("usage").cloned().unwrap_or(JsonValue::Null);
        let usage_obj = CompletionUsage {
            prompt_tokens: usage
                .get("prompt_tokens")
                .and_then(JsonValue::as_u64)
                .unwrap_or(0) as u32,
            completion_tokens: usage
                .get("completion_tokens")
                .and_then(JsonValue::as_u64)
                .unwrap_or(0) as u32,
            total_tokens: usage
                .get("total_tokens")
                .and_then(JsonValue::as_u64)
                .unwrap_or(0) as u32,
        };

        let latency_ms = (now_millis() - started).max(0.0) as u32;

        let result = CompletionResult {
            id: payload
                .get("id")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_string(),
            model: payload
                .get("model")
                .and_then(JsonValue::as_str)
                .unwrap_or(&model)
                .to_string(),
            role: choice
                .get("message")
                .and_then(|m| m.get("role"))
                .and_then(JsonValue::as_str)
                .unwrap_or("assistant")
                .to_string(),
            content,
            tool_calls,
            finish_reason: choice
                .get("finish_reason")
                .and_then(JsonValue::as_str)
                .map(str::to_string),
            usage: usage_obj,
            usage_available: usage.is_object(),
            latency_ms,
            raw: serde_json::to_string(&payload).ok(),
            healed: None,
            coerced: None,
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|_| js_error("failed to serialize completion result"))
    }

    #[wasm_bindgen(js_name = streamEvents)]
    pub async fn stream_events(
        &self,
        model: String,
        prompt_or_messages: JsValue,
        on_event: Function,
        options: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let opts = if let Some(value) = options {
            serde_wasm_bindgen::from_value::<CompleteOptions>(value)
                .map_err(|_| config_error("invalid options object"))?
        } else {
            CompleteOptions::default()
        };

        let messages = to_openai_messages(to_messages(prompt_or_messages)?);
        let messages_value = serde_json::to_value(messages)
            .map_err(|_| js_error("failed to serialize request messages"))?;
        let body = json!({
            "model": model,
            "messages": messages_value,
            "max_tokens": opts.max_tokens,
            "temperature": opts.temperature,
            "top_p": opts.top_p,
            "stream": true
        });

        let started = now_millis();
        let mut headers = self.headers.clone();
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", self.api_key),
        );
        if self.provider == "openrouter" {
            headers
                .entry("HTTP-Referer".to_string())
                .or_insert_with(|| "https://simpleagents.dev".to_string());
        }

        let response = js_fetch(
            &format!("{}/chat/completions", self.base_url),
            &body,
            &headers,
        )
        .await?;
        let ok = Reflect::get(&response, &JsValue::from_str("ok"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !ok {
            let status = Reflect::get(&response, &JsValue::from_str("status"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as u16;
            let text_js = call_method0(&response, "text").await?;
            let text = text_js.as_string().unwrap_or_default();
            let message = format!(
                "request failed ({status}): {}",
                text.chars().take(500).collect::<String>()
            );
            let err_event = json!({
                "eventType": "error",
                "error": { "message": message }
            });
            let event_js = serde_wasm_bindgen::to_value(&err_event)
                .map_err(|_| js_error("failed to serialize stream error event"))?;
            on_event
                .call1(&JsValue::NULL, &event_js)
                .map_err(|_| js_error("failed to call stream callback"))?;
            return Err(js_error(message));
        }

        let mut aggregate = String::new();
        let mut response_id = String::new();
        let mut response_model = model.clone();
        let mut finish_reason: Option<String> = None;

        consume_sse_blocks(&response, |block| {
            let Some(data) = parse_sse_data_line(&block) else {
                return Ok(true);
            };
            if data == "[DONE]" {
                return Ok(false);
            }

            let Ok(chunk) = serde_json::from_str::<JsonValue>(&data) else {
                return Ok(true);
            };
            let choice = chunk
                .get("choices")
                .and_then(JsonValue::as_array)
                .and_then(|arr| arr.first())
                .cloned()
                .unwrap_or(JsonValue::Null);

            let delta_content = choice
                .get("delta")
                .and_then(|d| d.get("content"))
                .and_then(JsonValue::as_str)
                .map(str::to_string);
            let delta_role = choice
                .get("delta")
                .and_then(|d| d.get("role"))
                .and_then(JsonValue::as_str)
                .map(str::to_string);
            let chunk_id = chunk
                .get("id")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_string();
            let chunk_model = chunk
                .get("model")
                .and_then(JsonValue::as_str)
                .unwrap_or(&response_model)
                .to_string();

            if response_id.is_empty() && !chunk_id.is_empty() {
                response_id = chunk_id.clone();
            }
            response_model = chunk_model.clone();
            if let Some(content) = delta_content.clone() {
                aggregate.push_str(&content);
            }
            finish_reason = choice
                .get("finish_reason")
                .and_then(JsonValue::as_str)
                .map(str::to_string)
                .or(finish_reason.clone());

            let delta_event = json!({
                "eventType": "delta",
                "delta": {
                    "id": chunk_id,
                    "model": chunk_model,
                    "index": choice.get("index").and_then(JsonValue::as_u64).unwrap_or(0),
                    "role": delta_role,
                    "content": delta_content,
                    "finishReason": choice.get("finish_reason").and_then(JsonValue::as_str),
                    "raw": data,
                }
            });
            let event_js = serde_wasm_bindgen::to_value(&delta_event)
                .map_err(|_| js_error("failed to serialize stream delta event"))?;
            on_event
                .call1(&JsValue::NULL, &event_js)
                .map_err(|_| js_error("failed to call stream callback"))?;

            Ok(true)
        })
        .await?;

        let done_event = json!({ "eventType": "done" });
        let done_js = serde_wasm_bindgen::to_value(&done_event)
            .map_err(|_| js_error("failed to serialize stream done event"))?;
        on_event
            .call1(&JsValue::NULL, &done_js)
            .map_err(|_| js_error("failed to call stream callback"))?;

        let latency_ms = (now_millis() - started).max(0.0) as u32;
        let result = CompletionResult {
            id: response_id,
            model: response_model,
            role: "assistant".to_string(),
            content: Some(aggregate),
            tool_calls: None,
            finish_reason,
            usage: CompletionUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            usage_available: false,
            latency_ms,
            raw: None,
            healed: None,
            coerced: None,
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|_| js_error("failed to serialize stream completion result"))
    }

    #[wasm_bindgen(js_name = runWorkflowYamlString)]
    pub async fn run_workflow_yaml_string(
        &self,
        yaml_text: String,
        workflow_input: JsValue,
        workflow_options: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let raw_doc: JsonValue = serde_yaml::from_str(&yaml_text)
            .map_err(|error| config_error(format!("invalid workflow YAML: {error}")))?;

        let mut context: JsonMap<String, JsonValue> =
            serde_wasm_bindgen::from_value(workflow_input)
                .map_err(|_| config_error("workflowInput must be an object"))?;

        let mut options = WorkflowRunOptions {
            telemetry: None,
            trace: None,
            functions_js: None,
        };
        if let Some(options_js) = workflow_options {
            options = serde_wasm_bindgen::from_value(options_js.clone())
                .map_err(|_| config_error("invalid workflowOptions object"))?;
            let functions_value = Reflect::get(&options_js, &JsValue::from_str("functions")).ok();
            options.functions_js = functions_value;
        }

        if raw_doc.get("entry_node").is_some() && raw_doc.get("nodes").is_some() {
            let graph_doc: GraphWorkflowDoc = serde_json::from_value(raw_doc.clone())
                .map_err(|error| config_error(format!("invalid graph workflow YAML: {error}")))?;

            let mut node_by_id: HashMap<String, GraphWorkflowNode> = HashMap::new();
            for node in &graph_doc.nodes {
                if node.id.trim().is_empty() {
                    return Err(config_error("graph workflow node id cannot be empty"));
                }
                node_by_id.insert(node.id.clone(), node.clone());
            }

            let mut edge_map: HashMap<String, Vec<String>> = HashMap::new();
            if let Some(edges) = graph_doc.edges.as_ref() {
                for edge in edges {
                    edge_map
                        .entry(edge.from.clone())
                        .or_default()
                        .push(edge.to.clone());
                }
            }

            let mut graph_context = json!({
                "input": JsonValue::Object(context.clone()),
                "nodes": JsonValue::Object(JsonMap::new())
            });

            let workflow_started = now_millis();
            let mut events = Vec::new();
            let mut output: Option<JsonValue> = None;
            let mut pointer = graph_doc.entry_node.clone();
            let mut iterations = 0usize;
            let mut workflow_ttft_ms: Option<u64> = None;
            let mut trace: Vec<String> = Vec::new();
            let mut step_timings: Vec<JsonValue> = Vec::new();
            let mut total_input_tokens: u64 = 0;
            let mut total_output_tokens: u64 = 0;
            let mut total_tokens: u64 = 0;
            let total_reasoning_tokens: u64 = 0;
            let mut llm_nodes_without_usage: Vec<String> = Vec::new();

            while !pointer.is_empty() {
                iterations += 1;
                if iterations > 1000 {
                    return Err(js_error("workflow exceeded maximum step iterations"));
                }

                let node = node_by_id.get(&pointer).cloned().ok_or_else(|| {
                    config_error(format!("workflow references unknown node '{}'", pointer))
                })?;

                let step_type = if node.node_type.llm_call.is_some() {
                    "llm_call"
                } else if node.node_type.switch.is_some() {
                    "switch"
                } else if node.node_type.custom_worker.is_some() {
                    "custom_worker"
                } else {
                    "unknown"
                };

                events.push(WorkflowRunEvent {
                    step_id: node.id.clone(),
                    step_type: step_type.to_string(),
                    status: "started".to_string(),
                });

                let node_started = now_millis();
                let mut model_name: Option<String> = None;
                let mut prompt_tokens: Option<u64> = None;
                let mut completion_tokens: Option<u64> = None;
                let mut total_node_tokens: Option<u64> = None;

                if let Some(llm) = node.node_type.llm_call.as_ref() {
                    let model = llm
                        .model
                        .clone()
                        .or_else(|| graph_doc.model.clone())
                        .or_else(|| {
                            context
                                .get("model")
                                .and_then(JsonValue::as_str)
                                .map(str::to_string)
                        })
                        .ok_or_else(|| {
                            config_error(format!(
                                "llm_call node '{}' requires node_type.llm_call.model",
                                node.id
                            ))
                        })?;

                    let prompt = interpolate_graph_prompt(
                        node.config
                            .as_ref()
                            .and_then(|config| config.prompt.as_deref())
                            .unwrap_or_default(),
                        &graph_context,
                    );

                    let prompt_js = if llm.messages_path.as_deref() == Some("input.messages") {
                        let mut history: Vec<MessageInput> =
                            get_path_value(&graph_context, "input.messages")
                                .and_then(|value| {
                                    serde_json::from_value::<Vec<MessageInput>>(value.clone()).ok()
                                })
                                .unwrap_or_default();
                        if llm.append_prompt_as_user.unwrap_or(true) {
                            history.push(MessageInput {
                                role: "user".to_string(),
                                content: JsonValue::String(prompt),
                                name: None,
                                tool_call_id: None,
                                tool_calls: None,
                            });
                        }
                        serde_wasm_bindgen::to_value(&history)
                            .map_err(|_| js_error("failed to serialize graph llm messages"))?
                    } else {
                        JsValue::from_str(&prompt)
                    };

                    let opts = json!({ "temperature": llm.temperature });
                    let completion_js = self
                        .complete(
                            model,
                            prompt_js,
                            Some(
                                serde_wasm_bindgen::to_value(&opts).map_err(|_| {
                                    js_error("failed to serialize completion options")
                                })?,
                            ),
                        )
                        .await?;
                    let completion: JsonValue = serde_wasm_bindgen::from_value(completion_js)
                        .map_err(|_| js_error("failed to parse completion result"))?;

                    if workflow_ttft_ms.is_none() {
                        let measured = (now_millis() - workflow_started).max(0.0) as u64;
                        workflow_ttft_ms = Some(if measured == 0 { 1 } else { measured });
                    }

                    model_name = completion
                        .get("model")
                        .and_then(JsonValue::as_str)
                        .map(ToString::to_string);
                    let usage = completion.get("usage");
                    let usage_available = completion
                        .get("usage_available")
                        .and_then(JsonValue::as_bool)
                        .unwrap_or(false);
                    if usage_available {
                        prompt_tokens = usage
                            .and_then(|value| value.get("prompt_tokens"))
                            .and_then(JsonValue::as_u64);
                        completion_tokens = usage
                            .and_then(|value| value.get("completion_tokens"))
                            .and_then(JsonValue::as_u64);
                        total_node_tokens = usage
                            .and_then(|value| value.get("total_tokens"))
                            .and_then(JsonValue::as_u64);
                    }

                    let raw_content = completion
                        .get("content")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default();
                    let parsed_output = parse_json_from_text(raw_content);
                    let output_schema = graph_llm_output_schema(node.config.as_ref());
                    validate_schema_instance(&output_schema, &parsed_output, "$").map_err(
                        |message| {
                            js_error(format!(
                                "llm_call node '{}' output failed schema validation: {}",
                                node.id, message
                            ))
                        },
                    )?;

                    if let Some(nodes_map) = graph_context
                        .get_mut("nodes")
                        .and_then(JsonValue::as_object_mut)
                    {
                        nodes_map.insert(
                            node.id.clone(),
                            json!({
                                "output": parsed_output,
                                "raw": raw_content
                            }),
                        );
                    }

                    output = Some(parsed_output);
                    pointer = edge_map
                        .get(&node.id)
                        .and_then(|targets| targets.first())
                        .cloned()
                        .unwrap_or_default();
                } else if let Some(switch) = node.node_type.switch.as_ref() {
                    let mut next_pointer = switch.default.clone().unwrap_or_default();
                    if let Some(branches) = switch.branches.as_ref() {
                        for branch in branches {
                            let matches = branch
                                .condition
                                .as_ref()
                                .map(|condition| {
                                    evaluate_switch_condition(condition, &graph_context)
                                })
                                .unwrap_or(false);
                            if matches {
                                next_pointer = branch.target.clone().unwrap_or_default();
                                break;
                            }
                        }
                    }
                    pointer = next_pointer;
                } else if let Some(custom_worker) = node.node_type.custom_worker.as_ref() {
                    let handler = custom_worker
                        .handler
                        .clone()
                        .unwrap_or_else(|| "custom_worker".to_string());
                    let lookup_key = custom_worker
                        .handler_file
                        .as_deref()
                        .map(|file| format!("{}#{}", file, handler.as_str()))
                        .unwrap_or_else(|| handler.clone());
                    let functions_js = options.functions_js.clone().ok_or_else(|| {
                        config_error(format!(
                            "custom_worker node '{}' requires workflowOptions.functions",
                            node.id
                        ))
                    })?;
                    let function_value = Reflect::get(&functions_js, &JsValue::from_str(&lookup_key))
                        .map_err(|_| {
                            config_error(format!(
                                "failed to resolve custom worker handler key '{}' from workflowOptions.functions",
                                lookup_key
                            ))
                        })?;
                    let function = function_value.dyn_into::<Function>().map_err(|_| {
                        config_error(format!(
                            "custom_worker node '{}' requires workflowOptions.functions['{}']",
                            node.id, lookup_key
                        ))
                    })?;

                    let worker_args = json!({
                        "handler": handler,
                        "handler_file": custom_worker.handler_file,
                        "handler_lookup_key": lookup_key,
                        "payload": node
                            .config
                            .as_ref()
                            .and_then(|config| config.payload.as_ref())
                            .map(|payload| interpolate_graph_json(payload, &graph_context))
                            .unwrap_or(JsonValue::Null),
                        "nodeId": node.id.clone()
                    });
                    let args_js = serde_wasm_bindgen::to_value(&worker_args)
                        .map_err(|_| js_error("failed to serialize custom_worker args"))?;
                    let context_js = serde_wasm_bindgen::to_value(&graph_context)
                        .map_err(|_| js_error("failed to serialize graph context"))?;
                    let worker_call_output = function
                        .call2(&JsValue::NULL, &args_js, &context_js)
                        .map_err(|_| {
                            js_error(format!(
                                "custom_worker node '{}' handler call failed",
                                node.id
                            ))
                        })?;
                    let resolved_output = if worker_call_output.is_instance_of::<Promise>() {
                        JsFuture::from(worker_call_output.unchecked_into::<Promise>())
                            .await
                            .map_err(|_| {
                                js_error(format!(
                                    "custom_worker node '{}' async handler promise rejected",
                                    node.id
                                ))
                            })?
                    } else {
                        worker_call_output
                    };
                    let worker_output =
                        serde_wasm_bindgen::from_value(resolved_output).unwrap_or(JsonValue::Null);

                    if let Some(nodes_map) = graph_context
                        .get_mut("nodes")
                        .and_then(JsonValue::as_object_mut)
                    {
                        nodes_map.insert(node.id.clone(), json!({ "output": worker_output }));
                    }

                    output = Some(worker_output);
                    pointer = edge_map
                        .get(&node.id)
                        .and_then(|targets| targets.first())
                        .cloned()
                        .unwrap_or_default();
                } else {
                    return Err(config_error(
                        "unsupported node_type in simple-agents-wasm graph workflow",
                    ));
                }

                events.push(WorkflowRunEvent {
                    step_id: node.id.clone(),
                    step_type: step_type.to_string(),
                    status: "completed".to_string(),
                });
                let elapsed_ms = (now_millis() - node_started).max(0.0) as u64;
                if step_type == "llm_call" {
                    if prompt_tokens.is_none() || completion_tokens.is_none() || total_node_tokens.is_none() {
                        llm_nodes_without_usage.push(node.id.clone());
                    }
                    total_input_tokens += prompt_tokens.unwrap_or(0);
                    total_output_tokens += completion_tokens.unwrap_or(0);
                    total_tokens += total_node_tokens.unwrap_or(0);
                }
                let step_tokens_per_second = completion_tokens.map(|tokens| {
                    if elapsed_ms == 0 {
                        tokens as f64
                    } else {
                        ((tokens as f64 / (elapsed_ms as f64 / 1000.0)) * 100.0).round() / 100.0
                    }
                });
                step_timings.push(json!({
                    "node_id": node.id.clone(),
                    "node_kind": step_type,
                    "model_name": model_name,
                    "elapsed_ms": elapsed_ms,
                    "prompt_tokens": prompt_tokens,
                    "completion_tokens": completion_tokens,
                    "total_tokens": total_node_tokens,
                    "reasoning_tokens": 0,
                    "tokens_per_second": step_tokens_per_second,
                }));
                trace.push(node.id.clone());
            }

            let total_elapsed_ms = (now_millis() - workflow_started).max(0.0) as u64;
            let terminal_node = trace.last().cloned().unwrap_or_default();
            let workflow_id = raw_doc
                .get("id")
                .and_then(JsonValue::as_str)
                .unwrap_or("wasm_workflow")
                .to_string();
            let email_text = graph_context
                .get("input")
                .and_then(|input| input.get("email_text"))
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_string();
            let outputs = graph_context
                .get("nodes")
                .cloned()
                .unwrap_or(JsonValue::Object(JsonMap::new()));
            let token_metrics_available = llm_nodes_without_usage.is_empty();
            let overall_tokens_per_second = if total_elapsed_ms == 0 {
                Some(total_output_tokens as f64)
            } else {
                Some(
                    ((total_output_tokens as f64 / (total_elapsed_ms as f64 / 1000.0)) * 100.0)
                        .round()
                        / 100.0,
                )
            };
            let nerdstats = json!({
                "workflow_id": workflow_id,
                "terminal_node": terminal_node,
                "total_elapsed_ms": total_elapsed_ms,
                "ttft_ms": workflow_ttft_ms.unwrap_or(0),
                "step_details": step_timings,
                "total_input_tokens": total_input_tokens,
                "total_output_tokens": total_output_tokens,
                "total_tokens": total_tokens,
                "total_reasoning_tokens": total_reasoning_tokens,
                "tokens_per_second": overall_tokens_per_second,
                "trace_id": JsonValue::Null,
                "token_metrics_available": token_metrics_available,
                "token_metrics_source": if token_metrics_available { "provider_usage" } else { "unavailable" },
                "llm_nodes_without_usage": llm_nodes_without_usage,
            });

            let result = WorkflowRunResult {
                status: "ok".to_string(),
                context: graph_context,
                output: output.clone(),
                events,
                workflow_id: Some(
                    raw_doc
                        .get("id")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("wasm_workflow")
                        .to_string(),
                ),
                entry_node: Some(graph_doc.entry_node),
                email_text: Some(email_text),
                trace: Some(trace),
                outputs: Some(outputs),
                terminal_node: Some(terminal_node),
                terminal_output: output,
                step_timings: Some(nerdstats.get("step_details").cloned().unwrap_or(JsonValue::Array(vec![]))),
                total_elapsed_ms: Some(total_elapsed_ms),
                ttft_ms: Some(workflow_ttft_ms.unwrap_or(0)),
                total_input_tokens: Some(total_input_tokens),
                total_output_tokens: Some(total_output_tokens),
                total_tokens: Some(total_tokens),
                total_reasoning_tokens: Some(total_reasoning_tokens),
                tokens_per_second: overall_tokens_per_second,
                trace_id: None,
                metadata: Some(json!({"nerdstats": nerdstats})),
            };
            return serde_wasm_bindgen::to_value(&result)
                .map_err(|_| js_error("failed to serialize workflow result"));
        }

        let doc: WorkflowDoc = serde_json::from_value(raw_doc)
            .map_err(|error| config_error(format!("invalid workflow YAML: {error}")))?;
        if doc.steps.is_empty() {
            return Err(config_error(
                "workflow YAML must contain a non-empty steps array",
            ));
        }

        let mut index_by_id = HashMap::new();
        for (index, step) in doc.steps.iter().enumerate() {
            if step.id.trim().is_empty() || step.step_type.trim().is_empty() {
                return Err(config_error(format!(
                    "workflow step at index {index} requires id and type"
                )));
            }
            index_by_id.insert(step.id.clone(), index);
        }

        let mut events = Vec::new();
        let mut output: Option<JsonValue> = None;
        let mut pointer = 0usize;
        let mut iterations = 0usize;

        while pointer < doc.steps.len() {
            iterations += 1;
            if iterations > 1000 {
                return Err(js_error("workflow exceeded maximum step iterations"));
            }

            let step = doc
                .steps
                .get(pointer)
                .cloned()
                .ok_or_else(|| js_error("workflow step index out of range"))?;

            events.push(WorkflowRunEvent {
                step_id: step.id.clone(),
                step_type: step.step_type.clone(),
                status: "started".to_string(),
            });

            match step.step_type.as_str() {
                "set" => {
                    let key = step.key.ok_or_else(|| {
                        config_error(format!("set step '{}' requires key", step.id))
                    })?;
                    let value = interpolate_json(&step.value.unwrap_or(JsonValue::Null), &context);
                    context.insert(key, value);
                }
                "llm_call" => {
                    let model = step
                        .model
                        .or_else(|| doc.model.clone())
                        .or_else(|| context.get("model").and_then(JsonValue::as_str).map(str::to_string))
                        .ok_or_else(|| {
                            config_error(format!(
                                "llm_call step '{}' requires model via step.model, workflow model, or workflowInput.model",
                                step.id
                            ))
                        })?;
                    let prompt =
                        interpolate_string(step.prompt.as_deref().unwrap_or_default(), &context);
                    let opts = json!({ "temperature": step.temperature });
                    let completion_js = self
                        .complete(
                            model,
                            JsValue::from_str(&prompt),
                            Some(
                                serde_wasm_bindgen::to_value(&opts).map_err(|_| {
                                    js_error("failed to serialize completion options")
                                })?,
                            ),
                        )
                        .await?;
                    let completion: JsonValue = serde_wasm_bindgen::from_value(completion_js)
                        .map_err(|_| js_error("failed to parse completion result"))?;
                    let content = completion
                        .get("content")
                        .cloned()
                        .unwrap_or_else(|| JsonValue::String(String::new()));
                    context.insert(step.id.clone(), content);
                }
                "if" => {
                    let condition = step.condition.ok_or_else(|| {
                        config_error(format!("if step '{}' requires condition", step.id))
                    })?;
                    let matched = evaluate_condition(&condition, &context);
                    let target_id = if matched { step.then } else { step.r#else };
                    if let Some(target) = target_id {
                        let jump_to = index_by_id.get(&target).copied().ok_or_else(|| {
                            config_error(format!(
                                "if step '{}' points to unknown step '{}'",
                                step.id, target
                            ))
                        })?;
                        events.push(WorkflowRunEvent {
                            step_id: step.id,
                            step_type: step.step_type,
                            status: "completed".to_string(),
                        });
                        pointer = jump_to;
                        continue;
                    }
                }
                "call_function" => {
                    let function_name = step.function.ok_or_else(|| {
                        config_error(format!(
                            "call_function step '{}' requires function",
                            step.id
                        ))
                    })?;
                    let functions_js = options.functions_js.clone().ok_or_else(|| {
                        config_error(
                            "workflowOptions.functions is required for call_function steps",
                        )
                    })?;
                    let function_value =
                        Reflect::get(&functions_js, &JsValue::from_str(&function_name)).map_err(
                            |_| {
                                config_error(format!(
                                "failed to resolve function '{}' from workflowOptions.functions",
                                function_name
                            ))
                            },
                        )?;
                    let function = function_value.dyn_into::<Function>().map_err(|_| {
                        config_error(format!(
                            "call_function step '{}' references unknown function '{}'",
                            step.id, function_name
                        ))
                    })?;

                    let args_value = interpolate_json(
                        &step
                            .args
                            .unwrap_or_else(|| JsonValue::Object(JsonMap::new())),
                        &context,
                    );
                    let args_js = serde_wasm_bindgen::to_value(&args_value)
                        .map_err(|_| js_error("failed to serialize call_function args"))?;
                    let context_js =
                        serde_wasm_bindgen::to_value(&JsonValue::Object(context.clone()))
                            .map_err(|_| js_error("failed to serialize workflow context"))?;
                    let call_output = function
                        .call2(&JsValue::NULL, &args_js, &context_js)
                        .map_err(|_| {
                            js_error(format!(
                                "call_function step '{}' failed for function '{}'",
                                step.id, function_name
                            ))
                        })?;
                    let resolved_output = if call_output.is_instance_of::<Promise>() {
                        JsFuture::from(call_output.unchecked_into::<Promise>())
                            .await
                            .map_err(|_| js_error("async call_function promise rejected"))?
                    } else {
                        call_output
                    };
                    let output_json =
                        serde_wasm_bindgen::from_value(resolved_output).unwrap_or(JsonValue::Null);
                    context.insert(step.id.clone(), output_json);
                }
                "output" => {
                    let source = step
                        .text
                        .map(JsonValue::String)
                        .or(step.value)
                        .unwrap_or_else(|| JsonValue::String(String::new()));
                    let rendered = interpolate_json(&source, &context);
                    output = Some(rendered.clone());
                    context.insert(step.id.clone(), rendered);
                }
                other => {
                    return Err(config_error(format!(
                        "unsupported step type '{}' in simple-agents-wasm",
                        other
                    )))
                }
            }

            events.push(WorkflowRunEvent {
                step_id: step.id.clone(),
                step_type: step.step_type.clone(),
                status: "completed".to_string(),
            });

            if let Some(next) = step.next {
                pointer = index_by_id.get(&next).copied().ok_or_else(|| {
                    config_error(format!(
                        "step '{}' points to unknown next step '{}'",
                        step.id, next
                    ))
                })?;
                continue;
            }

            pointer += 1;
        }

        let result = WorkflowRunResult {
            status: "ok".to_string(),
            context: JsonValue::Object(context),
            output,
            events,
            workflow_id: None,
            entry_node: None,
            email_text: None,
            trace: None,
            outputs: None,
            terminal_node: None,
            terminal_output: None,
            step_timings: None,
            total_elapsed_ms: None,
            ttft_ms: None,
            total_input_tokens: None,
            total_output_tokens: None,
            total_tokens: None,
            total_reasoning_tokens: None,
            tokens_per_second: None,
            trace_id: None,
            metadata: None,
        };
        serde_wasm_bindgen::to_value(&result)
            .map_err(|_| js_error("failed to serialize workflow result"))
    }

    #[wasm_bindgen(js_name = runWorkflowYaml)]
    pub fn run_workflow_yaml(
        &self,
        workflow_path: String,
        _workflow_input: JsValue,
    ) -> Result<JsValue, JsValue> {
        Err(js_error(format!(
            "workflow file paths are not supported in browser runtime: {workflow_path}"
        )))
    }

    /// Run a YAML workflow from a YAML text string (new unified API alias).
    ///
    /// This is an alias for `runWorkflowYamlString` using the new naming convention.
    /// WASM cannot use file paths; pass the YAML document text directly.
    ///
    /// ```js
    /// const result = await client.runYamlString(yamlText, messages);
    /// ```
    #[wasm_bindgen(js_name = runYamlString)]
    pub async fn run_yaml_string(
        &self,
        yaml_text: String,
        messages: JsValue,
        options: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        // Build workflow input envelope from the messages arg
        let messages_value: serde_json::Value = if messages.is_array() || messages.is_object() {
            serde_wasm_bindgen::from_value(messages)
                .unwrap_or(serde_json::json!([]))
        } else if let Some(prompt) = messages.as_string() {
            serde_json::json!([{"role": "user", "content": prompt}])
        } else {
            serde_json::json!([])
        };

        let input_js = serde_wasm_bindgen::to_value(&serde_json::json!({ "messages": messages_value }))
            .map_err(|_| js_error("failed to serialize messages input"))?;

        self.run_workflow_yaml_string(yaml_text, input_js, options).await
    }
}

#[wasm_bindgen(js_name = supportsRustWasm)]
pub fn supports_rust_wasm() -> bool {
    true
}

#[wasm_bindgen(js_name = toJsArray)]
pub fn to_js_array(value: JsValue) -> Array {
    let arr = Array::new();
    arr.push(&value);
    arr
}
