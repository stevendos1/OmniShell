//! Cache port (trait).

use crate::domain::error::Result;

use super::{CacheEntry, CacheKey};

/// Port for the response cache.
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>>;
    async fn put(&self, key: CacheKey, entry: CacheEntry) -> Result<()>;
    async fn remove(&self, key: &CacheKey) -> Result<()>;
    async fn clear(&self) -> Result<()>;
    async fn len(&self) -> Result<usize>;
    async fn is_empty(&self) -> Result<bool>;
    async fn byte_size(&self) -> Result<usize>;
}
