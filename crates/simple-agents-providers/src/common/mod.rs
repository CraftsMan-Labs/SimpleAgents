//! Common utilities shared across providers.

pub mod error;
pub mod http_client;

pub use error::{RetryableError, TransportError};
pub use http_client::HttpClient;
