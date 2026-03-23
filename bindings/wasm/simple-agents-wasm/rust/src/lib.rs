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
    content: String,
    name: Option<String>,
    tool_call_id: Option<String>,
    tool_calls: Option<Vec<JsToolCall>>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JsToolCall {
    id: String,
    tool_type: Option<String>,
    function: JsToolCallFunction,
}

#[derive(Deserialize, Serialize, Clone)]
struct JsToolCallFunction {
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
            content: trimmed.to_string(),
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

fn parse_sse_blocks(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_sse_data_line(block: &str) -> Option<String> {
    let mut data_lines = Vec::new();
    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
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

        let messages = to_messages(prompt_or_messages)?;
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

        let messages = to_messages(prompt_or_messages)?;
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

        let text_js = call_method0(&response, "text").await?;
        let text = text_js.as_string().unwrap_or_default();

        let mut aggregate = String::new();
        let mut response_id = String::new();
        let mut response_model = model.clone();
        let mut finish_reason: Option<String> = None;

        for block in parse_sse_blocks(&text) {
            let Some(data) = parse_sse_data_line(&block) else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }

            let Ok(chunk) = serde_json::from_str::<JsonValue>(&data) else {
                continue;
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
                .or(finish_reason);

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
        }

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
        let doc: WorkflowDoc = serde_yaml::from_str(&yaml_text)
            .map_err(|error| config_error(format!("invalid workflow YAML: {error}")))?;
        if doc.steps.is_empty() {
            return Err(config_error(
                "workflow YAML must contain a non-empty steps array",
            ));
        }

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
