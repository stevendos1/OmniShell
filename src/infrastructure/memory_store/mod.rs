//! In-memory long-term store.

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use tokio::sync::RwLock;

use crate::domain::context::MemoryStore;
use crate::domain::error::Result;

/// In-memory implementation of `MemoryStore`.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::memory_store::InMemoryStore;
/// let store = InMemoryStore::new();
/// ```
pub struct InMemoryStore {
    data: RwLock<HashMap<String, HashMap<String, String>>>,
}

impl InMemoryStore {
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
    async fn store(&self, ns: &str, key: &str, value: &str) -> Result<()> {
        self.data
            .write()
            .await
            .entry(ns.into())
            .or_default()
            .insert(key.into(), value.into());
        Ok(())
    }
    async fn retrieve(&self, ns: &str, key: &str) -> Result<Option<String>> {
        Ok(self
            .data
            .read()
            .await
            .get(ns)
            .and_then(|m| m.get(key))
            .cloned())
    }
    async fn list_keys(&self, ns: &str) -> Result<Vec<String>> {
        Ok(self
            .data
            .read()
            .await
            .get(ns)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default())
    }
    async fn delete(&self, ns: &str, key: &str) -> Result<()> {
        if let Some(m) = self.data.write().await.get_mut(ns) {
            m.remove(key);
        }
        Ok(())
    }
}
