//! # In-Memory Long-Term Memory Store
//!
//! Simple in-memory implementation of `MemoryStore`.
//! For production, replace with a persistent backend.

use std::collections::HashMap;

use tokio::sync::RwLock;

use crate::domain::context::MemoryStore;
use crate::domain::error::Result;

/// In-memory implementation of `MemoryStore`.
///
/// Data is lost when the process exits. This is suitable for
/// development and testing. For persistence, implement `MemoryStore`
/// with a file or database backend.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::memory_store::InMemoryStore;
///
/// let store = InMemoryStore::new();
/// ```
pub struct InMemoryStore {
    data: RwLock<HashMap<String, HashMap<String, String>>>,
}

impl InMemoryStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryStore for InMemoryStore {
    async fn store(&self, namespace: &str, key: &str, value: &str) -> Result<()> {
        let mut data = self.data.write().await;
        data.entry(namespace.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn retrieve(&self, namespace: &str, key: &str) -> Result<Option<String>> {
        let data = self.data.read().await;
        Ok(data.get(namespace).and_then(|ns| ns.get(key)).cloned())
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let data = self.data.read().await;
        Ok(data
            .get(namespace)
            .map(|ns| ns.keys().cloned().collect())
            .unwrap_or_default())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let mut data = self.data.write().await;
        if let Some(ns) = data.get_mut(namespace) {
            ns.remove(key);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let store = InMemoryStore::new();
        store
            .store("ns1", "key1", "value1")
            .await
            .expect("store should succeed");
        let result = store
            .retrieve("ns1", "key1")
            .await
            .expect("retrieve should succeed");
        assert_eq!(result, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_retrieve_missing() {
        let store = InMemoryStore::new();
        let result = store
            .retrieve("ns1", "missing")
            .await
            .expect("retrieve should succeed");
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_list_keys() {
        let store = InMemoryStore::new();
        store.store("ns1", "a", "1").await.expect("ok");
        store.store("ns1", "b", "2").await.expect("ok");
        let mut keys = store.list_keys("ns1").await.expect("ok");
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = InMemoryStore::new();
        store.store("ns1", "key", "val").await.expect("ok");
        store.delete("ns1", "key").await.expect("ok");
        assert_eq!(store.retrieve("ns1", "key").await.expect("ok"), None);
    }
}
