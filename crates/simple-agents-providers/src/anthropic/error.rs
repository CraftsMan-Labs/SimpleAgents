//! Anthropic-specific error handling.
//!
//! Implementation coming in Phase 2.

use thiserror::Error;

/// Anthropic-specific errors
#[derive(Error, Debug)]
pub enum AnthropicError {
    /// Placeholder error
    #[error("Not implemented")]
    NotImplemented,
}
