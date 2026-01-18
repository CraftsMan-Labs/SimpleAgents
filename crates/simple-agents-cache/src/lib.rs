//! Cache implementations for SimpleAgents.
//!
//! Provides various caching strategies for LLM responses.

mod memory;
mod noop;

pub use memory::InMemoryCache;
pub use noop::NoOpCache;

// Re-export the Cache trait
pub use simple_agents_types::cache::Cache;
