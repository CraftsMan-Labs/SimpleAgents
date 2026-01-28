//! Python bindings for SimpleAgents using PyO3.

#![allow(clippy::useless_conversion)]

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use reqwest::Client as HttpClient;
use simple_agents_core::{SimpleAgentsClient, SimpleAgentsClientBuilder};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{ApiKey, CompletionRequest, Provider, Result, SimpleAgentsError};
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
}

#[pymodule]
fn simple_agents_py(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Client>()?;
    Ok(())
}
