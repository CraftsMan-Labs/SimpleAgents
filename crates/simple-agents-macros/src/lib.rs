//! Procedural macros for SimpleAgents.
//!
//! This crate provides derive macros for automatic schema generation and partial type
//! support for streaming responses.
//!
//! # Macros
//!
//! - [`PartialType`] - Generates a partial version of a type with all fields as `Option<T>`
//!
//! # Example
//!
//! ```rust
//! use simple_agents_macros::PartialType;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, PartialType, Serialize, Deserialize)]
//! pub struct User {
//!     pub id: u64,
//!     pub name: String,
//!     pub email: String,
//!     pub age: u32,
//! }
//!
//! let partial = PartialUser {
//!     id: Some(1),
//!     name: Some("Ada".to_string()),
//!     email: Some("ada@example.com".to_string()),
//!     age: Some(42),
//! };
//! let user = User::from_partial(partial).unwrap();
//! assert_eq!(user.name, "Ada");
//! ```

#![deny(missing_docs)]

mod partial;

use proc_macro::TokenStream;

/// Derives a partial type for streaming support.
///
/// Generates a `Partial{TypeName}` struct with all fields wrapped in `Option<T>`.
/// The partial type is useful for progressive emission during streaming responses,
/// where not all fields may be available yet.
///
/// # Generated Code
///
/// For each struct annotated with `#[derive(PartialType)]`, this macro generates:
///
/// 1. **Partial Struct**: `Partial{TypeName}` with all fields as `Option<T>`
///    - Derives: `Debug`, `Clone`, `Default`, `Serialize`, `Deserialize`
///
/// 2. **Conversion Method**: `from_partial()` on the original type
///    - Converts `PartialType` → `Type`
///    - Returns `Result<Type, String>` (error if required fields missing)
///
/// 3. **Merge Method**: `merge()` on the partial type
///    - Merges two partial values (newer values take precedence)
///
/// # Field Attributes
///
/// - `#[partial(skip)]` - Exclude field from partial type (always uses default)
/// - `#[partial(default)]` - Use default value if missing in partial
///
/// # Example
///
/// ```rust
/// use simple_agents_macros::PartialType;
/// use std::time::Duration;
///
/// #[derive(PartialType)]
/// pub struct Resume {
///     pub name: String,
///     pub email: String,
///     #[partial(default)]
///     pub skills: Vec<String>,
///     #[partial(skip)]
///     pub created_at: Duration,
/// }
///
/// let mut partial = PartialResume::default();
/// partial.name = Some("Alice".to_string());
/// assert_eq!(partial.email, None);
///
/// partial.merge(PartialResume {
///     email: Some("alice@example.com".to_string()),
///     skills: Some(vec!["Rust".to_string()]),
///     ..Default::default()
/// });
///
/// let resume = Resume::from_partial(partial).unwrap();
/// assert_eq!(resume.name, "Alice");
/// assert_eq!(resume.skills, vec!["Rust".to_string()]);
/// ```
///
/// # Streaming Annotations
///
/// Future versions will support streaming annotations:
/// - `#[partial(stream_not_null)]` - Don't emit until non-null
/// - `#[partial(stream_done)]` - Only emit when complete
#[proc_macro_derive(PartialType, attributes(partial))]
pub fn derive_partial_type(input: TokenStream) -> TokenStream {
    partial::derive(input)
}
