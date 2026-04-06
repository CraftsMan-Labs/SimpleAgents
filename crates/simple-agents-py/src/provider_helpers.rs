use simple_agent_type::prelude::{ApiKey, Provider, Result, SimpleAgentsError};
use simple_agent_type::telemetry::ApiFormat;
use simple_agents_providers::openai::OpenAiCompatProvider;
use std::sync::Arc;

pub(crate) fn build_provider(
    api_key: &str,
    base_url: Option<&str>,
    api_format: Option<&str>,
) -> Result<Arc<dyn Provider>> {
    let key = ApiKey::new(api_key)?;
    let format = parse_api_format(api_format)?;
    let provider = match base_url {
        Some(url) => OpenAiCompatProvider::with_base_url_and_format(key, url.to_string(), format)?,
        None => OpenAiCompatProvider::new_with_format(key, format)?,
    };
    Ok(Arc::new(provider))
}

/// Build a provider using the provider-name API.
///
/// `provider` must be "openai" (only supported name). `api_key` is used if
/// provided; otherwise `OPENAI_API_KEY` env var is read. `base_url` overrides
/// the default endpoint. Any other provider name returns "Unknown provider: X".
pub(crate) fn build_provider_from_name(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    api_format: Option<&str>,
) -> Result<Arc<dyn Provider>> {
    match provider {
        "openai" => {
            let key = if let Some(k) = api_key {
                k.to_string()
            } else {
                std::env::var("OPENAI_API_KEY").map_err(|_| {
                    SimpleAgentsError::Config(
                        "OPENAI_API_KEY environment variable not set".to_string(),
                    )
                })?
            };
            build_provider(&key, base_url, api_format)
        }
        other => Err(SimpleAgentsError::Config(format!(
            "Unknown provider: {other}"
        ))),
    }
}

fn parse_api_format(api_format: Option<&str>) -> Result<ApiFormat> {
    match api_format {
        Some("responses") => Ok(ApiFormat::Responses),
        Some("chat_completions") | None => Ok(ApiFormat::ChatCompletions),
        Some(other) => Err(SimpleAgentsError::Config(format!(
            "unknown api_format '{other}'; expected 'chat_completions' or 'responses'"
        ))),
    }
}
