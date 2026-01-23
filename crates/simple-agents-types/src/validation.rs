//! Validation types for sensitive data.
//!
//! Provides secure handling of API keys and other credentials.

use crate::error::{Result, ValidationError};
use serde::{Deserialize, Serialize};
use std::fmt;
use subtle::ConstantTimeEq;

/// API key (validated, never logged or displayed).
///
/// This type ensures API keys are:
/// - Validated on construction
/// - Never logged in Debug output
/// - Never serialized in plain text
/// - Only exposed through explicit `expose()` method
///
/// # Example
/// ```
/// use simple_agents_types::validation::ApiKey;
///
/// let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
/// let debug_str = format!("{:?}", key);
/// assert!(debug_str.contains("REDACTED"));
/// assert!(!debug_str.contains("sk-"));
/// ```
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    /// Create a new API key with validation.
    ///
    /// # Validation Rules
    /// - Must not be empty
    /// - Must be at least 20 characters
    /// - Must not contain null bytes (security)
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::validation::ApiKey;
    ///
    /// let key = ApiKey::new("sk-1234567890abcdef1234567890");
    /// assert!(key.is_ok());
    ///
    /// let invalid = ApiKey::new("short");
    /// assert!(invalid.is_err());
    /// ```
    pub fn new(key: impl Into<String>) -> Result<Self> {
        let key = key.into();

        if key.is_empty() {
            return Err(ValidationError::Empty {
                field: "api_key".to_string(),
            }
            .into());
        }

        if key.len() < 20 {
            return Err(ValidationError::TooShort {
                field: "api_key".to_string(),
                min: 20,
            }
            .into());
        }

        // Security: prevent null byte injection
        if key.contains('\0') {
            return Err(ValidationError::InvalidFormat {
                field: "api_key".to_string(),
                reason: "contains null bytes".to_string(),
            }
            .into());
        }

        Ok(Self(key))
    }

    /// Expose the raw API key.
    ///
    /// # Security Warning
    /// Only use this when actually making API requests. Never log or display the result.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::validation::ApiKey;
    ///
    /// let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
    /// let raw = key.expose();
    /// assert_eq!(raw, "sk-1234567890abcdef1234567890");
    /// ```
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Get a redacted preview of the key (for debugging).
    ///
    /// Shows only the prefix and length.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::validation::ApiKey;
    ///
    /// let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
    /// let preview = key.preview();
    /// assert!(preview.contains("sk-"));
    /// assert!(preview.contains("29 chars"));
    /// ```
    pub fn preview(&self) -> String {
        let prefix = if self.0.len() >= 7 {
            &self.0[..7]
        } else {
            &self.0
        };
        format!("{}*** ({} chars)", prefix, self.0.len())
    }
}

// CRITICAL: Never log API keys in Debug output
impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey([REDACTED])")
    }
}

// CRITICAL: Never serialize API keys in plain text
impl Serialize for ApiKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

// Allow deserialization for config loading
impl<'de> Deserialize<'de> for ApiKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ApiKey::new(s).map_err(serde::de::Error::custom)
    }
}

// Implement PartialEq with constant-time comparison for security
impl PartialEq for ApiKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

impl Eq for ApiKey {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_valid() {
        let key = ApiKey::new("sk-1234567890abcdef1234567890");
        assert!(key.is_ok());
    }

    #[test]
    fn test_api_key_empty() {
        let key = ApiKey::new("");
        assert!(key.is_err());
        assert!(matches!(
            key.unwrap_err(),
            crate::error::SimpleAgentsError::Validation(ValidationError::Empty { .. })
        ));
    }

    #[test]
    fn test_api_key_too_short() {
        let key = ApiKey::new("short");
        assert!(key.is_err());
        assert!(matches!(
            key.unwrap_err(),
            crate::error::SimpleAgentsError::Validation(ValidationError::TooShort { .. })
        ));
    }

    #[test]
    fn test_api_key_null_byte() {
        let key = ApiKey::new("sk-12345678901234567890\0extra");
        assert!(key.is_err());
        assert!(matches!(
            key.unwrap_err(),
            crate::error::SimpleAgentsError::Validation(ValidationError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn test_api_key_expose() {
        let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        assert_eq!(key.expose(), "sk-1234567890abcdef1234567890");
    }

    #[test]
    fn test_api_key_debug_redacted() {
        let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let debug = format!("{:?}", key);
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("sk-"));
        assert!(!debug.contains("1234"));
    }

    #[test]
    fn test_api_key_serialize_redacted() {
        let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, "\"[REDACTED]\"");
        assert!(!json.contains("sk-"));
    }

    #[test]
    fn test_api_key_deserialize() {
        let json = "\"sk-1234567890abcdef1234567890\"";
        let key: ApiKey = serde_json::from_str(json).unwrap();
        assert_eq!(key.expose(), "sk-1234567890abcdef1234567890");
    }

    #[test]
    fn test_api_key_preview() {
        let key = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let preview = key.preview();
        assert!(preview.contains("sk-"));
        assert!(preview.contains("29 chars"));
        assert!(!preview.contains("abcdef"));
    }

    #[test]
    fn test_api_key_equality() {
        let key1 = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let key2 = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let key3 = ApiKey::new("sk-differentkey1234567890").unwrap();

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_api_key_constant_time_comparison() {
        // Test that constant-time comparison works correctly
        let key1 = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let key2 = ApiKey::new("sk-1234567890abcdef1234567890").unwrap();
        let key3 = ApiKey::new("sk-9999999999999999999999").unwrap();

        // Same keys should be equal
        assert_eq!(key1, key2);

        // Different keys should not be equal
        assert_ne!(key1, key3);

        // Keys differing only in last character should still be detected as different
        let key4 = ApiKey::new("sk-1234567890abcdef12345678901").unwrap();
        assert_ne!(key1, key4);
    }
}
