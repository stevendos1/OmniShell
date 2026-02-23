//! # Orchestrator Domain
//!
//! Defines the top-level orchestrator port and the planner/router/aggregator
//! sub-ports.
//!
//! There is **one** orchestrator. Internally it delegates to workers (agents)
//! through the planner -> router -> queue -> agent pipeline.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::agent::AgentResponse;
use crate::domain::error::Result;
use crate::domain::task::ExecutionPlan;

// ---------------------------------------------------------------------------
// User-facing request / response
// ---------------------------------------------------------------------------

/// A user request to the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRequest {
    /// Unique request identifier.
    pub id: String,
    /// Session identifier (for context continuity).
    pub session_id: String,
    /// The user's message / task description.
    pub message: String,
    /// Optional: which capability the user wants (if they know).
    pub preferred_capability: Option<String>,
    /// Maximum tokens the user wants to spend.
    pub max_tokens: Option<u64>,
}

/// The aggregated response returned to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateResponse {
    /// The original request ID.
    pub request_id: String,
    /// Final combined content.
    pub content: String,
    /// Structured data (if any).
    pub structured_data: Option<serde_json::Value>,
    /// Per-worker results for traceability.
    pub worker_results: Vec<WorkerResult>,
    /// Total estimated tokens consumed.
    pub total_tokens: u64,
    /// Total wall-clock time.
    pub total_duration: Duration,
    /// Cache statistics.
    pub cache_stats: CacheStats,
}

/// Result from a single worker, used for traceability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    /// Which agent handled this.
    pub agent_id: String,
    /// Subtask description.
    pub subtask_description: String,
    /// Duration for this subtask.
    pub duration: Duration,
    /// Estimated tokens consumed.
    pub estimated_tokens: u64,
    /// Whether this was a cache hit.
    pub cache_hit: bool,
    /// Errors/warnings (empty if success).
    pub errors: Vec<String>,
}

/// Cache hit/miss statistics for the aggregate response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of cache hits.
    pub hits: u32,
    /// Number of cache misses.
    pub misses: u32,
}

// ---------------------------------------------------------------------------
// Planner port
// ---------------------------------------------------------------------------

/// Port for generating execution plans from user requests.
///
/// The planner can be deterministic (rule-based) or use an AI agent.
/// The implementation is pluggable.
#[async_trait::async_trait]
pub trait Planner: Send + Sync {
    /// Generate an execution plan for the given user request.
    ///
    /// # Errors
    /// Returns `OrchestratorError::AgentError` if a planner agent fails,
    /// or `OrchestratorError::InvalidConfig` if no suitable plan can be made.
    async fn plan(&self, request: &UserRequest) -> Result<ExecutionPlan>;
}

// ---------------------------------------------------------------------------
// Router port
// ---------------------------------------------------------------------------

/// Port for routing subtasks to agents based on capabilities.
#[async_trait::async_trait]
pub trait Router: Send + Sync {
    /// Given a required capability, return the ID of the best matching agent.
    ///
    /// # Errors
    /// Returns `OrchestratorError::InvalidConfig` if no agent matches.
    async fn route(&self, capability: &str) -> Result<String>;

    /// Return all agent IDs that match a capability.
    async fn all_matching(&self, capability: &str) -> Result<Vec<String>>;
}

// ---------------------------------------------------------------------------
// Aggregator port
// ---------------------------------------------------------------------------

/// Port for aggregating multiple agent responses into a single response.
#[async_trait::async_trait]
pub trait Aggregator: Send + Sync {
    /// Combine multiple agent responses into an aggregate.
    ///
    /// # Errors
    /// Returns `OrchestratorError::AgentError` if aggregation logic fails.
    async fn aggregate(
        &self,
        request_id: &str,
        responses: Vec<AgentResponse>,
    ) -> Result<AggregateResponse>;
}

// ---------------------------------------------------------------------------
// Orchestrator port (top-level)
// ---------------------------------------------------------------------------

/// The top-level orchestrator port.
///
/// There is exactly **one** orchestrator in the system. It:
/// 1. Receives a `UserRequest`.
/// 2. Plans subtasks via the `Planner`.
/// 3. Routes subtasks via the `Router`.
/// 4. Enqueues subtasks and dispatches to agents.
/// 5. Aggregates responses via the `Aggregator`.
/// 6. Manages context, cache, and token budgets.
///
/// # Errors
/// Propagates errors from all subsystems.
#[async_trait::async_trait]
pub trait Orchestrator: Send + Sync {
    /// Process a user request end-to-end.
    async fn process(&self, request: UserRequest) -> Result<AggregateResponse>;

    /// List currently active (enabled) agent IDs.
    async fn active_agents(&self) -> Result<Vec<String>>;

    /// Health-check all agents and return a map of agent_id -> status.
    async fn health_check(&self) -> Result<Vec<(String, bool)>>;
}
