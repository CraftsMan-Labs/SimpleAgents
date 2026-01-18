//! OpenAI provider implementation.
//!
//! This module provides integration with the OpenAI API, supporting:
//! - GPT-4, GPT-3.5-Turbo, and other OpenAI models
//! - Streaming responses via Server-Sent Events (SSE)
//! - Function calling and vision capabilities
//! - Comprehensive error handling and retry logic

mod models;
mod error;

pub use models::*;
pub use error::OpenAIError;

use async_trait::async_trait;
use reqwest::Client;
use simple_agents_types::prelude::*;
use std::time::Duration;

/// OpenAI API provider
#[derive(Debug, Clone)]
pub struct OpenAIProvider {
    api_key: ApiKey,
    base_url: String,
    client: Client,
}

impl OpenAIProvider {
    /// Default OpenAI API base URL
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com/v1";

    /// Create a new OpenAI provider with default configuration
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key (starts with "sk-")
    ///
    /// # Errors
    ///
    /// Returns error if the HTTP client cannot be created
    pub fn new(api_key: ApiKey) -> Result<Self> {
        Self::with_base_url(api_key, Self::DEFAULT_BASE_URL.to_string())
    }

    /// Create a new OpenAI provider with custom base URL
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key
    /// * `base_url` - Custom base URL (e.g., for Azure OpenAI)
    pub fn with_base_url(api_key: ApiKey, base_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| SimpleAgentsError::Config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            api_key,
            base_url,
            client,
        })
    }

    /// Get the base URL for this provider
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(&self, req: &CompletionRequest) -> Result<ProviderRequest> {
        // Build OpenAI-specific request
        let openai_request = OpenAICompletionRequest {
            model: req.model.clone(),
            messages: req.messages.clone(),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            top_p: req.top_p,
            n: req.n,
            stream: Some(false),
            stop: req.stop.clone(),
        };

        let body = serde_json::to_value(&openai_request)?;

        Ok(ProviderRequest {
            url: format!("{}/chat/completions", self.base_url),
            headers: vec![
                ("Authorization".into(), format!("Bearer {}", self.api_key.expose())),
                ("Content-Type".into(), "application/json".into()),
            ],
            body,
            timeout: None,
        })
    }

    async fn execute(&self, req: ProviderRequest) -> Result<ProviderResponse> {
        // Build headers
        let headers = crate::utils::build_headers(req.headers)
            .map_err(|e| SimpleAgentsError::Config(format!("Invalid headers: {}", e)))?;

        // Make HTTP request
        let response = self.client
            .post(&req.url)
            .headers(headers)
            .json(&req.body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    SimpleAgentsError::Provider(ProviderError::Timeout(Duration::from_secs(30)))
                } else {
                    SimpleAgentsError::Network(format!("Network error: {}", e))
                }
            })?;

        let status = response.status();

        // Handle error responses
        if !status.is_success() {
            let error_body = response.text().await
                .unwrap_or_else(|_| "Failed to read error response".to_string());

            let openai_error = OpenAIError::from_response(status.as_u16(), &error_body);
            return Err(SimpleAgentsError::Provider(openai_error.into()));
        }

        // Parse successful response
        let body = response.json::<serde_json::Value>().await
            .map_err(|e| SimpleAgentsError::Provider(
                ProviderError::InvalidResponse(format!("Failed to parse JSON response: {}", e))
            ))?;

        Ok(ProviderResponse {
            status: status.as_u16(),
            body,
            headers: None,
        })
    }

    fn transform_response(&self, resp: ProviderResponse) -> Result<CompletionResponse> {
        // Parse OpenAI response
        let openai_response: OpenAICompletionResponse = serde_json::from_value(resp.body)
            .map_err(|e| SimpleAgentsError::Provider(
                ProviderError::InvalidResponse(format!("Failed to deserialize response: {}", e))
            ))?;

        // Transform choices to unified format
        let choices: Vec<CompletionChoice> = openai_response.choices.iter().map(|choice| {
            CompletionChoice {
                index: choice.index,
                message: choice.message.clone(),
                finish_reason: choice.finish_reason.as_ref()
                    .map(|s: &String| match s.as_str() {
                        "stop" => FinishReason::Stop,
                        "length" => FinishReason::Length,
                        "content_filter" => FinishReason::ContentFilter,
                        "tool_calls" => FinishReason::ToolCalls,
                        _ => FinishReason::Stop,
                    })
                    .unwrap_or(FinishReason::Stop),
                logprobs: None,
            }
        }).collect();

        Ok(CompletionResponse {
            id: openai_response.id,
            model: openai_response.model,
            choices,
            usage: Usage {
                prompt_tokens: openai_response.usage.prompt_tokens,
                completion_tokens: openai_response.usage.completion_tokens,
                total_tokens: openai_response.usage.total_tokens,
            },
            created: Some(openai_response.created as i64),
            provider: Some(self.name().to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.base_url(), OpenAIProvider::DEFAULT_BASE_URL);
    }

    #[test]
    fn test_transform_request() {
        let api_key = ApiKey::new("sk-test1234567890123456789012345678901234567890").unwrap();
        let provider = OpenAIProvider::new(api_key).unwrap();

        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .temperature(0.7)
            .build()
            .unwrap();

        let provider_request = provider.transform_request(&request).unwrap();

        assert_eq!(provider_request.url, "https://api.openai.com/v1/chat/completions");
        assert!(provider_request.headers.iter().any(|(k, _)| k == "Authorization"));
        assert!(provider_request.body["model"] == "gpt-4");
    }
}
