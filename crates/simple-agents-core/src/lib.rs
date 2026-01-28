//! Core client API for SimpleAgents.
//!
//! This crate provides the unified `SimpleAgentsClient` that integrates
//! providers, routing, caching, healing, and middleware in one place.
//!
//! # Example
//! ```no_run
//! use simple_agents_core::{SimpleAgentsClient, SimpleAgentsClientBuilder, RoutingMode};
//! use simple_agent_type::prelude::*;
//! # use async_trait::async_trait;
//! # use std::sync::Arc;
//! #
//! # struct MockProvider;
//! #
//! # #[async_trait]
//! # impl Provider for MockProvider {
//! #     fn name(&self) -> &str { "mock" }
//! #     fn transform_request(&self, _req: &CompletionRequest) -> Result<ProviderRequest> {
//! #         Ok(ProviderRequest::new("http://example.com"))
//! #     }
//! #     async fn execute(&self, _req: ProviderRequest) -> Result<ProviderResponse> {
//! #         Ok(ProviderResponse::new(200, serde_json::json!({"ok": true})))
//! #     }
//! #     fn transform_response(&self, _resp: ProviderResponse) -> Result<CompletionResponse> {
//! #         Ok(CompletionResponse {
//! #             id: "resp_1".to_string(),
//! #             model: "test".to_string(),
//! #             choices: vec![CompletionChoice {
//! #                 index: 0,
//! #                 message: Message::assistant("ok"),
//! #                 finish_reason: FinishReason::Stop,
//! #                 logprobs: None,
//! #             }],
//! #             usage: Usage::new(1, 1),
//! #             created: None,
//! #             provider: Some("mock".to_string()),
//! #             healing_metadata: None,
//! #         })
//! #     }
//! # }
//! #
//! # async fn example() -> Result<()> {
//! let client = SimpleAgentsClientBuilder::new()
//!     .with_provider(Arc::new(MockProvider))
//!     .with_routing_mode(RoutingMode::RoundRobin)
//!     .build()?;
//!
//! let request = CompletionRequest::builder()
//!     .model("gpt-4")
//!     .message(Message::user("Hello!"))
//!     .build()?;
//!
//! let response = client.complete(&request).await?;
//! println!("{}", response.content().unwrap_or_default());
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]

mod client;
mod healing;
mod middleware;
mod routing;

pub use client::{SimpleAgentsClient, SimpleAgentsClientBuilder};
pub use healing::{HealedJsonResponse, HealedSchemaResponse, HealingSettings};
pub use middleware::Middleware;
pub use routing::RoutingMode;

// Re-export commonly used types.
pub use simple_agent_type::prelude;
