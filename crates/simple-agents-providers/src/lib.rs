//! Provider implementations for SimpleAgents.
//!
//! This crate provides concrete implementations of LLM providers that integrate
//! with the SimpleAgents framework. Each provider handles the specifics of
//! transforming requests, making HTTP calls, and parsing responses for different
//! LLM APIs.
//!
//! # Supported Providers
//!
//! - [`openai`]: OpenAI API (GPT-4, GPT-3.5-Turbo, etc.)
//! - [`anthropic`]: Anthropic API (Claude 3 Opus, Sonnet, Haiku)
//! - [`openrouter`]: OpenRouter API (unified access to open-source models)
//!
//! # Examples
//!
//! ```no_run
//! use simple_agents_providers::openai::OpenAIProvider;
//! use simple_agent_type::prelude::*;
//!
//! # async fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! let api_key = ApiKey::new("sk-...")?;
//! let provider = OpenAIProvider::new(api_key)?;
//!
//! let request = CompletionRequest::builder()
//!     .model("gpt-4")
//!     .message(Message::user("Hello!"))
//!     .build()?;
//!
//! let provider_request = provider.transform_request(&request)?;
//! let provider_response = provider.execute(provider_request).await?;
//! let response = provider.transform_response(provider_response)?;
//!
//! println!("{}", response.content().unwrap_or(""));
//! # Ok(())
//! # }
//! ```

pub mod anthropic;
pub mod common;
pub mod healing_integration;
pub mod metrics;
pub mod openai;
pub mod openrouter;
pub mod rate_limit;
pub mod retry;
pub mod schema_converter;
pub mod streaming_structured;
mod utils;

// Re-export common utilities
pub use common::{HttpClient, ProviderError, RetryableError};

// Re-export Provider trait and types from simple-agent-type
pub use simple_agent_type::prelude::{Provider, ProviderRequest, ProviderResponse};

use async_trait::async_trait;
use futures_core::Stream;
use serde::de::DeserializeOwned;
use simple_agent_type::error::SimpleAgentsError;
use simple_agent_type::request::CompletionRequest;
use simple_agent_type::response::CompletionChunk;

/// Result type for provider operations.
pub type Result<T> = std::result::Result<T, ProviderError>;

/// Extension trait for providers with structured streaming support.
///
/// This trait provides methods for streaming structured outputs with
/// progressive parsing and automatic healing.
#[async_trait]
pub trait ProviderStructuredExt: Provider {
    /// Execute a streaming request with structured output parsing.
    ///
    /// Returns a stream that accumulates chunks and provides structured
    /// events (partial updates and final complete value).
    ///
    /// # Example
    /// ```ignore
    /// use simple_agents_providers::ProviderStructuredExt;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Deserialize, Serialize)]
    /// struct MyData {
    ///     field: String,
    /// }
    ///
    /// let stream = provider.execute_stream_structured::<MyData>(request).await?;
    /// ```
    async fn execute_stream_structured<T>(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<
        streaming_structured::StructuredStream<
            impl Stream<Item = std::result::Result<CompletionChunk, SimpleAgentsError>> + Send,
            T,
        >,
        SimpleAgentsError,
    >
    where
        T: DeserializeOwned + Send + 'static;
}
