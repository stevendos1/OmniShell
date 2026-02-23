//! Builder for constructing an `OrchestratorService`.

use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::application::circuit_breaker::CircuitBreaker;
use crate::application::router::CapabilityRouter;
use crate::domain::cache::Cache;
use crate::domain::context::ContextManager;
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::orchestrator::*;
use crate::domain::policy::PolicyGuard;
use crate::domain::task::{RetryPolicy, TimeoutPolicy};
use crate::domain::token::TokenCounter;

use super::OrchestratorService;

pub struct OrchestratorServiceBuilder {
    planner: Option<Arc<dyn Planner>>, router: Option<Arc<CapabilityRouter>>,
    aggregator: Option<Arc<dyn Aggregator>>, context_manager: Option<Arc<dyn ContextManager>>,
    cache: Option<Arc<dyn Cache>>, token_counter: Option<Arc<dyn TokenCounter>>,
    policy_guard: Option<Arc<dyn PolicyGuard>>, circuit_breaker: Option<CircuitBreaker>,
    retry_policy: RetryPolicy, timeout_policy: TimeoutPolicy,
    max_concurrency: usize, config_version: String,
}

impl OrchestratorServiceBuilder {
    pub fn new() -> Self {
        Self { planner: None, router: None, aggregator: None, context_manager: None, cache: None,
            token_counter: None, policy_guard: None, circuit_breaker: None,
            retry_policy: RetryPolicy::default(), timeout_policy: TimeoutPolicy::default(),
            max_concurrency: 10, config_version: "v1".to_string() }
    }
    pub fn planner(mut self, p: Arc<dyn Planner>) -> Self { self.planner = Some(p); self }
    pub fn router(mut self, r: Arc<CapabilityRouter>) -> Self { self.router = Some(r); self }
    pub fn aggregator(mut self, a: Arc<dyn Aggregator>) -> Self { self.aggregator = Some(a); self }
    pub fn context_manager(mut self, cm: Arc<dyn ContextManager>) -> Self { self.context_manager = Some(cm); self }
    pub fn cache(mut self, c: Arc<dyn Cache>) -> Self { self.cache = Some(c); self }
    pub fn token_counter(mut self, tc: Arc<dyn TokenCounter>) -> Self { self.token_counter = Some(tc); self }
    pub fn policy_guard(mut self, pg: Arc<dyn PolicyGuard>) -> Self { self.policy_guard = Some(pg); self }
    pub fn circuit_breaker(mut self, cb: CircuitBreaker) -> Self { self.circuit_breaker = Some(cb); self }
    pub fn retry_policy(mut self, rp: RetryPolicy) -> Self { self.retry_policy = rp; self }
    pub fn timeout_policy(mut self, tp: TimeoutPolicy) -> Self { self.timeout_policy = tp; self }
    pub fn max_concurrency(mut self, n: usize) -> Self { self.max_concurrency = n; self }
    pub fn config_version(mut self, v: impl Into<String>) -> Self { self.config_version = v.into(); self }

    pub fn build(self) -> Result<OrchestratorService> {
        let missing = |name: &str| OrchestratorError::InvalidConfig(format!("{name} is required but not set"));
        Ok(OrchestratorService {
            planner: self.planner.ok_or_else(|| missing("planner"))?,
            router: self.router.ok_or_else(|| missing("router"))?,
            aggregator: self.aggregator.ok_or_else(|| missing("aggregator"))?,
            context_manager: self.context_manager.ok_or_else(|| missing("context_manager"))?,
            cache: self.cache.ok_or_else(|| missing("cache"))?,
            token_counter: self.token_counter.ok_or_else(|| missing("token_counter"))?,
            policy_guard: self.policy_guard.ok_or_else(|| missing("policy_guard"))?,
            circuit_breaker: self.circuit_breaker.unwrap_or_else(|| CircuitBreaker::new(Default::default())),
            retry_policy: self.retry_policy, timeout_policy: self.timeout_policy,
            concurrency_semaphore: Arc::new(Semaphore::new(self.max_concurrency)),
            config_version: self.config_version,
        })
    }
}

impl Default for OrchestratorServiceBuilder { fn default() -> Self { Self::new() } }
