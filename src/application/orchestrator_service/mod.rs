//! Orchestrator service — the core use case.

mod builder;
mod process;
mod worker;

#[cfg(test)]
mod test_fakes;
#[cfg(test)]
mod tests;

pub use builder::OrchestratorServiceBuilder;

use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::application::circuit_breaker::CircuitBreaker;
use crate::application::router::CapabilityRouter;
use crate::domain::cache::Cache;
use crate::domain::context::ContextManager;
use crate::domain::orchestrator::*;
use crate::domain::policy::PolicyGuard;
use crate::domain::task::{RetryPolicy, TimeoutPolicy};
use crate::domain::token::TokenCounter;

/// The orchestrator service — routes tasks, manages context,
/// enforces budgets, and aggregates results.
pub struct OrchestratorService {
    pub(crate) planner: Arc<dyn Planner>,
    pub(crate) router: Arc<CapabilityRouter>,
    pub(crate) aggregator: Arc<dyn Aggregator>,
    pub(crate) context_manager: Arc<dyn ContextManager>,
    pub(crate) cache: Arc<dyn Cache>,
    pub(crate) token_counter: Arc<dyn TokenCounter>,
    pub(crate) policy_guard: Arc<dyn PolicyGuard>,
    pub(crate) circuit_breaker: CircuitBreaker,
    pub(crate) retry_policy: RetryPolicy,
    pub(crate) timeout_policy: TimeoutPolicy,
    pub(crate) concurrency_semaphore: Arc<Semaphore>,
    pub(crate) config_version: String,
}
