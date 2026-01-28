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
/// ```rust
/// use simple_agent_type::cache::Cache;
/// use simple_agent_type::error::Result;
/// use async_trait::async_trait;
/// use std::collections::HashMap;
/// use std::sync::{Arc, Mutex};
/// use std::time::Duration;
///
/// struct InMemoryCache {
///     store: Arc<Mutex<HashMap<String, Vec<u8>>>>,
/// }
///
/// #[async_trait]
/// impl Cache for InMemoryCache {
///     async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
///         let store = self.store.lock().unwrap();
///         Ok(store.get(key).cloned())
///     }
///
///     async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> Result<()> {
///         let mut store = self.store.lock().unwrap();
///         store.insert(key.to_string(), value);
///         Ok(())
///     }
///
///     async fn delete(&self, key: &str) -> Result<()> {
///         let mut store = self.store.lock().unwrap();
///         store.remove(key);
///         Ok(())
///     }
///
///     async fn clear(&self) -> Result<()> {
///         let mut store = self.store.lock().unwrap();
///         store.clear();
///         Ok(())
///     }
/// }
///
/// let cache = InMemoryCache {
///     store: Arc::new(Mutex::new(HashMap::new())),
/// };
///
/// let rt = tokio::runtime::Runtime::new().unwrap();
/// rt.block_on(async {
///     cache
///         .set("request:abc123", b"ok".to_vec(), Duration::from_secs(60))
///         .await
///         .unwrap();
///     let value = cache.get("request:abc123").await.unwrap();
///     assert_eq!(value, Some(b"ok".to_vec()));
/// });
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
    /// ```rust
    /// use simple_agent_type::cache::Cache;
    /// use async_trait::async_trait;
    /// use std::collections::HashMap;
    /// use std::sync::{Arc, Mutex};
    /// use std::time::Duration;
    ///
    /// # struct InMemoryCache {
    /// #     store: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// # }
    /// # #[async_trait]
    /// # impl Cache for InMemoryCache {
    /// #     async fn get(&self, key: &str) -> simple_agent_type::error::Result<Option<Vec<u8>>> {
    /// #         let store = self.store.lock().unwrap();
    /// #         Ok(store.get(key).cloned())
    /// #     }
    /// #     async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.insert(key.to_string(), value);
    /// #         Ok(())
    /// #     }
    /// #     async fn delete(&self, key: &str) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.remove(key);
    /// #         Ok(())
    /// #     }
    /// #     async fn clear(&self) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.clear();
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let cache = InMemoryCache {
    /// #     store: Arc::new(Mutex::new(HashMap::new())),
    /// # };
    /// # let rt = tokio::runtime::Runtime::new().unwrap();
    /// # rt.block_on(async {
    /// cache
    ///     .set("request:abc123", b"ok".to_vec(), Duration::from_secs(60))
    ///     .await
    ///     .unwrap();
    /// let value = cache.get("request:abc123").await.unwrap();
    /// assert_eq!(value, Some(b"ok".to_vec()));
    /// # });
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
    /// ```rust
    /// use simple_agent_type::cache::Cache;
    /// use async_trait::async_trait;
    /// use std::collections::HashMap;
    /// use std::sync::{Arc, Mutex};
    /// use std::time::Duration;
    ///
    /// # struct InMemoryCache {
    /// #     store: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// # }
    /// # #[async_trait]
    /// # impl Cache for InMemoryCache {
    /// #     async fn get(&self, key: &str) -> simple_agent_type::error::Result<Option<Vec<u8>>> {
    /// #         let store = self.store.lock().unwrap();
    /// #         Ok(store.get(key).cloned())
    /// #     }
    /// #     async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.insert(key.to_string(), value);
    /// #         Ok(())
    /// #     }
    /// #     async fn delete(&self, key: &str) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.remove(key);
    /// #         Ok(())
    /// #     }
    /// #     async fn clear(&self) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.clear();
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let cache = InMemoryCache {
    /// #     store: Arc::new(Mutex::new(HashMap::new())),
    /// # };
    /// # let rt = tokio::runtime::Runtime::new().unwrap();
    /// # rt.block_on(async {
    /// cache
    ///     .set("request:abc123", b"payload".to_vec(), Duration::from_secs(3600))
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;

    /// Delete a value from the cache.
    ///
    /// # Arguments
    /// - `key`: Cache key
    ///
    /// # Example
    /// ```rust
    /// use simple_agent_type::cache::Cache;
    /// use async_trait::async_trait;
    /// use std::collections::HashMap;
    /// use std::sync::{Arc, Mutex};
    /// use std::time::Duration;
    ///
    /// # struct InMemoryCache {
    /// #     store: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// # }
    /// # #[async_trait]
    /// # impl Cache for InMemoryCache {
    /// #     async fn get(&self, key: &str) -> simple_agent_type::error::Result<Option<Vec<u8>>> {
    /// #         let store = self.store.lock().unwrap();
    /// #         Ok(store.get(key).cloned())
    /// #     }
    /// #     async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.insert(key.to_string(), value);
    /// #         Ok(())
    /// #     }
    /// #     async fn delete(&self, key: &str) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.remove(key);
    /// #         Ok(())
    /// #     }
    /// #     async fn clear(&self) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.clear();
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let cache = InMemoryCache {
    /// #     store: Arc::new(Mutex::new(HashMap::new())),
    /// # };
    /// # let rt = tokio::runtime::Runtime::new().unwrap();
    /// # rt.block_on(async {
    /// cache
    ///     .set("request:abc123", b"payload".to_vec(), Duration::from_secs(60))
    ///     .await
    ///     .unwrap();
    /// cache.delete("request:abc123").await.unwrap();
    /// # });
    /// ```
    async fn delete(&self, key: &str) -> Result<()>;

    /// Clear all values from the cache.
    ///
    /// # Warning
    /// This is a destructive operation. Use with caution.
    ///
    /// # Example
    /// ```rust
    /// use simple_agent_type::cache::Cache;
    /// use async_trait::async_trait;
    /// use std::collections::HashMap;
    /// use std::sync::{Arc, Mutex};
    /// use std::time::Duration;
    ///
    /// # struct InMemoryCache {
    /// #     store: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// # }
    /// # #[async_trait]
    /// # impl Cache for InMemoryCache {
    /// #     async fn get(&self, key: &str) -> simple_agent_type::error::Result<Option<Vec<u8>>> {
    /// #         let store = self.store.lock().unwrap();
    /// #         Ok(store.get(key).cloned())
    /// #     }
    /// #     async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.insert(key.to_string(), value);
    /// #         Ok(())
    /// #     }
    /// #     async fn delete(&self, key: &str) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.remove(key);
    /// #         Ok(())
    /// #     }
    /// #     async fn clear(&self) -> simple_agent_type::error::Result<()> {
    /// #         let mut store = self.store.lock().unwrap();
    /// #         store.clear();
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let cache = InMemoryCache {
    /// #     store: Arc::new(Mutex::new(HashMap::new())),
    /// # };
    /// # let rt = tokio::runtime::Runtime::new().unwrap();
    /// # rt.block_on(async {
    /// cache
    ///     .set("request:abc123", b"payload".to_vec(), Duration::from_secs(60))
    ///     .await
    ///     .unwrap();
    /// cache.clear().await.unwrap();
    /// # });
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
    /// Uses blake3 for cryptographically secure and deterministic hashing.
    ///
    /// # Example
    /// ```
    /// use simple_agent_type::cache::CacheKey;
    ///
    /// let key = CacheKey::from_parts("openai", "gpt-4", "user:Hello");
    /// assert!(key.starts_with("openai:"));
    /// ```
    pub fn from_parts(provider: &str, model: &str, content: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(provider.as_bytes());
        hasher.update(model.as_bytes());
        hasher.update(content.as_bytes());
        let hash = hasher.finalize();
        format!("{}:{}:{}", provider, model, hash.to_hex())
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

    #[test]
    fn test_cache_key_blake3_deterministic() {
        // Verify blake3 produces deterministic hashes
        let key1 = CacheKey::from_parts("openai", "gpt-4", "Hello, world!");
        let key2 = CacheKey::from_parts("openai", "gpt-4", "Hello, world!");
        assert_eq!(key1, key2, "Blake3 hashing should be deterministic");
    }

    #[test]
    fn test_cache_key_blake3_collision_resistance() {
        // Verify different inputs produce different hashes
        let key1 = CacheKey::from_parts("openai", "gpt-4", "Hello");
        let key2 = CacheKey::from_parts("openai", "gpt-4", "Hello!");
        let key3 = CacheKey::from_parts("openai", "gpt-3.5", "Hello");
        let key4 = CacheKey::from_parts("anthropic", "gpt-4", "Hello");

        assert_ne!(
            key1, key2,
            "Different content should produce different hashes"
        );
        assert_ne!(
            key1, key3,
            "Different models should produce different hashes"
        );
        assert_ne!(
            key1, key4,
            "Different providers should produce different hashes"
        );
    }

    #[test]
    fn test_cache_key_blake3_format() {
        // Verify the hash format is correct (provider:model:hex_hash)
        let key = CacheKey::from_parts("openai", "gpt-4", "test");
        let parts: Vec<&str> = key.split(':').collect();

        assert_eq!(parts.len(), 3, "Key should have 3 parts");
        assert_eq!(parts[0], "openai", "First part should be provider");
        assert_eq!(parts[1], "gpt-4", "Second part should be model");
        assert_eq!(
            parts[2].len(),
            64,
            "Blake3 hash should be 64 hex characters"
        );
        assert!(
            parts[2].chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should be valid hex"
        );
    }
}
