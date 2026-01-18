//! Coercion types for tracking response healing.
//!
//! Provides transparency into how LLM responses were transformed.

use serde::{Deserialize, Serialize};

/// Flag indicating a specific coercion/healing operation.
///
/// These flags provide full transparency into how a response was modified
/// to make it parseable or conform to expected types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoercionFlag {
    /// Stripped markdown code fences
    StrippedMarkdown,

    /// Fixed trailing comma in JSON
    FixedTrailingComma,

    /// Fixed mismatched quotes
    FixedQuotes,

    /// Matched field name using fuzzy matching
    FuzzyFieldMatch {
        /// Expected field name
        expected: String,
        /// Actual field name found
        found: String,
    },

    /// Coerced value from one type to another
    TypeCoercion {
        /// Source type
        from: String,
        /// Target type
        to: String,
    },

    /// Used default value for missing field
    UsedDefaultValue {
        /// Field name
        field: String,
    },

    /// Truncated malformed JSON
    TruncatedJson,

    /// Fixed unquoted object keys
    FixedUnquotedKeys,

    /// Fixed control characters
    FixedControlCharacters,

    /// Removed BOM (byte order mark)
    RemovedBom,
}

impl CoercionFlag {
    /// Get a human-readable description of this coercion.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::coercion::CoercionFlag;
    ///
    /// let flag = CoercionFlag::StrippedMarkdown;
    /// assert_eq!(flag.description(), "Stripped markdown code fences");
    ///
    /// let flag = CoercionFlag::TypeCoercion {
    ///     from: "string".to_string(),
    ///     to: "number".to_string(),
    /// };
    /// assert!(flag.description().contains("string"));
    /// assert!(flag.description().contains("number"));
    /// ```
    pub fn description(&self) -> String {
        match self {
            Self::StrippedMarkdown => "Stripped markdown code fences".to_string(),
            Self::FixedTrailingComma => "Fixed trailing comma in JSON".to_string(),
            Self::FixedQuotes => "Fixed mismatched quotes".to_string(),
            Self::FuzzyFieldMatch { expected, found } => {
                format!("Matched field '{}' as '{}'", found, expected)
            }
            Self::TypeCoercion { from, to } => {
                format!("Coerced type from {} to {}", from, to)
            }
            Self::UsedDefaultValue { field } => {
                format!("Used default value for field '{}'", field)
            }
            Self::TruncatedJson => "Truncated malformed JSON".to_string(),
            Self::FixedUnquotedKeys => "Fixed unquoted object keys".to_string(),
            Self::FixedControlCharacters => "Fixed control characters".to_string(),
            Self::RemovedBom => "Removed byte order mark (BOM)".to_string(),
        }
    }

    /// Check if this coercion is considered "major" (potentially changes semantics).
    pub fn is_major(&self) -> bool {
        matches!(
            self,
            Self::TypeCoercion { .. }
                | Self::UsedDefaultValue { .. }
                | Self::TruncatedJson
                | Self::FuzzyFieldMatch { .. }
        )
    }
}

/// Result of a coercion operation with transparency.
///
/// Tracks the value, all coercions applied, and confidence score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoercionResult<T> {
    /// The coerced value
    pub value: T,
    /// All coercion flags applied
    pub flags: Vec<CoercionFlag>,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
}

impl<T> CoercionResult<T> {
    /// Create a new coercion result with perfect confidence.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::coercion::CoercionResult;
    ///
    /// let result = CoercionResult::new(42);
    /// assert_eq!(result.value, 42);
    /// assert_eq!(result.confidence, 1.0);
    /// assert!(result.flags.is_empty());
    /// ```
    pub fn new(value: T) -> Self {
        Self {
            value,
            flags: Vec::new(),
            confidence: 1.0,
        }
    }

    /// Create a result with a specific confidence.
    pub fn with_confidence(value: T, confidence: f32) -> Self {
        Self {
            value,
            flags: Vec::new(),
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Set the confidence score (builder pattern).
    pub fn set_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add a coercion flag.
    pub fn with_flag(mut self, flag: CoercionFlag) -> Self {
        self.flags.push(flag);
        self
    }

    /// Add multiple coercion flags.
    pub fn with_flags(mut self, flags: Vec<CoercionFlag>) -> Self {
        self.flags.extend(flags);
        self
    }

    /// Check if any coercions were applied.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::coercion::{CoercionResult, CoercionFlag};
    ///
    /// let result = CoercionResult::new(42);
    /// assert!(!result.was_coerced());
    ///
    /// let result = result.with_flag(CoercionFlag::StrippedMarkdown);
    /// assert!(result.was_coerced());
    /// ```
    pub fn was_coerced(&self) -> bool {
        !self.flags.is_empty()
    }

    /// Check if confidence meets a threshold.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::coercion::CoercionResult;
    ///
    /// let result = CoercionResult::with_confidence(42, 0.8);
    /// assert!(result.is_confident(0.7));
    /// assert!(!result.is_confident(0.9));
    /// ```
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }

    /// Check if any major coercions were applied.
    pub fn has_major_coercions(&self) -> bool {
        self.flags.iter().any(|f| f.is_major())
    }

    /// Map the value while preserving flags and confidence.
    pub fn map<U, F>(self, f: F) -> CoercionResult<U>
    where
        F: FnOnce(T) -> U,
    {
        CoercionResult {
            value: f(self.value),
            flags: self.flags,
            confidence: self.confidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coercion_flag_description() {
        let flag = CoercionFlag::StrippedMarkdown;
        assert!(!flag.description().is_empty());

        let flag = CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "number".to_string(),
        };
        assert!(flag.description().contains("string"));
        assert!(flag.description().contains("number"));
    }

    #[test]
    fn test_coercion_flag_is_major() {
        assert!(!CoercionFlag::StrippedMarkdown.is_major());
        assert!(!CoercionFlag::FixedTrailingComma.is_major());
        assert!(!CoercionFlag::FixedQuotes.is_major());

        assert!(CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "number".to_string(),
        }
        .is_major());

        assert!(CoercionFlag::UsedDefaultValue {
            field: "test".to_string()
        }
        .is_major());

        assert!(CoercionFlag::TruncatedJson.is_major());
    }

    #[test]
    fn test_coercion_result_new() {
        let result = CoercionResult::new(42);
        assert_eq!(result.value, 42);
        assert_eq!(result.confidence, 1.0);
        assert!(result.flags.is_empty());
        assert!(!result.was_coerced());
    }

    #[test]
    fn test_coercion_result_with_confidence() {
        let result = CoercionResult::with_confidence(42, 0.8);
        assert_eq!(result.value, 42);
        assert_eq!(result.confidence, 0.8);
        assert!(result.is_confident(0.7));
        assert!(!result.is_confident(0.9));
    }

    #[test]
    fn test_coercion_result_confidence_clamped() {
        let result = CoercionResult::with_confidence(42, 1.5);
        assert_eq!(result.confidence, 1.0);

        let result = CoercionResult::with_confidence(42, -0.5);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_coercion_result_with_flag() {
        let result = CoercionResult::new(42).with_flag(CoercionFlag::StrippedMarkdown);
        assert!(result.was_coerced());
        assert_eq!(result.flags.len(), 1);
    }

    #[test]
    fn test_coercion_result_with_flags() {
        let flags = vec![
            CoercionFlag::StrippedMarkdown,
            CoercionFlag::FixedTrailingComma,
        ];
        let result = CoercionResult::new(42).with_flags(flags);
        assert_eq!(result.flags.len(), 2);
    }

    #[test]
    fn test_coercion_result_has_major_coercions() {
        let result = CoercionResult::new(42).with_flag(CoercionFlag::StrippedMarkdown);
        assert!(!result.has_major_coercions());

        let result = CoercionResult::new(42).with_flag(CoercionFlag::TruncatedJson);
        assert!(result.has_major_coercions());
    }

    #[test]
    fn test_coercion_result_map() {
        let result = CoercionResult::new(42)
            .with_flag(CoercionFlag::StrippedMarkdown)
            .set_confidence(0.8);

        let mapped = result.map(|x: i32| x.to_string());
        assert_eq!(mapped.value, "42");
        assert_eq!(mapped.flags.len(), 1);
        assert_eq!(mapped.confidence, 0.8);
    }

    #[test]
    fn test_coercion_flag_serialization() {
        let flag = CoercionFlag::StrippedMarkdown;
        let json = serde_json::to_string(&flag).unwrap();
        let parsed: CoercionFlag = serde_json::from_str(&json).unwrap();
        assert_eq!(flag, parsed);

        let flag = CoercionFlag::TypeCoercion {
            from: "string".to_string(),
            to: "number".to_string(),
        };
        let json = serde_json::to_string(&flag).unwrap();
        let parsed: CoercionFlag = serde_json::from_str(&json).unwrap();
        assert_eq!(flag, parsed);
    }

    #[test]
    fn test_coercion_result_serialization() {
        let result = CoercionResult::new(42)
            .with_flag(CoercionFlag::StrippedMarkdown)
            .set_confidence(0.8);

        let json = serde_json::to_string(&result).unwrap();
        let parsed: CoercionResult<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(result.value, parsed.value);
        assert_eq!(result.flags, parsed.flags);
        assert_eq!(result.confidence, parsed.confidence);
    }
}
