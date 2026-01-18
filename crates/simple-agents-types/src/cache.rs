//! Cache trait for response caching.
//!
//! Provides an abstract interface for caching LLM responses.

use crate::error::Result;
use async_trait::async_trait;
use std::time::Duration;

/// Trait for caching LLM responses.
///
/// Implementations can use various backends:
/// - In-memory (HashMap, LRU)
/// - Redis
/// - Disk-based
/// - Distributed caches
///
/// # Example Implementation
///
/// ```ignore
/// use simple_agents_types::cache::Cache;
/// use simple_agents_types::error::Result;
/// use async_trait::async_trait;
/// use std::time::Duration;
/// use std::collections::HashMap;
///
/// struct InMemoryCache {
///     store: HashMap<String, Vec<u8>>,
/// }
///
/// #[async_trait]
/// impl Cache for InMemoryCache {
///     async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
///         Ok(self.store.get(key).cloned())
///     }
///
///     async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> Result<()> {
///         // Note: real implementation would need Arc<Mutex<HashMap>>
///         Ok(())
///     }
///
///     async fn delete(&self, key: &str) -> Result<()> {
///         Ok(())
///     }
///
///     async fn clear(&self) -> Result<()> {
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait Cache: Send + Sync {
    /// Get a value from the cache.
    ///
    /// Returns `Ok(None)` if the key doesn't exist or has expired.
    ///
    /// # Arguments
    /// - `key`: Cache key
    ///
    /// # Example
    /// ```ignore
    /// let value = cache.get("request:abc123").await?;
    /// if let Some(bytes) = value {
    ///     let response: CompletionResponse = serde_json::from_slice(&bytes)?;
    /// }
    /// ```
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set a value in the cache with TTL.
    ///
    /// # Arguments
    /// - `key`: Cache key
    /// - `value`: Serialized value (typically JSON bytes)
    /// - `ttl`: Time-to-live (expiration duration)
    ///
    /// # Example
    /// ```ignore
    /// let response_bytes = serde_json::to_vec(&response)?;
    /// cache.set("request:abc123", response_bytes, Duration::from_secs(3600)).await?;
    /// ```
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;

    /// Delete a value from the cache.
    ///
    /// # Arguments
    /// - `key`: Cache key
    ///
    /// # Example
    /// ```ignore
    /// cache.delete("request:abc123").await?;
    /// ```
    async fn delete(&self, key: &str) -> Result<()>;

    /// Clear all values from the cache.
    ///
    /// # Warning
    /// This is a destructive operation. Use with caution.
    ///
    /// # Example
    /// ```ignore
    /// cache.clear().await?;
    /// ```
    async fn clear(&self) -> Result<()>;

    /// Check if caching is enabled.
    ///
    /// This allows for a "no-op" cache implementation that always
    /// returns false, disabling caching without changing call sites.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Get the cache name/type.
    ///
    /// Used for logging and debugging.
    fn name(&self) -> &str {
        "cache"
    }
}

/// Cache key builder for standardized key generation.
///
/// Generates deterministic cache keys from requests.
pub struct CacheKey;

impl CacheKey {
    /// Generate a cache key from a request.
    ///
    /// # Example
    /// ```
    /// use simple_agents_types::cache::CacheKey;
    ///
    /// let key = CacheKey::from_parts("openai", "gpt-4", "user:Hello");
    /// assert!(key.starts_with("openai:"));
    /// ```
    pub fn from_parts(provider: &str, model: &str, content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        provider.hash(&mut hasher);
        model.hash(&mut hasher);
        content.hash(&mut hasher);

        format!("{}:{}:{:x}", provider, model, hasher.finish())
    }

    /// Generate a cache key with custom namespace.
    pub fn with_namespace(namespace: &str, key: &str) -> String {
        format!("{}:{}", namespace, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_from_parts() {
        let key1 = CacheKey::from_parts("openai", "gpt-4", "Hello");
        let key2 = CacheKey::from_parts("openai", "gpt-4", "Hello");
        let key3 = CacheKey::from_parts("openai", "gpt-4", "Goodbye");

        // Same inputs produce same key
        assert_eq!(key1, key2);

        // Different inputs produce different keys
        assert_ne!(key1, key3);

        // Keys contain provider and model
        assert!(key1.starts_with("openai:"));
        assert!(key1.contains("gpt-4"));
    }

    #[test]
    fn test_cache_key_with_namespace() {
        let key = CacheKey::with_namespace("responses", "abc123");
        assert_eq!(key, "responses:abc123");
    }

    #[test]
    fn test_cache_key_deterministic() {
        // Keys should be deterministic across runs
        let key1 = CacheKey::from_parts("test", "model", "content");
        let key2 = CacheKey::from_parts("test", "model", "content");
        assert_eq!(key1, key2);
    }

    // Test that Cache trait is object-safe
    #[test]
    fn test_cache_object_safety() {
        fn _assert_object_safe(_: &dyn Cache) {}
    }
}
