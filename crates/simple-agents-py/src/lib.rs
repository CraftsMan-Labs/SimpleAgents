//! Python bindings for SimpleAgents using PyO3.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use simple_agents_core::{SimpleAgentsClient, SimpleAgentsClientBuilder};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use simple_agents_types::message::Message;
use simple_agents_types::prelude::{CompletionRequest, Provider, Result, SimpleAgentsError};
use std::sync::{Arc, Mutex};

type Runtime = tokio::runtime::Runtime;

#[pyclass]
struct Client {
    runtime: Mutex<Runtime>,
    client: SimpleAgentsClient,
}

fn provider_from_env(provider_name: &str) -> Result<Arc<dyn Provider>> {
    match provider_name {
        "openai" => Ok(Arc::new(OpenAIProvider::from_env()?)),
        "anthropic" => Ok(Arc::new(AnthropicProvider::from_env()?)),
        "openrouter" => Ok(Arc::new(OpenRouterProvider::from_env()?)),
        _ => Err(SimpleAgentsError::Config(format!(
            "Unknown provider '{provider_name}'"
        ))),
    }
}

fn build_request(
    model: &str,
    prompt: &str,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> Result<CompletionRequest> {
    if model.is_empty() {
        return Err(SimpleAgentsError::Config("model cannot be empty".to_string()));
    }
    if prompt.is_empty() {
        return Err(SimpleAgentsError::Config("prompt cannot be empty".to_string()));
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
impl Client {
    #[new]
    fn new(provider: &str) -> PyResult<Self> {
        let provider = provider_from_env(provider).map_err(py_err)?;
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
