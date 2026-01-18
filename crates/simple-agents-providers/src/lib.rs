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
//!
//! # Examples
//!
//! ```no_run
//! use simple_agents_providers::openai::OpenAIProvider;
//! use simple_agents_types::prelude::*;
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

pub mod openai;
pub mod anthropic;
pub mod retry;
mod utils;

// Re-export common types from simple-agents-types
pub use simple_agents_types::prelude::{Provider, ProviderRequest, ProviderResponse};
