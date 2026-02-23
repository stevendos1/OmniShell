//! Orchestrator sub-ports: Planner, Router, Aggregator, Orchestrator.

use crate::domain::agent::AgentResponse;
use crate::domain::error::Result;
use crate::domain::task::ExecutionPlan;

use super::{AggregateResponse, UserRequest};

/// Port for generating execution plans from user requests.
#[async_trait::async_trait]
pub trait Planner: Send + Sync {
    async fn plan(&self, request: &UserRequest) -> Result<ExecutionPlan>;
}

/// Port for routing subtasks to agents based on capabilities.
#[async_trait::async_trait]
pub trait Router: Send + Sync {
    /// Return the ID of the best matching agent.
    async fn route(&self, capability: &str) -> Result<String>;
    /// Return all agent IDs that match a capability.
    async fn all_matching(&self, capability: &str) -> Result<Vec<String>>;
}

/// Port for aggregating multiple agent responses.
#[async_trait::async_trait]
pub trait Aggregator: Send + Sync {
    async fn aggregate(&self, request_id: &str, responses: Vec<AgentResponse>) -> Result<AggregateResponse>;
}

/// The top-level orchestrator port.
///
/// There is exactly **one** orchestrator. It:
/// 1. Receives a `UserRequest`.
/// 2. Plans subtasks via the `Planner`.
/// 3. Routes subtasks via the `Router`.
/// 4. Dispatches to agents and aggregates results.
/// 5. Manages context, cache, and token budgets.
#[async_trait::async_trait]
pub trait Orchestrator: Send + Sync {
    async fn process(&self, request: UserRequest) -> Result<AggregateResponse>;
    async fn active_agents(&self) -> Result<Vec<String>>;
    async fn health_check(&self) -> Result<Vec<(String, bool)>>;
}
