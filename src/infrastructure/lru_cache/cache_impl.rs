//! LruCacheImpl — Cache trait implementation.

use tokio::sync::RwLock;
use tracing::debug;

use crate::domain::cache::*;
use crate::domain::error::Result;

use super::state::{LruNode, LruState};

/// In-memory LRU cache.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::lru_cache::LruCacheImpl;
/// use omnishell_orchestrator::domain::cache::CacheConfig;
/// let cache = LruCacheImpl::new(CacheConfig::default());
/// ```
pub struct LruCacheImpl {
    config: CacheConfig,
    state: RwLock<LruState>,
}

impl LruCacheImpl {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            state: RwLock::new(LruState::new()),
        }
    }
    fn is_expired(&self, entry: &CacheEntry) -> bool {
        self.config.ttl_seconds > 0
            && (chrono::Utc::now().timestamp() - entry.created_at) as u64 > self.config.ttl_seconds
    }
}

#[async_trait::async_trait]
impl Cache for LruCacheImpl {
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let mut s = self.state.write().await;
        if s.map
            .get(key)
            .map(|n| self.is_expired(&n.entry))
            .unwrap_or(false)
        {
            let bs = s.map.get(key).map(|n| n.entry.byte_size).unwrap_or(0);
            s.unlink(key);
            s.map.remove(key);
            s.total_bytes = s.total_bytes.saturating_sub(bs);
            return Ok(None);
        }
        if s.map.contains_key(key) {
            s.unlink(key);
            s.push_front(key);
            if let Some(n) = s.map.get_mut(key) {
                n.entry.hit_count += 1;
                return Ok(Some(n.entry.clone()));
            }
        }
        Ok(None)
    }
    async fn put(&self, key: CacheKey, entry: CacheEntry) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let mut s = self.state.write().await;
        if s.map.contains_key(&key) {
            let ob = s.map.get(&key).map(|n| n.entry.byte_size).unwrap_or(0);
            s.unlink(&key);
            s.map.remove(&key);
            s.total_bytes = s.total_bytes.saturating_sub(ob);
        }
        while s.map.len() >= self.config.max_entries
            || s.total_bytes + entry.byte_size > self.config.max_bytes
        {
            if let Some(eb) = s.evict_oldest() {
                s.total_bytes = s.total_bytes.saturating_sub(eb);
                debug!("LRU evicted {eb}B");
            } else {
                break;
            }
        }
        let bs = entry.byte_size;
        s.map.insert(
            key.clone(),
            LruNode {
                entry,
                newer: None,
                older: None,
            },
        );
        s.push_front(&key);
        s.total_bytes += bs;
        Ok(())
    }
    async fn remove(&self, key: &CacheKey) -> Result<()> {
        let mut s = self.state.write().await;
        if s.map.contains_key(key) {
            let bs = s.map.get(key).map(|n| n.entry.byte_size).unwrap_or(0);
            s.unlink(key);
            s.map.remove(key);
            s.total_bytes = s.total_bytes.saturating_sub(bs);
        }
        Ok(())
    }
    async fn clear(&self) -> Result<()> {
        let mut s = self.state.write().await;
        s.map.clear();
        s.newest = None;
        s.oldest = None;
        s.total_bytes = 0;
        Ok(())
    }
    async fn len(&self) -> Result<usize> {
        Ok(self.state.read().await.map.len())
    }
    async fn is_empty(&self) -> Result<bool> {
        Ok(self.state.read().await.map.is_empty())
    }
    async fn byte_size(&self) -> Result<usize> {
        Ok(self.state.read().await.total_bytes)
    }
}
