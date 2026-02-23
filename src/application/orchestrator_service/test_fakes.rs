//! Test fakes for the orchestrator service tests.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use super::builder::OrchestratorServiceBuilder;
use super::OrchestratorService;
use crate::application::aggregator::ConcatAggregator;
use crate::application::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::application::context_service::InMemoryContextManager;
use crate::application::planner::DeterministicPlanner;
use crate::application::router::CapabilityRouter;
use crate::domain::agent::*;
use crate::domain::cache::*;
use crate::domain::context::ContextConfig;
use crate::domain::error::Result as OrcResult;
use crate::domain::orchestrator::*;
use crate::domain::policy::*;
use crate::domain::token::SimpleTokenCounter;

pub(crate) struct FakeAgent {
    pub info: AgentInfo,
    pub response_content: String,
}
impl FakeAgent {
    pub fn new(id: &str, caps: &[&str], content: &str) -> Self {
        Self {
            info: AgentInfo {
                id: id.into(),
                display_name: id.into(),
                capabilities: caps
                    .iter()
                    .map(|c| AgentCapability::new(*c))
                    .collect::<HashSet<_>>(),
                max_concurrency: 2,
                default_timeout: Duration::from_secs(60),
                enabled: true,
                priority: 1,
            },
            response_content: content.into(),
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
            content: self.response_content.clone(),
            structured_data: None,
            estimated_tokens: 50,
            duration: Duration::from_millis(10),
            cache_hit: false,
            warnings: Vec::new(),
        })
    }
    async fn health_check(&self) -> OrcResult<()> {
        Ok(())
    }
}

pub(crate) struct FakeCache;
#[async_trait::async_trait]
impl Cache for FakeCache {
    async fn get(&self, _: &CacheKey) -> OrcResult<Option<CacheEntry>> {
        Ok(None)
    }
    async fn put(&self, _: CacheKey, _: CacheEntry) -> OrcResult<()> {
        Ok(())
    }
    async fn remove(&self, _: &CacheKey) -> OrcResult<()> {
        Ok(())
    }
    async fn clear(&self) -> OrcResult<()> {
        Ok(())
    }
    async fn len(&self) -> OrcResult<usize> {
        Ok(0)
    }
    async fn is_empty(&self) -> OrcResult<bool> {
        Ok(true)
    }
    async fn byte_size(&self) -> OrcResult<usize> {
        Ok(0)
    }
}

pub(crate) struct FakePolicyGuard;
impl PolicyGuard for FakePolicyGuard {
    fn check_user_input(&self, _: &str) -> OrcResult<PolicyCheckResult> {
        Ok(PolicyCheckResult::pass())
    }
    fn check_tool_request(&self, _: &str, _: &[String]) -> OrcResult<PolicyCheckResult> {
        Ok(PolicyCheckResult::pass())
    }
    fn check_agent_output(&self, _: &str) -> OrcResult<PolicyCheckResult> {
        Ok(PolicyCheckResult::pass())
    }
    fn redact(&self, text: &str) -> String {
        text.to_string()
    }
}

pub(crate) async fn build_test_orchestrator(
    content: &str,
) -> (OrchestratorService, Arc<CapabilityRouter>) {
    let tc: Arc<dyn crate::domain::token::TokenCounter> = Arc::new(SimpleTokenCounter);
    let planner: Arc<dyn Planner> = Arc::new(DeterministicPlanner::new(
        tc.clone(),
        "code-generation".into(),
    ));
    let router = Arc::new(CapabilityRouter::new());
    let agg: Arc<dyn Aggregator> = Arc::new(ConcatAggregator::default());
    let ctx: Arc<dyn crate::domain::context::ContextManager> = Arc::new(
        InMemoryContextManager::new(ContextConfig::default(), tc.clone()),
    );
    let (cache, pg): (Arc<dyn Cache>, Arc<dyn PolicyGuard>) =
        (Arc::new(FakeCache), Arc::new(FakePolicyGuard));
    router
        .register(Arc::new(FakeAgent::new(
            "test-agent",
            &["code-generation"],
            content,
        )))
        .await;
    let svc = OrchestratorServiceBuilder::new()
        .planner(planner)
        .router(router.clone())
        .aggregator(agg)
        .context_manager(ctx)
        .cache(cache)
        .token_counter(tc)
        .policy_guard(pg)
        .circuit_breaker(CircuitBreaker::new(CircuitBreakerConfig::default()))
        .max_concurrency(4)
        .build()
        .expect("builder should succeed");
    (svc, router)
}
