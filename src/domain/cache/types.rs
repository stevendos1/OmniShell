//! Cache key, entry, and configuration types.

use serde::{Deserialize, Serialize};

/// A cache key computed from normalized prompt, context, agent id, and config version.
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

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A cached entry storing the response and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub value: String,
    pub created_at: i64,
    pub hit_count: u64,
    pub byte_size: usize,
}

/// Configuration for the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub max_bytes: usize,
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
            max_bytes: 50 * 1024 * 1024,
            ttl_seconds: 3600,
        }
    }
}
