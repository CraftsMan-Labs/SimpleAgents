//! Node.js bindings for SimpleAgents using napi-rs.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use simple_agents_core::{
    CompletionOptions, CompletionOutcome, SimpleAgentsClient, SimpleAgentsClientBuilder,
};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use simple_agent_type::message::Message;
use simple_agent_type::prelude::{
    CompletionRequest, Provider, Result as SaResult, SimpleAgentsError,
};
use std::sync::{Arc, Mutex};

type Runtime = tokio::runtime::Runtime;

fn provider_from_env(provider_name: &str) -> SaResult<Arc<dyn Provider>> {
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
    temperature: Option<f64>,
) -> SaResult<CompletionRequest> {
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
        builder = builder.temperature(temperature as f32);
    }

    builder.build()
}

fn napi_err(error: SimpleAgentsError) -> Error {
    Error::from_reason(error.to_string())
}

#[napi]
pub struct Client {
    runtime: Mutex<Runtime>,
    client: SimpleAgentsClient,
}

#[napi]
impl Client {
    #[napi(constructor)]
    pub fn new(provider: String) -> Result<Self> {
        let provider = provider_from_env(&provider).map_err(napi_err)?;
        let client = SimpleAgentsClientBuilder::new()
            .with_provider(provider)
            .build()
            .map_err(napi_err)?;
        let runtime = Runtime::new().map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Self {
            runtime: Mutex::new(runtime),
            client,
        })
    }

    #[napi]
    pub fn complete(
        &self,
        model: String,
        prompt: String,
        max_tokens: Option<u32>,
        temperature: Option<f64>,
    ) -> Result<String> {
        let request = build_request(&model, &prompt, max_tokens, temperature).map_err(napi_err)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| Error::from_reason("runtime lock poisoned"))?;
        let outcome = runtime
            .block_on(self.client.complete(&request, CompletionOptions::default()))
            .map_err(napi_err)?;
        let response = match outcome {
            CompletionOutcome::Response(response) => response,
            CompletionOutcome::Stream(_) => {
                return Err(Error::from_reason(
                    "streaming response returned from complete".to_string(),
                ))
            }
            CompletionOutcome::HealedJson(_) => {
                return Err(Error::from_reason(
                    "healed json response returned from complete".to_string(),
                ))
            }
            CompletionOutcome::CoercedSchema(_) => {
                return Err(Error::from_reason(
                    "schema response returned from complete".to_string(),
                ))
            }
        };

        Ok(response.content().unwrap_or_default().to_string())
    }
}
