//! No-op cache implementation for testing and disabling caching.

use async_trait::async_trait;
use simple_agents_types::cache::Cache;
use simple_agents_types::error::Result;
use std::time::Duration;

/// A cache that doesn't actually cache anything.
///
/// Useful for:
/// - Testing without caching
/// - Disabling caching in production
/// - Debugging cache-related issues
///
/// # Example
/// ```no_run
/// use simple_agents_cache::NoOpCache;
/// use simple_agents_types::cache::Cache;
/// use std::time::Duration;
///
/// # async fn example() {
/// let cache = NoOpCache;
///
/// cache.set("key1", b"value1".to_vec(), Duration::from_secs(60)).await.unwrap();
/// let value = cache.get("key1").await.unwrap();
/// assert_eq!(value, None); // NoOpCache always returns None
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpCache;

#[async_trait]
impl Cache for NoOpCache {
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn set(&self, _key: &str, _value: Vec<u8>, _ttl: Duration) -> Result<()> {
        Ok(())
    }

    async fn delete(&self, _key: &str) -> Result<()> {
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "noop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_always_returns_none() {
        let cache = NoOpCache;

        cache.set("key1", b"value1".to_vec(), Duration::from_secs(60)).await.unwrap();
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_noop_is_disabled() {
        let cache = NoOpCache;
        assert!(!cache.is_enabled());
    }

    #[tokio::test]
    async fn test_noop_name() {
        let cache = NoOpCache;
        assert_eq!(cache.name(), "noop");
    }

    #[tokio::test]
    async fn test_noop_delete_does_nothing() {
        let cache = NoOpCache;
        cache.delete("key1").await.unwrap();
        // Should not panic or error
    }

    #[tokio::test]
    async fn test_noop_clear_does_nothing() {
        let cache = NoOpCache;
        cache.clear().await.unwrap();
        // Should not panic or error
    }
}
