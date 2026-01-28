//! Python bindings for SimpleAgents using PyO3.

#![allow(clippy::useless_conversion)]

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use reqwest::Client as HttpClient;
use simple_agents_core::{SimpleAgentsClient, SimpleAgentsClientBuilder};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{ApiKey, CompletionRequest, Provider, Result, SimpleAgentsError};
use simple_agent_type::request::{JsonSchemaFormat, ResponseFormat};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Runtime = tokio::runtime::Runtime;

#[pyclass]
struct Client {
    runtime: Mutex<Runtime>,
    client: SimpleAgentsClient,
}

fn provider_from_params(
    provider_name: &str,
    api_key: Option<&str>,
    api_base: Option<&str>,
) -> Result<Arc<dyn Provider>> {
    let api_key = match api_key {
        Some(value) => Some(ApiKey::new(value)?),
        None => None,
    };

    match provider_name {
        "openai" => {
            let provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        if is_local_base(api_base) {
                            let client = HttpClient::builder()
                                .timeout(Duration::from_secs(30))
                                .pool_max_idle_per_host(10)
                                .pool_idle_timeout(Duration::from_secs(90))
                                .no_proxy()
                                .build()
                                .map_err(|e| {
                                    SimpleAgentsError::Config(format!(
                                        "Failed to create HTTP client: {}",
                                        e
                                    ))
                                })?;
                            OpenAIProvider::with_client(api_key, api_base.to_string(), client)?
                        } else {
                            OpenAIProvider::with_base_url(api_key, api_base.to_string())?
                        }
                    }
                    None => OpenAIProvider::new(api_key)?,
                },
                None => OpenAIProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        "anthropic" => {
            let provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        AnthropicProvider::with_base_url(api_key, api_base.to_string())?
                    }
                    None => AnthropicProvider::new(api_key)?,
                },
                None => AnthropicProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        "openrouter" => {
            let provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        OpenRouterProvider::with_base_url(api_key, api_base.to_string())?
                    }
                    None => OpenRouterProvider::new(api_key)?,
                },
                None => OpenRouterProvider::from_env()?,
            };
            Ok(Arc::new(provider))
        }
        _ => Err(SimpleAgentsError::Config(format!(
            "Unknown provider '{provider_name}'"
        ))),
    }
}

fn is_local_base(api_base: &str) -> bool {
    api_base.contains("localhost") || api_base.contains("127.0.0.1")
}

fn build_request(
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> Result<CompletionRequest> {
    if model.is_empty() {
        return Err(SimpleAgentsError::Config(
            "model cannot be empty".to_string(),
        ));
    }
    if prompt.is_empty() {
        return Err(SimpleAgentsError::Config(
            "prompt cannot be empty".to_string(),
        ));
    }

    let mut builder = CompletionRequest::builder()
        .model(model)
        .message(Message::user(prompt));

    if let Some(max_tokens) = max_tokens {
        builder = builder.max_tokens(max_tokens);
    }
    if let Some(temperature) = temperature {
        builder = builder.temperature(temperature);
    }

    builder.build()
}

fn build_request_with_messages(
    model: &str,
    messages: Vec<Message>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    response_format: Option<ResponseFormat>,
) -> Result<CompletionRequest> {
    if model.is_empty() {
        return Err(SimpleAgentsError::Config(
            "model cannot be empty".to_string(),
        ));
    }
    if messages.is_empty() {
        return Err(SimpleAgentsError::Config(
            "messages cannot be empty".to_string(),
        ));
    }

    let mut builder = CompletionRequest::builder().model(model);
    for message in messages {
        builder = builder.message(message);
    }

    if let Some(max_tokens) = max_tokens {
        builder = builder.max_tokens(max_tokens);
    }
    if let Some(temperature) = temperature {
        builder = builder.temperature(temperature);
    }
    if let Some(top_p) = top_p {
        builder = builder.top_p(top_p);
    }
    if let Some(format) = response_format {
        builder = builder.response_format(format);
    }

    builder.build()
}

fn parse_messages(messages: &Bound<'_, PyAny>) -> Result<Vec<Message>> {
    let list: &PyList = messages.downcast().map_err(|_| {
        SimpleAgentsError::Config("messages must be a list of dicts".to_string())
    })?;
    let mut result = Vec::with_capacity(list.len());

    for (idx, item) in list.iter().enumerate() {
        let dict: &PyDict = item
            .downcast()
            .map_err(|_| SimpleAgentsError::Config(format!("message[{idx}] must be a dict")))?;

        let role_obj = dict.get_item("role").ok_or_else(|| {
            SimpleAgentsError::Config(format!("message[{idx}] missing 'role'"))
        })?;
        let role: &str = role_obj.extract().map_err(|_| {
            SimpleAgentsError::Config(format!("message[{idx}].role must be a string"))
        })?;

        let content_obj = dict.get_item("content").ok_or_else(|| {
            SimpleAgentsError::Config(format!("message[{idx}] missing 'content'"))
        })?;
        let content: &str = content_obj.extract().map_err(|_| {
            SimpleAgentsError::Config(format!(
                "message[{idx}].content must be a string"
            ))
        })?;

        let mut message = match role {
            "user" => Message::user(content),
            "assistant" => Message::assistant(content),
            "system" => Message::system(content),
            "tool" => {
                let tool_call_id = dict
                    .get_item("tool_call_id")
                    .ok_or_else(|| {
                        SimpleAgentsError::Config(format!(
                            "message[{idx}] missing 'tool_call_id' for tool role"
                        ))
                    })?
                    .extract::<String>()
                    .map_err(|_| {
                        SimpleAgentsError::Config(format!(
                            "message[{idx}].tool_call_id must be a string"
                        ))
                    })?;
                Message::tool(content, tool_call_id)
            }
            _ => {
                return Err(SimpleAgentsError::Config(format!(
                    "message[{idx}].role must be one of: user, assistant, system, tool"
                )))
            }
        };

        if let Some(name_obj) = dict.get_item("name") {
            if !name_obj.is_none() {
                let name: String = name_obj.extract().map_err(|_| {
                    SimpleAgentsError::Config(format!("message[{idx}].name must be a string"))
                })?;
                message = message.with_name(name);
            }
        }

        result.push(message);
    }

    Ok(result)
}

fn py_err(error: SimpleAgentsError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

#[pymethods]
#[allow(clippy::useless_conversion)]
impl Client {
    #[new]
    #[pyo3(signature = (provider, api_key=None, api_base=None))]
    fn new(provider: &str, api_key: Option<String>, api_base: Option<String>) -> PyResult<Self> {
        let provider = provider_from_params(
            provider,
            api_key.as_deref(),
            api_base.as_deref(),
        )
        .map_err(py_err)?;
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(provider)
            .build()
            .map_err(py_err)?;
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            runtime: Mutex::new(runtime),
            client,
        })
    }

    #[pyo3(signature = (model, prompt, max_tokens=None, temperature=None))]
    fn complete(
        &self,
        model: &str,
        prompt: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> PyResult<String> {
        let request = build_request(model, prompt, max_tokens, temperature).map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }

    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn complete_messages(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<String> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            None,
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }

    #[pyo3(signature = (model, messages, max_tokens=None, temperature=None, top_p=None))]
    fn complete_json(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
    ) -> PyResult<String> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            Some(ResponseFormat::JsonObject),
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }

    #[pyo3(signature = (model, messages, schema, schema_name, max_tokens=None, temperature=None, top_p=None, strict=true))]
    fn complete_json_schema(
        &self,
        model: &str,
        messages: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        schema_name: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        strict: bool,
    ) -> PyResult<String> {
        let messages = parse_messages(messages).map_err(py_err)?;
        let schema_value: serde_json::Value = schema.extract().map_err(|_| {
            py_err(SimpleAgentsError::Config(
                "schema must be JSON-serializable".to_string(),
            ))
        })?;
        let response_format = ResponseFormat::JsonSchema {
            json_schema: JsonSchemaFormat {
                name: schema_name.to_string(),
                schema: schema_value,
                strict: Some(strict),
            },
        };
        let request = build_request_with_messages(
            model,
            messages,
            max_tokens,
            temperature,
            top_p,
            Some(response_format),
        )
        .map_err(py_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| PyRuntimeError::new_err("runtime lock poisoned"))?;
        let response = runtime
            .block_on(self.client.complete(&request))
            .map_err(py_err)?;

        Ok(response.content().unwrap_or_default().to_string())
    }
}

#[pymodule]
fn simple_agents_py(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Client>()?;
    Ok(())
}
