//! In-memory cache implementation with LRU eviction.

use async_trait::async_trait;
use simple_agent_type::cache::Cache;
use simple_agent_type::error::Result;
use std::collections::{HashMap, VecDeque};
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
    /// Logical access tick used for LRU ordering.
    access_tick: u64,
}

#[derive(Debug, Default)]
struct CacheState {
    store: HashMap<String, CacheEntry>,
    access_order: VecDeque<(String, u64)>,
    total_size: usize,
    next_tick: u64,
}

impl CacheState {
    fn next_access_tick(&mut self) -> u64 {
        self.next_tick = self.next_tick.saturating_add(1);
        self.next_tick
    }

    fn is_expired(entry: &CacheEntry) -> bool {
        Instant::now() >= entry.expires_at
    }

    fn touch_key(&mut self, key: &str) {
        let tick = self.next_access_tick();
        if let Some(entry) = self.store.get_mut(key) {
            entry.access_tick = tick;
            self.access_order.push_back((key.to_string(), tick));
        }
    }

    fn upsert(&mut self, key: &str, value: Vec<u8>, ttl: Duration) {
        let tick = self.next_access_tick();
        if let Some(previous) = self.store.get(key) {
            self.total_size = self.total_size.saturating_sub(previous.data.len());
        }

        self.total_size = self.total_size.saturating_add(value.len());
        self.store.insert(
            key.to_string(),
            CacheEntry {
                data: value,
                expires_at: Instant::now() + ttl,
                access_tick: tick,
            },
        );
        self.access_order.push_back((key.to_string(), tick));
    }

    fn remove_key(&mut self, key: &str) {
        if let Some(entry) = self.store.remove(key) {
            self.total_size = self.total_size.saturating_sub(entry.data.len());
        }
    }

    fn evict_expired(&mut self) {
        let now = Instant::now();
        self.store.retain(|_, entry| {
            if now >= entry.expires_at {
                self.total_size = self.total_size.saturating_sub(entry.data.len());
                false
            } else {
                true
            }
        });
    }

    fn evict_lru_until_within_limits(&mut self, max_size: usize, max_entries: usize) {
        let over_size = |total_size: usize| max_size > 0 && total_size > max_size;
        let over_count = |entry_count: usize| max_entries > 0 && entry_count > max_entries;

        while over_size(self.total_size) || over_count(self.store.len()) {
            let Some((key, tick)) = self.access_order.pop_front() else {
                break;
            };

            let should_remove = self
                .store
                .get(key.as_str())
                .is_some_and(|entry| entry.access_tick == tick);

            if should_remove {
                self.remove_key(key.as_str());
            }
        }
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
/// use simple_agent_type::cache::Cache;
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
    /// Shared cache state.
    state: Arc<RwLock<CacheState>>,
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
            state: Arc::new(RwLock::new(CacheState::default())),
            max_size,
            max_entries,
        }
    }
}

#[async_trait]
impl Cache for InMemoryCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut state = self.state.write().await;
        match state.store.get(key) {
            Some(entry) if CacheState::is_expired(entry) => {
                state.remove_key(key);
                Ok(None)
            }
            Some(_) => {
                let value = state.store.get(key).map(|entry| entry.data.clone());
                state.touch_key(key);
                Ok(value)
            }
            None => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()> {
        let mut state = self.state.write().await;
        state.upsert(key, value, ttl);
        state.evict_expired();
        state.evict_lru_until_within_limits(self.max_size, self.max_entries);

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut state = self.state.write().await;
        state.remove_key(key);
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.store.clear();
        state.access_order.clear();
        state.total_size = 0;
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

        cache
            .set("key1", b"value1".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();
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
        cache
            .set("key1", b"value1".to_vec(), Duration::from_millis(100))
            .await
            .unwrap();

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

        cache
            .set("key1", b"value1".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(cache.get("key1").await.unwrap().is_some());

        cache.delete("key1").await.unwrap();
        assert!(cache.get("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = InMemoryCache::new(1024, 10);

        cache
            .set("key1", b"value1".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .set("key2", b"value2".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();

        cache.clear().await.unwrap();

        assert!(cache.get("key1").await.unwrap().is_none());
        assert!(cache.get("key2").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_lru_eviction_by_count() {
        let cache = InMemoryCache::new(0, 2); // Max 2 entries

        cache
            .set("key1", b"value1".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .set("key2", b"value2".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();

        // At this point we have 2 entries (at limit)

        // Add a third entry, should trigger eviction
        cache
            .set("key3", b"value3".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();

        // After eviction, we should have at most 2 entries
        let store = cache.state.read().await;
        assert!(
            store.store.len() <= 2,
            "Cache should not exceed max_entries"
        );
        // key3 (most recent) should definitely exist
        assert!(
            store.store.contains_key("key3"),
            "Most recently added key should exist"
        );
    }

    #[tokio::test]
    async fn test_lru_eviction_by_size() {
        let cache = InMemoryCache::new(10, 0); // Max 10 bytes

        cache
            .set("key1", vec![1, 2, 3, 4, 5], Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .set("key2", vec![6, 7, 8, 9, 10], Duration::from_secs(60))
            .await
            .unwrap();

        // Access key1 to make it more recently used
        cache.get("key1").await.unwrap();

        // Add a new entry that would exceed size limit
        cache
            .set("key3", vec![11, 12], Duration::from_secs(60))
            .await
            .unwrap();

        // key1 should still exist, key2 should be evicted
        assert!(cache.get("key1").await.unwrap().is_some());
        // key3 should exist
        assert!(cache.get("key3").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_concurrent_gets_do_not_serialize_readers() {
        let cache = Arc::new(InMemoryCache::new(1024, 10));
        cache
            .set("shared", b"value".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();

        let mut handles = Vec::new();
        for _ in 0..25 {
            let cache = cache.clone();
            handles.push(tokio::spawn(
                async move { cache.get("shared").await.unwrap() },
            ));
        }

        for handle in handles {
            assert_eq!(handle.await.unwrap(), Some(b"value".to_vec()));
        }
    }

    #[tokio::test]
    async fn test_cache_name() {
        let cache = InMemoryCache::new(1024, 10);
        assert_eq!(cache.name(), "in-memory");
    }

    #[tokio::test]
    async fn test_update_existing_key_keeps_latest_value() {
        let cache = InMemoryCache::new(6, 2);

        cache
            .set("key", b"abc".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .set("key", b"abcdef".to_vec(), Duration::from_secs(60))
            .await
            .unwrap();

        let value = cache.get("key").await.unwrap();
        assert_eq!(value, Some(b"abcdef".to_vec()));
    }
}
