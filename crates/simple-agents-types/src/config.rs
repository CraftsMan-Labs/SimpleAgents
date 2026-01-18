//! Configuration types for SimpleAgents.
//!
//! Provides configuration for retry, healing, and provider capabilities.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Retry configuration for failed requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial backoff duration
    #[serde(with = "duration_millis")]
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    #[serde(with = "duration_millis")]
    pub max_backoff: Duration,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f32,
    /// Add random jitter to backoff
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Calculate backoff duration for a given attempt.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::config::RetryConfig;
    /// use std::time::Duration;
    ///
    /// let config = RetryConfig::default();
    /// let backoff = config.calculate_backoff(1);
    /// assert!(backoff >= Duration::from_millis(100));
    /// assert!(backoff <= Duration::from_millis(200)); // with jitter
    /// ```
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base =
            self.initial_backoff.as_millis() as f32 * self.backoff_multiplier.powi(attempt as i32);
        let capped = base.min(self.max_backoff.as_millis() as f32);

        let duration = if self.jitter {
            // Add up to 50% jitter
            let jitter_factor = 0.5 + (rand() * 0.5);
            Duration::from_millis((capped * jitter_factor) as u64)
        } else {
            Duration::from_millis(capped as u64)
        };

        duration.min(self.max_backoff)
    }
}

// Cryptographically secure random number generator for jitter (0.0-1.0)
fn rand() -> f32 {
    use rand::Rng;
    rand::thread_rng().gen()
}

/// Healing configuration for response coercion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealingConfig {
    /// Enable healing/coercion
    pub enabled: bool,
    /// Strict mode (fail on coercion)
    pub strict_mode: bool,
    /// Allow type coercion
    pub allow_type_coercion: bool,
    /// Minimum confidence threshold (0.0-1.0)
    pub min_confidence: f32,
    /// Allow fuzzy field name matching
    pub allow_fuzzy_matching: bool,
    /// Maximum healing attempts
    pub max_attempts: u32,
}

impl Default for HealingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strict_mode: false,
            allow_type_coercion: true,
            min_confidence: 0.7,
            allow_fuzzy_matching: true,
            max_attempts: 3,
        }
    }
}

impl HealingConfig {
    /// Create a strict healing configuration.
    pub fn strict() -> Self {
        Self {
            enabled: true,
            strict_mode: true,
            allow_type_coercion: false,
            min_confidence: 0.95,
            allow_fuzzy_matching: false,
            max_attempts: 1,
        }
    }

    /// Create a lenient healing configuration.
    pub fn lenient() -> Self {
        Self {
            enabled: true,
            strict_mode: false,
            allow_type_coercion: true,
            min_confidence: 0.5,
            allow_fuzzy_matching: true,
            max_attempts: 5,
        }
    }
}

/// Provider capabilities.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Supports streaming responses
    pub streaming: bool,
    /// Supports function/tool calling
    pub function_calling: bool,
    /// Supports vision/image inputs
    pub vision: bool,
    /// Maximum output tokens
    pub max_tokens: u32,
}

/// Provider configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider name
    pub name: String,
    /// Base URL for API
    pub base_url: String,
    /// API key (optional for some providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Default model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    /// Retry configuration
    #[serde(default)]
    pub retry_config: RetryConfig,
    /// Request timeout
    #[serde(with = "duration_millis")]
    pub timeout: Duration,
    /// Provider capabilities
    #[serde(default)]
    pub capabilities: Capabilities,
}

impl ProviderConfig {
    /// Create a new provider configuration.
    pub fn new(name: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            api_key: None,
            default_model: None,
            retry_config: RetryConfig::default(),
            timeout: Duration::from_secs(30),
            capabilities: Capabilities::default(),
        }
    }

    /// Set the API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

// Serde helper for Duration serialization/deserialization as milliseconds
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_backoff, Duration::from_millis(100));
        assert_eq!(config.max_backoff, Duration::from_secs(10));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.jitter);
    }

    #[test]
    fn test_retry_config_backoff() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let backoff1 = config.calculate_backoff(0);
        let backoff2 = config.calculate_backoff(1);
        let backoff3 = config.calculate_backoff(2);

        assert_eq!(backoff1, Duration::from_millis(100));
        assert_eq!(backoff2, Duration::from_millis(200));
        assert_eq!(backoff3, Duration::from_millis(400));
    }

    #[test]
    fn test_retry_config_max_backoff() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let backoff = config.calculate_backoff(10);
        assert!(backoff <= Duration::from_secs(1));
    }

    #[test]
    fn test_healing_config_default() {
        let config = HealingConfig::default();
        assert!(config.enabled);
        assert!(!config.strict_mode);
        assert!(config.allow_type_coercion);
        assert_eq!(config.min_confidence, 0.7);
        assert!(config.allow_fuzzy_matching);
    }

    #[test]
    fn test_healing_config_strict() {
        let config = HealingConfig::strict();
        assert!(config.enabled);
        assert!(config.strict_mode);
        assert!(!config.allow_type_coercion);
        assert_eq!(config.min_confidence, 0.95);
        assert!(!config.allow_fuzzy_matching);
    }

    #[test]
    fn test_healing_config_lenient() {
        let config = HealingConfig::lenient();
        assert!(config.enabled);
        assert!(!config.strict_mode);
        assert!(config.allow_type_coercion);
        assert_eq!(config.min_confidence, 0.5);
        assert!(config.allow_fuzzy_matching);
    }

    #[test]
    fn test_capabilities_default() {
        let caps = Capabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.function_calling);
        assert!(!caps.vision);
        assert_eq!(caps.max_tokens, 0);
    }

    #[test]
    fn test_provider_config_builder() {
        let config = ProviderConfig::new("openai", "https://api.openai.com/v1")
            .with_api_key("sk-test")
            .with_default_model("gpt-4")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(config.name, "openai");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.api_key, Some("sk-test".to_string()));
        assert_eq!(config.default_model, Some("gpt-4".to_string()));
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_serialization() {
        let config = RetryConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: RetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn test_provider_config_serialization() {
        let config = ProviderConfig::new("test", "https://example.com");
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.name, parsed.name);
        assert_eq!(config.base_url, parsed.base_url);
    }

    #[test]
    fn test_jitter_randomness() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
        };

        // Generate multiple backoffs and verify they're different (with high probability)
        let backoffs: Vec<Duration> = (0..10)
            .map(|_| config.calculate_backoff(1))
            .collect();

        // All values should be within expected range (50-150ms for attempt 1 with jitter)
        for backoff in &backoffs {
            let ms = backoff.as_millis();
            assert!(ms >= 100, "Backoff too small: {}ms", ms); // 50% of 200ms = 100ms
            assert!(ms <= 300, "Backoff too large: {}ms", ms); // 150% of 200ms = 300ms
        }

        // At least some values should be different (very high probability with true randomness)
        let unique_count = backoffs.iter().collect::<std::collections::HashSet<_>>().len();
        assert!(unique_count > 1, "All jitter values are the same - RNG may not be working");
    }
}
