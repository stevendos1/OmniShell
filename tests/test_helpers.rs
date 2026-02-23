//! Shared test helpers for integration tests.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use omnishell_orchestrator::application::aggregator::ConcatAggregator;
use omnishell_orchestrator::application::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use omnishell_orchestrator::application::context_service::InMemoryContextManager;
use omnishell_orchestrator::application::orchestrator_service::OrchestratorServiceBuilder;
use omnishell_orchestrator::application::planner::DeterministicPlanner;
use omnishell_orchestrator::application::router::CapabilityRouter;
use omnishell_orchestrator::domain::agent::*;
use omnishell_orchestrator::domain::cache::Cache;
use omnishell_orchestrator::domain::context::ContextConfig;
use omnishell_orchestrator::domain::error::Result as OrcResult;
use omnishell_orchestrator::domain::orchestrator::{Aggregator, Orchestrator, Planner};
use omnishell_orchestrator::domain::policy::PolicyGuard;
use omnishell_orchestrator::domain::token::{SimpleTokenCounter, TokenCounter};
use omnishell_orchestrator::infrastructure::lru_cache::LruCacheImpl;
use omnishell_orchestrator::infrastructure::policy_guard::DefaultPolicyGuard;

pub struct FakeAgent {
    info: AgentInfo,
    response: String,
}

impl FakeAgent {
    pub fn new(id: &str, caps: &[&str], response: &str) -> Self {
        Self {
            info: AgentInfo {
                id: id.into(),
                display_name: id.into(),
                capabilities: caps
                    .iter()
                    .map(|c| AgentCapability::new(*c))
                    .collect::<HashSet<_>>(),
                max_concurrency: 2,
                default_timeout: Duration::from_secs(30),
                enabled: true,
                priority: 1,
            },
            response: response.into(),
        }
    }
}

#[async_trait::async_trait]
impl AiAgent for FakeAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }
    async fn execute(&self, req: AgentRequest) -> OrcResult<AgentResponse> {
        Ok(AgentResponse {
            request_id: req.request_id,
            agent_id: self.info.id.clone(),
            content: self.response.clone(),
            structured_data: None,
            estimated_tokens: (self.response.len() as u64).div_ceil(4),
            duration: Duration::from_millis(5),
            cache_hit: false,
            warnings: Vec::new(),
        })
    }
    async fn health_check(&self) -> OrcResult<()> {
        Ok(())
    }
}

pub async fn build_orchestrator(agents: Vec<Arc<dyn AiAgent>>) -> impl Orchestrator {
    let tc: Arc<dyn TokenCounter> = Arc::new(SimpleTokenCounter);
    let router = Arc::new(CapabilityRouter::new());
    for a in agents {
        router.register(a).await;
    }
    let planner: Arc<dyn Planner> =
        Arc::new(DeterministicPlanner::new(tc.clone(), "general".into()));
    let agg: Arc<dyn Aggregator> = Arc::new(ConcatAggregator::default());
    let ctx: Arc<dyn omnishell_orchestrator::domain::context::ContextManager> = Arc::new(
        InMemoryContextManager::new(ContextConfig::default(), tc.clone()),
    );
    let cache: Arc<dyn Cache> = Arc::new(LruCacheImpl::new(Default::default()));
    let pg: Arc<dyn PolicyGuard> = Arc::new(DefaultPolicyGuard::new(Default::default()).unwrap());
    OrchestratorServiceBuilder::new()
        .planner(planner)
        .router(router)
        .aggregator(agg)
        .context_manager(ctx)
        .cache(cache)
        .token_counter(tc)
        .policy_guard(pg)
        .circuit_breaker(CircuitBreaker::new(CircuitBreakerConfig::default()))
        .max_concurrency(4)
        .build()
        .unwrap()
}
