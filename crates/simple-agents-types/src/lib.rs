//! Core types and traits for SimpleAgents.
//!
//! This crate provides all foundational types, traits, and error definitions
//! shared across the SimpleAgents project. It is a pure types crate with no
//! runtime, HTTP, or I/O dependencies.
//!
//! # Architecture
//!
//! SimpleAgents follows a trait-based architecture:
//!
//! - **Provider**: Trait for LLM provider implementations
//! - **Cache**: Trait for caching responses
//! - **RoutingStrategy**: Trait for provider selection
//!
//! # Main Types
//!
//! - [`Message`]: Role-based conversation messages
//! - [`CompletionRequest`]: Unified request format
//! - [`CompletionResponse`]: Unified response format
//! - [`ApiKey`]: Secure API key handling
//!
//! # Example
//!
//! ```
//! use simple_agents_types::prelude::*;
//!
//! // Create a request
//! let request = CompletionRequest::builder()
//!     .model("gpt-4")
//!     .message(Message::user("Hello!"))
//!     .temperature(0.7)
//!     .build()
//!     .unwrap();
//!
//! // Access request properties
//! assert_eq!(request.model, "gpt-4");
//! assert_eq!(request.messages.len(), 1);
//! ```
//!
//! # Features
//!
//! - **Type Safety**: Strong types prevent common errors
//! - **Transparency**: All transformations tracked via [`CoercionFlag`]
//! - **Security**: API keys never logged ([`ApiKey`])
//! - **Validation**: Early validation with clear errors
//! - **Async**: All traits use `async_trait`

#![deny(missing_docs)]
#![deny(unsafe_code)]

// Core modules
pub mod cache;
pub mod coercion;
pub mod config;
pub mod error;
pub mod message;
pub mod provider;
pub mod request;
pub mod response;
pub mod router;
pub mod validation;

// Re-export commonly used types at crate root
pub use error::{HealingError, ProviderError, Result, SimpleAgentsError, ValidationError};

/// Prelude module for convenient imports.
///
/// # Example
/// ```
/// use simple_agents_types::prelude::*;
///
/// let msg = Message::user("Hello!");
/// let request = CompletionRequest::builder()
///     .model("gpt-4")
///     .message(msg)
///     .build()
///     .unwrap();
/// ```
pub mod prelude {
    // Messages
    pub use crate::message::{Message, Role};

    // Requests and responses
    pub use crate::request::{CompletionRequest, CompletionRequestBuilder};
    pub use crate::response::{
        ChoiceDelta, CompletionChoice, CompletionChunk, CompletionResponse, FinishReason,
        MessageDelta, Usage,
    };

    // Errors
    pub use crate::error::{
        HealingError, ProviderError, Result, SimpleAgentsError, ValidationError,
    };

    // Validation
    pub use crate::validation::ApiKey;

    // Configuration
    pub use crate::config::{Capabilities, HealingConfig, ProviderConfig, RetryConfig};

    // Coercion
    pub use crate::coercion::{CoercionFlag, CoercionResult};

    // Traits
    pub use crate::cache::Cache;
    pub use crate::provider::Provider;
    pub use crate::router::RoutingStrategy;

    // Provider types
    pub use crate::provider::{ProviderRequest, ProviderResponse};

    // Router types
    pub use crate::router::{ProviderHealth, ProviderMetrics, RoutingMode};
}

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn test_prelude_imports() {
        // Test that all major types are importable
        let _msg = Message::user("test");
        let _request = CompletionRequest::builder()
            .model("test")
            .message(Message::user("test"))
            .build()
            .unwrap();
    }

    #[test]
    fn test_error_conversion() {
        let validation_err = ValidationError::new("test");
        let _agents_err: SimpleAgentsError = validation_err.into();
    }

    #[test]
    fn test_api_key_security() {
        let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let debug = format!("{:?}", key);
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn test_coercion_transparency() {
        let result = CoercionResult::new(42).with_flag(CoercionFlag::StrippedMarkdown);
        assert!(result.was_coerced());
        assert_eq!(result.flags.len(), 1);
    }

    #[test]
    fn test_builder_pattern() {
        let request = CompletionRequest::builder()
            .model("gpt-4")
            .message(Message::user("Hello"))
            .temperature(0.7)
            .build()
            .unwrap();

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_response_helper() {
        let response = CompletionResponse {
            id: "resp_123".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                message: Message::assistant("Hello!"),
                finish_reason: FinishReason::Stop,
                logprobs: None,
            }],
            usage: Usage::new(10, 5),
            created: None,
            provider: None,
        };

        assert_eq!(response.content(), Some("Hello!"));
    }

    #[test]
    fn test_all_types_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        // Core types
        assert_send_sync::<Message>();
        assert_send_sync::<CompletionRequest>();
        assert_send_sync::<CompletionResponse>();

        // Config types
        assert_send_sync::<RetryConfig>();
        assert_send_sync::<HealingConfig>();
        assert_send_sync::<Capabilities>();

        // Coercion types
        assert_send_sync::<CoercionFlag>();
        assert_send_sync::<CoercionResult<String>>();

        // Provider types
        assert_send_sync::<ProviderRequest>();
        assert_send_sync::<ProviderResponse>();
    }
}
