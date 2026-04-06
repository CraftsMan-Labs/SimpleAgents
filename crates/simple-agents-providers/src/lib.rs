//! Provider implementations for SimpleAgents.
//!
//! This crate provides concrete implementations of LLM providers that integrate
//! with the SimpleAgents framework. Each provider handles the specifics of
//! transforming requests, making HTTP calls, and parsing responses for different
//! LLM APIs.
//!
//! # Supported Providers
//!
//! - [`openai`]: OpenAI-compatible API (GPT-4, GPT-3.5-Turbo, etc.)
//!
//! # Examples
//!
//! ```no_run
//! use simple_agents_providers::openai::OpenAiCompatProvider;
//! use simple_agent_type::prelude::*;
//!
//! # async fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! let api_key = ApiKey::new("sk-...")?;
//! let provider = OpenAiCompatProvider::new(api_key)?;
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

pub mod common;
pub mod healing_integration;
pub mod openai;
pub mod responses;
pub mod schema_converter;
mod utils;

// Re-export the renamed provider
pub use openai::OpenAiCompatProvider;

// Re-export common utilities
pub use common::{HttpClient, ProviderError, RetryableError};

// Re-export Provider trait and types from simple-agent-type
pub use simple_agent_type::prelude::{Provider, ProviderRequest, ProviderResponse};

/// Result type for provider operations.
pub type Result<T> = std::result::Result<T, ProviderError>;
