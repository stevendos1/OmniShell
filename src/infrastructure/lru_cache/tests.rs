//! LRU cache unit tests.

use crate::domain::cache::*;

use super::LruCacheImpl;

fn small_config() -> CacheConfig {
    CacheConfig {
        enabled: true,
        max_entries: 3,
        max_bytes: 10_000,
        ttl_seconds: 0,
    }
}

fn make_entry(value: &str) -> CacheEntry {
    CacheEntry {
        value: value.into(),
        created_at: chrono::Utc::now().timestamp(),
        hit_count: 0,
        byte_size: value.len(),
    }
}

#[tokio::test]
async fn test_put_and_get() {
    let cache = LruCacheImpl::new(small_config());
    let key = CacheKey::compute("p", "c", "a", "v1");
    cache
        .put(key.clone(), make_entry("response"))
        .await
        .unwrap();
    let r = cache.get(&key).await.unwrap();
    assert!(r.is_some());
    assert_eq!(r.as_ref().map(|e| e.value.as_str()), Some("response"));
    assert_eq!(r.as_ref().map(|e| e.hit_count), Some(1));
}

#[tokio::test]
async fn test_eviction_by_count() {
    let cache = LruCacheImpl::new(small_config());
    for i in 0..5 {
        cache
            .put(
                CacheKey::compute(&format!("p{i}"), "", "", ""),
                make_entry(&format!("v{i}")),
            )
            .await
            .unwrap();
    }
    assert_eq!(cache.len().await.unwrap(), 3);
    assert!(cache
        .get(&CacheKey::compute("p0", "", "", ""))
        .await
        .unwrap()
        .is_none());
    assert!(cache
        .get(&CacheKey::compute("p4", "", "", ""))
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_remove() {
    let cache = LruCacheImpl::new(small_config());
    let k = CacheKey::compute("p", "", "", "");
    cache.put(k.clone(), make_entry("v")).await.unwrap();
    cache.remove(&k).await.unwrap();
    assert!(cache.get(&k).await.unwrap().is_none());
}

#[tokio::test]
async fn test_clear() {
    let cache = LruCacheImpl::new(small_config());
    cache
        .put(CacheKey::compute("p", "", "", ""), make_entry("v"))
        .await
        .unwrap();
    cache.clear().await.unwrap();
    assert_eq!(cache.len().await.unwrap(), 0);
}

#[tokio::test]
async fn test_disabled_cache() {
    let cache = LruCacheImpl::new(CacheConfig {
        enabled: false,
        ..small_config()
    });
    let k = CacheKey::compute("p", "", "", "");
    cache.put(k.clone(), make_entry("v")).await.unwrap();
    assert!(cache.get(&k).await.unwrap().is_none());
}
