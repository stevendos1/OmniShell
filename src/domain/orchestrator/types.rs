//! User-facing request/response types and worker result.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A user request to the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRequest {
    pub id: String,
    pub session_id: String,
    pub message: String,
    pub preferred_capability: Option<String>,
    pub max_tokens: Option<u64>,
}

/// The aggregated response returned to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateResponse {
    pub request_id: String,
    pub content: String,
    pub structured_data: Option<serde_json::Value>,
    pub worker_results: Vec<WorkerResult>,
    pub total_tokens: u64,
    pub total_duration: Duration,
    pub cache_stats: CacheStats,
}

/// Result from a single worker, used for traceability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub agent_id: String,
    pub subtask_description: String,
    pub duration: Duration,
    pub estimated_tokens: u64,
    pub cache_hit: bool,
    pub errors: Vec<String>,
}

/// Cache hit/miss statistics for the aggregate response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u32,
    pub misses: u32,
}
