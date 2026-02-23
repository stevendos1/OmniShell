//! # Cache Domain
//!
//! Defines the port for response caching and the cache key type.

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;

/// A cache key computed from the normalized prompt, context, agent id, and config version.
///
/// # Construction
/// Use [`CacheKey::compute`] to build a key from its components.
/// The key is a hex-encoded SHA-256 hash.
///
/// # Example
/// ```
/// use omnishell_orchestrator::domain::cache::CacheKey;
/// let key = CacheKey::compute("hello", "ctx", "agent-1", "v1");
/// assert_eq!(key.as_str().len(), 64); // SHA-256 hex
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey(String);

impl CacheKey {
    /// Compute a cache key by hashing all components.
    pub fn compute(
        normalized_prompt: &str,
        relevant_context: &str,
        agent_id: &str,
        config_version: &str,
    ) -> Self {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(normalized_prompt.as_bytes());
        hasher.update(b"|");
        hasher.update(relevant_context.as_bytes());
        hasher.update(b"|");
        hasher.update(agent_id.as_bytes());
        hasher.update(b"|");
        hasher.update(config_version.as_bytes());
        let hash = hasher.finalize();
        Self(hex::encode(hash))
    }

    /// The hex string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A cached entry storing the response and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The cached response (serialized).
    pub value: String,
    /// When this entry was created (Unix timestamp).
    pub created_at: i64,
    /// How many times this entry has been hit.
    pub hit_count: u64,
    /// Estimated byte size of the value.
    pub byte_size: usize,
}

/// Configuration for the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled.
    pub enabled: bool,
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Maximum total bytes across all entries.
    pub max_bytes: usize,
    /// TTL in seconds (0 = no expiration).
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
            max_bytes: 50 * 1024 * 1024, // 50 MiB
            ttl_seconds: 3600,
        }
    }
}

/// Port for the response cache.
///
/// Implementations should be LRU-based and respect the configured limits.
///
/// # Errors
/// Returns `OrchestratorError::CacheError` on serialization or internal failures.
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// Look up a cached response.
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>>;

    /// Insert or update a cache entry.
    async fn put(&self, key: CacheKey, entry: CacheEntry) -> Result<()>;

    /// Remove a specific entry.
    async fn remove(&self, key: &CacheKey) -> Result<()>;

    /// Clear the entire cache.
    async fn clear(&self) -> Result<()>;

    /// Current number of entries.
    async fn len(&self) -> Result<usize>;

    /// Current total byte usage.
    async fn byte_size(&self) -> Result<usize>;
}
