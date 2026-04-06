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

    let format = match api_format {
        Some("responses") => ApiFormat::Responses,
        Some("chat_completions") | None => ApiFormat::ChatCompletions,
        Some(other) => {
            return Err(SimpleAgentsError::Config(format!(
                "unknown api_format '{other}'; expected 'chat_completions' or 'responses'"
            )));
        }
    };

    let provider = match base_url {
        Some(url) => OpenAiCompatProvider::with_base_url_and_format(key, url.to_string(), format)?,
        None => OpenAiCompatProvider::new_with_format(key, format)?,
    };

    Ok(Arc::new(provider))
}
