//! # LRU Cache Implementation
//!
//! In-memory LRU cache for agent responses.
//! Respects entry count, byte size, and TTL limits.

use std::collections::HashMap;

use tokio::sync::RwLock;
use tracing::debug;

use crate::domain::cache::*;
use crate::domain::error::Result;

/// A node in the LRU doubly-linked list.
#[derive(Debug, Clone)]
struct LruNode {
    key: CacheKey,
    entry: CacheEntry,
    /// More recent neighbor.
    newer: Option<CacheKey>,
    /// Older neighbor.
    older: Option<CacheKey>,
}

/// In-memory LRU cache.
///
/// Evicts the least-recently-used entry when limits are exceeded.
///
/// # Thread safety
/// All state is behind an `RwLock`.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::lru_cache::LruCacheImpl;
/// use omnishell_orchestrator::domain::cache::CacheConfig;
///
/// let cache = LruCacheImpl::new(CacheConfig::default());
/// ```
pub struct LruCacheImpl {
    config: CacheConfig,
    state: RwLock<LruState>,
}

#[derive(Debug)]
struct LruState {
    map: HashMap<CacheKey, LruNode>,
    newest: Option<CacheKey>,
    oldest: Option<CacheKey>,
    total_bytes: usize,
}

impl LruCacheImpl {
    /// Create a new LRU cache with the given configuration.
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            state: RwLock::new(LruState {
                map: HashMap::new(),
                newest: None,
                oldest: None,
                total_bytes: 0,
            }),
        }
    }

    /// Check if an entry has expired based on TTL.
    fn is_expired(&self, entry: &CacheEntry) -> bool {
        if self.config.ttl_seconds == 0 {
            return false;
        }
        let now = chrono::Utc::now().timestamp();
        (now - entry.created_at) as u64 > self.config.ttl_seconds
    }
}

impl LruState {
    /// Remove a key from the linked list (but not from the map).
    fn unlink(&mut self, key: &CacheKey) {
        let node = match self.map.get(key) {
            Some(n) => n.clone(),
            None => return,
        };

        // Fix neighbors.
        if let Some(ref newer_key) = node.newer {
            if let Some(newer_node) = self.map.get_mut(newer_key) {
                newer_node.older = node.older.clone();
            }
        }
        if let Some(ref older_key) = node.older {
            if let Some(older_node) = self.map.get_mut(older_key) {
                older_node.newer = node.newer.clone();
            }
        }

        // Fix head/tail.
        if self.newest.as_ref() == Some(key) {
            self.newest = node.older.clone();
        }
        if self.oldest.as_ref() == Some(key) {
            self.oldest = node.newer.clone();
        }
    }

    /// Push a key to the front (newest) of the list.
    fn push_front(&mut self, key: &CacheKey) {
        if let Some(node) = self.map.get_mut(key) {
            node.newer = None;
            node.older = self.newest.clone();
        }

        if let Some(ref current_newest) = self.newest {
            if let Some(cn) = self.map.get_mut(current_newest) {
                cn.newer = Some(key.clone());
            }
        }

        self.newest = Some(key.clone());

        if self.oldest.is_none() {
            self.oldest = Some(key.clone());
        }
    }

    /// Evict the oldest entry and return its byte size.
    fn evict_oldest(&mut self) -> Option<usize> {
        let oldest_key = self.oldest.clone()?;
        self.unlink(&oldest_key);
        let removed = self.map.remove(&oldest_key)?;
        Some(removed.entry.byte_size)
    }
}

#[async_trait::async_trait]
impl Cache for LruCacheImpl {
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let mut state = self.state.write().await;

        // Check existence and TTL.
        let expired = state
            .map
            .get(key)
            .map(|n| self.is_expired(&n.entry))
            .unwrap_or(false);

        if expired {
            let byte_size = state.map.get(key).map(|n| n.entry.byte_size).unwrap_or(0);
            state.unlink(key);
            state.map.remove(key);
            state.total_bytes = state.total_bytes.saturating_sub(byte_size);
            return Ok(None);
        }

        if state.map.contains_key(key) {
            // Move to front.
            state.unlink(key);
            state.push_front(key);

            // Increment hit count.
            if let Some(node) = state.map.get_mut(key) {
                node.entry.hit_count += 1;
                return Ok(Some(node.entry.clone()));
            }
        }

        Ok(None)
    }

    async fn put(&self, key: CacheKey, entry: CacheEntry) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut state = self.state.write().await;

        // Remove existing entry if present.
        if state.map.contains_key(&key) {
            let old_bytes = state.map.get(&key).map(|n| n.entry.byte_size).unwrap_or(0);
            state.unlink(&key);
            state.map.remove(&key);
            state.total_bytes = state.total_bytes.saturating_sub(old_bytes);
        }

        // Evict until we have room.
        while state.map.len() >= self.config.max_entries
            || state.total_bytes + entry.byte_size > self.config.max_bytes
        {
            if let Some(evicted_bytes) = state.evict_oldest() {
                state.total_bytes = state.total_bytes.saturating_sub(evicted_bytes);
                debug!("LRU cache evicted entry ({evicted_bytes} bytes)");
            } else {
                break;
            }
        }

        let byte_size = entry.byte_size;
        let node = LruNode {
            key: key.clone(),
            entry,
            newer: None,
            older: None,
        };
        state.map.insert(key.clone(), node);
        state.push_front(&key);
        state.total_bytes += byte_size;

        Ok(())
    }

    async fn remove(&self, key: &CacheKey) -> Result<()> {
        let mut state = self.state.write().await;
        if state.map.contains_key(key) {
            let byte_size = state.map.get(key).map(|n| n.entry.byte_size).unwrap_or(0);
            state.unlink(key);
            state.map.remove(key);
            state.total_bytes = state.total_bytes.saturating_sub(byte_size);
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.map.clear();
        state.newest = None;
        state.oldest = None;
        state.total_bytes = 0;
        Ok(())
    }

    async fn len(&self) -> Result<usize> {
        let state = self.state.read().await;
        Ok(state.map.len())
    }

    async fn byte_size(&self) -> Result<usize> {
        let state = self.state.read().await;
        Ok(state.total_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config() -> CacheConfig {
        CacheConfig {
            enabled: true,
            max_entries: 3,
            max_bytes: 10_000,
            ttl_seconds: 0, // no expiration for tests
        }
    }

    fn make_entry(value: &str) -> CacheEntry {
        CacheEntry {
            value: value.to_string(),
            created_at: chrono::Utc::now().timestamp(),
            hit_count: 0,
            byte_size: value.len(),
        }
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let cache = LruCacheImpl::new(small_config());
        let key = CacheKey::compute("prompt", "ctx", "agent", "v1");
        cache
            .put(key.clone(), make_entry("response"))
            .await
            .expect("put should succeed");

        let result = cache.get(&key).await.expect("get should succeed");
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|e| e.value.as_str()), Some("response"));
        assert_eq!(result.as_ref().map(|e| e.hit_count), Some(1));
    }

    #[tokio::test]
    async fn test_eviction_by_count() {
        let cache = LruCacheImpl::new(small_config()); // max 3 entries

        for i in 0..5 {
            let key = CacheKey::compute(&format!("p{i}"), "", "", "");
            cache
                .put(key, make_entry(&format!("v{i}")))
                .await
                .expect("put should succeed");
        }

        let len = cache.len().await.expect("len should succeed");
        assert_eq!(len, 3);

        // First two should be evicted.
        let k0 = CacheKey::compute("p0", "", "", "");
        assert!(cache.get(&k0).await.expect("get").is_none());

        // Last should still be present.
        let k4 = CacheKey::compute("p4", "", "", "");
        assert!(cache.get(&k4).await.expect("get").is_some());
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = LruCacheImpl::new(small_config());
        let key = CacheKey::compute("p", "", "", "");
        cache
            .put(key.clone(), make_entry("v"))
            .await
            .expect("put should succeed");
        cache.remove(&key).await.expect("remove should succeed");
        assert!(cache.get(&key).await.expect("get").is_none());
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = LruCacheImpl::new(small_config());
        let key = CacheKey::compute("p", "", "", "");
        cache
            .put(key, make_entry("v"))
            .await
            .expect("put should succeed");
        cache.clear().await.expect("clear should succeed");
        assert_eq!(cache.len().await.expect("len"), 0);
    }

    #[tokio::test]
    async fn test_disabled_cache() {
        let config = CacheConfig {
            enabled: false,
            ..small_config()
        };
        let cache = LruCacheImpl::new(config);
        let key = CacheKey::compute("p", "", "", "");
        cache
            .put(key.clone(), make_entry("v"))
            .await
            .expect("put should succeed");
        assert!(cache.get(&key).await.expect("get").is_none());
    }
}
