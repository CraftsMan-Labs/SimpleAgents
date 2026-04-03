use reqwest::Client as HttpClient;
use simple_agent_type::prelude::{ApiKey, Provider, Result, SimpleAgentsError};
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::healing_integration::HealingConfig as ProviderHealingConfig;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use std::sync::Arc;
use std::time::Duration;

pub(crate) fn provider_name_exists(providers: &[Arc<dyn Provider>], name: &str) -> bool {
    providers.iter().any(|p| p.name() == name)
}

pub(crate) fn provider_from_params(
    provider_name: &str,
    api_key: Option<&str>,
    api_base: Option<&str>,
    enable_healing: bool,
    timeout: Duration,
) -> Result<Arc<dyn Provider>> {
    let api_key = match api_key {
        Some(value) => Some(ApiKey::new(value)?),
        None => None,
    };

    match provider_name {
        "openai" => {
            let mut provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        if is_local_base(api_base) {
                            let client = HttpClient::builder()
                                .timeout(timeout)
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
            if enable_healing {
                provider = provider.with_healing(ProviderHealingConfig::default());
            }
            Ok(Arc::new(provider))
        }
        "anthropic" => {
            let mut provider = match api_key {
                Some(api_key) => match api_base {
                    Some(api_base) => {
                        AnthropicProvider::with_base_url(api_key, api_base.to_string())?
                    }
                    None => AnthropicProvider::new(api_key)?,
                },
                None => AnthropicProvider::from_env()?,
            };
            if enable_healing {
                provider = provider.with_healing(ProviderHealingConfig::default());
            }
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
