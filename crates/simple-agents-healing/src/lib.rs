//! Response healing system for SimpleAgents.
//!
//! Implements BAML-inspired JSON parsing and type coercion to handle malformed LLM outputs.
//!
//! # Architecture
//!
//! The healing system consists of three main components:
//!
//! 1. **Jsonish Parser**: Three-phase parsing strategy that handles malformed JSON
//!    - Strip & Fix: Remove markdown, fix commas, normalize quotes
//!    - Standard Parse: Try serde_json (fast path)
//!    - Lenient Parse: Character-by-character state machine
//!
//! 2. **Coercion Engine**: Type coercion with confidence scoring
//!    - String → Number coercion
//!    - Fuzzy field matching (case-insensitive, snake_case ↔ camelCase)
//!    - Union resolution with best-match selection
//!    - Default value injection
//!
//! 3. **Streaming Parser**: Incremental parsing for streaming responses
//!    - Partial value extraction
//!    - Progressive emission during streaming
//!    - Annotation support (stream.not_null, stream.done)
//!
//! # Example
//!
//! ```
//! use simple_agents_healing::parser::JsonishParser;
//!
//! let malformed = r#"```json
//! {"name": "test", "age": 25,}
//! ```"#;
//!
//! let parser = JsonishParser::new();
//! let result = parser.parse(malformed).unwrap();
//!
//! assert_eq!(result.value["name"], "test");
//! assert_eq!(result.value["age"], 25);
//! assert!(result.flags.iter().any(|f| matches!(f,
//!     simple_agent_type::coercion::CoercionFlag::StrippedMarkdown)));
//! ```
//!
//! # Transparency
//!
//! All transformations are tracked via [`CoercionFlag`]s and assigned confidence scores:
//!
//! - **1.0**: Perfect parse, no healing needed
//! - **0.9-0.99**: Minor fixes (markdown, trailing commas)
//! - **0.7-0.89**: Type coercion or fuzzy field matching
//! - **0.5-0.69**: Multiple coercions or truncation
//! - **<0.5**: Significant healing required, review recommended
//!
//! [`CoercionFlag`]: simple_agent_type::coercion::CoercionFlag

#![deny(missing_docs)]
#![deny(unsafe_code)]

// Public modules
pub mod coercion;
pub mod parser;
pub mod schema;
pub mod streaming;
pub mod string_utils;

// Re-export commonly used types
pub use coercion::{CoercionConfig, CoercionEngine};
pub use parser::{JsonishParser, ParserConfig, ParserResult};
pub use schema::{Field, ObjectSchema, Schema, StreamAnnotation};
pub use streaming::{PartialExtractor, StreamingParser};

// Re-export from simple-agent-type for convenience
pub use simple_agent_type::coercion::{CoercionFlag, CoercionResult};
pub use simple_agent_type::error::{HealingError, Result};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::coercion::{CoercionConfig, CoercionEngine};
    pub use crate::parser::{JsonishParser, ParserConfig, ParserResult};
    pub use crate::schema::{Field, ObjectSchema, Schema, StreamAnnotation};
    pub use crate::streaming::{PartialExtractor, StreamingParser};
    pub use simple_agent_type::coercion::{CoercionFlag, CoercionResult};
    pub use simple_agent_type::error::{HealingError, Result};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_prelude_imports() {
        use crate::prelude::*;

        // Verify all types are importable
        let _parser = JsonishParser::new();
        let _result = CoercionResult::new(42);
    }
}
