//! In-memory cache implementation with LRU eviction.

use async_trait::async_trait;
use simple_agents_types::cache::Cache;
use simple_agents_types::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Entry in the cache with expiration and access tracking.
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Cached data
    data: Vec<u8>,
    /// When this entry expires
    expires_at: Instant,
    /// Last time this entry was accessed (for LRU)
    last_accessed: Instant,
}

impl CacheEntry {
    /// Check if this entry has expired.
    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    /// Update the last accessed time.
    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

/// In-memory cache with TTL and LRU eviction.
///
/// This cache stores entries in memory and automatically evicts:
/// - Expired entries (based on TTL)
/// - Least recently used entries (when max size or max entries exceeded)
///
/// # Example
/// ```no_run
/// use simple_agents_cache::InMemoryCache;
/// use simple_agents_types::cache::Cache;
/// use std::time::Duration;
///
/// # async fn example() {
/// let cache = InMemoryCache::new(1024 * 1024, 100); // 1MB, 100 entries
///
/// cache.set("key1", b"value1".to_vec(), Duration::from_secs(60)).await.unwrap();
/// let value = cache.get("key1").await.unwrap();
/// assert_eq!(value, Some(b"value1".to_vec()));
/// # }
/// ```
pub struct InMemoryCache {
    /// The cache store
    store: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Maximum total size in bytes
    max_size: usize,
    /// Maximum number of entries
    max_entries: usize,
}

impl InMemoryCache {
    /// Create a new in-memory cache.
    ///
    /// # Arguments
    /// - `max_size`: Maximum total size in bytes (0 = unlimited)
    /// - `max_entries`: Maximum number of entries (0 = unlimited)
    pub fn new(max_size: usize, max_entries: usize) -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_entries,
        }
    }

    /// Evict expired entries from the cache.
    async fn evict_expired(&self) {
        let mut store = self.store.write().await;
        store.retain(|_, entry| !entry.is_expired());
    }

    /// Evict least recently used entries to enforce size/count limits.
    async fn evict_lru(&self) {
        let mut store = self.store.write().await;

        // Calculate current size
        let current_size: usize = store.values().map(|e| e.data.len()).sum();

        // Check if we need to evict based on size or entry count
        let needs_eviction = (self.max_size > 0 && current_size > self.max_size)
            || (self.max_entries > 0 && store.len() > self.max_entries);

        if !needs_eviction {
            return;
        }

        // Sort entries by last accessed time (oldest first)
        let mut entries: Vec<_> = store.iter().map(|(k, v)| (k.clone(), v.last_accessed, v.data.len())).collect();
        entries.sort_by_key(|(_, accessed, _)| *accessed);

        // Remove oldest entries until we're under the limit
        let mut remaining_size = current_size;
        let mut remaining_count = store.len();
        let mut entries_to_remove = Vec::new();

        for (key, _, size) in entries {
            // Check if we're now under both limits
            let under_size_limit = self.max_size == 0 || remaining_size <= self.max_size;
            let under_count_limit = self.max_entries == 0 || remaining_count <= self.max_entries;

            if under_size_limit && under_count_limit {
                break;
            }

            // Mark this entry for removal
            remaining_size = remaining_size.saturating_sub(size);
            remaining_count = remaining_count.saturating_sub(1);
            entries_to_remove.push(key);
        }

        // Remove the marked entries
        for key in entries_to_remove {
            store.remove(&key);
        }
    }
}

#[async_trait]
impl Cache for InMemoryCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // First evict expired entries
        self.evict_expired().await;

        let mut store = self.store.write().await;

        if let Some(entry) = store.get_mut(key) {
            if entry.is_expired() {
                store.remove(key);
                return Ok(None);
            }

            entry.touch();
            Ok(Some(entry.data.clone()))
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()> {
        let entry = CacheEntry {
            data: value,
            expires_at: Instant::now() + ttl,
            last_accessed: Instant::now(),
        };

        {
            let mut store = self.store.write().await;
            store.insert(key.to_string(), entry);
        }

        // Evict if needed
        self.evict_lru().await;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut store = self.store.write().await;
        store.remove(key);
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut store = self.store.write().await;
        store.clear();
        Ok(())
    }

    fn name(&self) -> &str {
        "in-memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_basic_set_get() {
        let cache = InMemoryCache::new(1024, 10);

        cache.set("key1", b"value1".to_vec(), Duration::from_secs(60)).await.unwrap();
        let value = cache.get("key1").await.unwrap();

        assert_eq!(value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let cache = InMemoryCache::new(1024, 10);
        let value = cache.get("nonexistent").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let cache = InMemoryCache::new(1024, 10);

        // Set with very short TTL
        cache.set("key1", b"value1".to_vec(), Duration::from_millis(100)).await.unwrap();

        // Should exist immediately
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should be expired
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let cache = InMemoryCache::new(1024, 10);

        cache.set("key1", b"value1".to_vec(), Duration::from_secs(60)).await.unwrap();
        assert!(cache.get("key1").await.unwrap().is_some());

        cache.delete("key1").await.unwrap();
        assert!(cache.get("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = InMemoryCache::new(1024, 10);

        cache.set("key1", b"value1".to_vec(), Duration::from_secs(60)).await.unwrap();
        cache.set("key2", b"value2".to_vec(), Duration::from_secs(60)).await.unwrap();

        cache.clear().await.unwrap();

        assert!(cache.get("key1").await.unwrap().is_none());
        assert!(cache.get("key2").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_lru_eviction_by_count() {
        let cache = InMemoryCache::new(0, 2); // Max 2 entries

        cache.set("key1", b"value1".to_vec(), Duration::from_secs(60)).await.unwrap();
        cache.set("key2", b"value2".to_vec(), Duration::from_secs(60)).await.unwrap();

        // At this point we have 2 entries (at limit)

        // Add a third entry, should trigger eviction
        cache.set("key3", b"value3".to_vec(), Duration::from_secs(60)).await.unwrap();

        // After eviction, we should have at most 2 entries
        let store = cache.store.read().await;
        assert!(store.len() <= 2, "Cache should not exceed max_entries");
        // key3 (most recent) should definitely exist
        assert!(store.contains_key("key3"), "Most recently added key should exist");
    }

    #[tokio::test]
    async fn test_lru_eviction_by_size() {
        let cache = InMemoryCache::new(10, 0); // Max 10 bytes

        cache.set("key1", vec![1, 2, 3, 4, 5], Duration::from_secs(60)).await.unwrap();
        cache.set("key2", vec![6, 7, 8, 9, 10], Duration::from_secs(60)).await.unwrap();

        // Access key1 to make it more recently used
        cache.get("key1").await.unwrap();

        // Add a new entry that would exceed size limit
        cache.set("key3", vec![11, 12], Duration::from_secs(60)).await.unwrap();

        // key1 should still exist, key2 should be evicted
        assert!(cache.get("key1").await.unwrap().is_some());
        // key3 should exist
        assert!(cache.get("key3").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cache_name() {
        let cache = InMemoryCache::new(1024, 10);
        assert_eq!(cache.name(), "in-memory");
    }
}
