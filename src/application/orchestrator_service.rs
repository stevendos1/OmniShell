//! # Orchestrator Service
//!
//! The single orchestrator implementation. It coordinates:
//! planner → router → dispatch → agents → aggregator.
//!
//! This is the core use case of the application layer.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{error, info, instrument, warn};

use crate::application::circuit_breaker::CircuitBreaker;
use crate::application::router::CapabilityRouter;
use crate::domain::agent::{AgentRequest, AgentResponse, AiAgent};
use crate::domain::cache::{Cache, CacheEntry, CacheKey};
use crate::domain::context::ContextManager;
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::orchestrator::*;
use crate::domain::policy::PolicyGuard;
use crate::domain::task::{RetryPolicy, SubTask, TimeoutPolicy};
use crate::domain::token::TokenCounter;

/// The orchestrator service — the single hub that routes tasks,
/// manages context, enforces budgets, and aggregates results.
///
/// # Architecture
/// ```text
/// UserRequest
///   → PolicyGuard (validate input)
///   → Planner (split into subtasks)
///   → Router (assign agents)
///   → Dispatch (parallel execution with limits)
///       → Cache check
///       → Agent execution
///       → Circuit breaker tracking
///   → Aggregator (combine results)
///   → ContextManager (update history)
/// → AggregateResponse
/// ```
pub struct OrchestratorService {
    planner: Arc<dyn Planner>,
    router: Arc<CapabilityRouter>,
    aggregator: Arc<dyn Aggregator>,
    context_manager: Arc<dyn ContextManager>,
    cache: Arc<dyn Cache>,
    token_counter: Arc<dyn TokenCounter>,
    policy_guard: Arc<dyn PolicyGuard>,
    circuit_breaker: CircuitBreaker,
    retry_policy: RetryPolicy,
    timeout_policy: TimeoutPolicy,
    /// Global concurrency limiter.
    concurrency_semaphore: Arc<Semaphore>,
    config_version: String,
}

/// Builder for constructing an `OrchestratorService`.
///
/// # Example
/// ```no_run
/// use omnishell_orchestrator::application::orchestrator_service::OrchestratorServiceBuilder;
///
/// // let service = OrchestratorServiceBuilder::new()
/// //     .planner(planner)
/// //     .router(router)
/// //     ... etc
/// //     .build()?;
/// ```
pub struct OrchestratorServiceBuilder {
    planner: Option<Arc<dyn Planner>>,
    router: Option<Arc<CapabilityRouter>>,
    aggregator: Option<Arc<dyn Aggregator>>,
    context_manager: Option<Arc<dyn ContextManager>>,
    cache: Option<Arc<dyn Cache>>,
    token_counter: Option<Arc<dyn TokenCounter>>,
    policy_guard: Option<Arc<dyn PolicyGuard>>,
    circuit_breaker: Option<CircuitBreaker>,
    retry_policy: RetryPolicy,
    timeout_policy: TimeoutPolicy,
    max_concurrency: usize,
    config_version: String,
}

impl OrchestratorServiceBuilder {
    /// Start building a new orchestrator service.
    pub fn new() -> Self {
        Self {
            planner: None,
            router: None,
            aggregator: None,
            context_manager: None,
            cache: None,
            token_counter: None,
            policy_guard: None,
            circuit_breaker: None,
            retry_policy: RetryPolicy::default(),
            timeout_policy: TimeoutPolicy::default(),
            max_concurrency: 10,
            config_version: "v1".to_string(),
        }
    }

    /// Set the planner.
    pub fn planner(mut self, p: Arc<dyn Planner>) -> Self {
        self.planner = Some(p);
        self
    }

    /// Set the router.
    pub fn router(mut self, r: Arc<CapabilityRouter>) -> Self {
        self.router = Some(r);
        self
    }

    /// Set the aggregator.
    pub fn aggregator(mut self, a: Arc<dyn Aggregator>) -> Self {
        self.aggregator = Some(a);
        self
    }

    /// Set the context manager.
    pub fn context_manager(mut self, cm: Arc<dyn ContextManager>) -> Self {
        self.context_manager = Some(cm);
        self
    }

    /// Set the cache.
    pub fn cache(mut self, c: Arc<dyn Cache>) -> Self {
        self.cache = Some(c);
        self
    }

    /// Set the token counter.
    pub fn token_counter(mut self, tc: Arc<dyn TokenCounter>) -> Self {
        self.token_counter = Some(tc);
        self
    }

    /// Set the policy guard.
    pub fn policy_guard(mut self, pg: Arc<dyn PolicyGuard>) -> Self {
        self.policy_guard = Some(pg);
        self
    }

    /// Set the circuit breaker.
    pub fn circuit_breaker(mut self, cb: CircuitBreaker) -> Self {
        self.circuit_breaker = Some(cb);
        self
    }

    /// Set the retry policy.
    pub fn retry_policy(mut self, rp: RetryPolicy) -> Self {
        self.retry_policy = rp;
        self
    }

    /// Set the timeout policy.
    pub fn timeout_policy(mut self, tp: TimeoutPolicy) -> Self {
        self.timeout_policy = tp;
        self
    }

    /// Set maximum global concurrency.
    pub fn max_concurrency(mut self, n: usize) -> Self {
        self.max_concurrency = n;
        self
    }

    /// Set the config version (used in cache key computation).
    pub fn config_version(mut self, v: impl Into<String>) -> Self {
        self.config_version = v.into();
        self
    }

    /// Build the orchestrator service.
    ///
    /// # Errors
    /// Returns `InvalidConfig` if any required component is missing.
    pub fn build(self) -> Result<OrchestratorService> {
        let missing = |name: &str| {
            OrchestratorError::InvalidConfig(format!("{name} is required but not set"))
        };

        Ok(OrchestratorService {
            planner: self.planner.ok_or_else(|| missing("planner"))?,
            router: self.router.ok_or_else(|| missing("router"))?,
            aggregator: self.aggregator.ok_or_else(|| missing("aggregator"))?,
            context_manager: self
                .context_manager
                .ok_or_else(|| missing("context_manager"))?,
            cache: self.cache.ok_or_else(|| missing("cache"))?,
            token_counter: self.token_counter.ok_or_else(|| missing("token_counter"))?,
            policy_guard: self.policy_guard.ok_or_else(|| missing("policy_guard"))?,
            circuit_breaker: self
                .circuit_breaker
                .unwrap_or_else(|| CircuitBreaker::new(Default::default())),
            retry_policy: self.retry_policy,
            timeout_policy: self.timeout_policy,
            concurrency_semaphore: Arc::new(Semaphore::new(self.max_concurrency)),
            config_version: self.config_version,
        })
    }
}

impl Default for OrchestratorServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl OrchestratorService {
    /// Execute a single subtask against a specific agent, with retries.
    #[instrument(skip(self, agent), fields(agent_id = %agent.info().id, subtask_id = %subtask.id))]
    async fn execute_subtask(
        &self,
        agent: Arc<dyn AiAgent>,
        subtask: &SubTask,
        session_id: &str,
    ) -> Result<AgentResponse> {
        let agent_id = agent.info().id.clone();

        // Check circuit breaker.
        self.circuit_breaker.check(&agent_id).await?;

        // Check cache first.
        let cache_key = CacheKey::compute(
            &subtask.prompt,
            "", // context hash could be added
            &agent_id,
            &self.config_version,
        );

        if let Ok(Some(cached)) = self.cache.get(&cache_key).await {
            info!(agent_id = %agent_id, "cache hit");
            let response: AgentResponse = serde_json::from_str(&cached.value).map_err(|e| {
                OrchestratorError::CacheError(format!("failed to deserialize cached response: {e}"))
            })?;
            return Ok(AgentResponse {
                cache_hit: true,
                ..response
            });
        }

        // Build agent input.
        let agent_input = self
            .context_manager
            .build_agent_input(session_id, &subtask.prompt, &[])
            .await?;

        let request = AgentRequest {
            request_id: subtask.id.clone(),
            system_prompt: self.context_manager.build_system_prompt(session_id).await?,
            user_prompt: subtask.prompt.clone(),
            context: agent_input,
            allowed_tools: Vec::new(),
            max_response_tokens: subtask.max_tokens,
        };

        // Execute with retries.
        let mut last_error = None;
        for attempt in 0..=self.retry_policy.max_retries {
            if attempt > 0 {
                let delay = self.compute_retry_delay(attempt);
                warn!(
                    agent_id = %agent_id,
                    attempt,
                    delay_ms = delay.as_millis() as u64,
                    "retrying subtask"
                );
                tokio::time::sleep(delay).await;
            }

            let task_timeout = subtask
                .timeout
                .unwrap_or(self.timeout_policy.default_timeout);

            let result = timeout(task_timeout, agent.execute(request.clone())).await;

            match result {
                Ok(Ok(response)) => {
                    self.circuit_breaker.record_success(&agent_id).await;

                    // Cache the response.
                    let serialized = serde_json::to_string(&response).map_err(|e| {
                        OrchestratorError::SerializationError(format!(
                            "failed to serialize response for cache: {e}"
                        ))
                    })?;
                    let entry = CacheEntry {
                        value: serialized,
                        created_at: chrono::Utc::now().timestamp(),
                        hit_count: 0,
                        byte_size: response.content.len(),
                    };
                    if let Err(e) = self.cache.put(cache_key, entry).await {
                        warn!("failed to cache response: {e}");
                    }

                    return Ok(response);
                }
                Ok(Err(e)) => {
                    self.circuit_breaker.record_failure(&agent_id).await;
                    error!(
                        agent_id = %agent_id,
                        attempt,
                        error = %e,
                        "agent execution failed"
                    );
                    last_error = Some(e);
                }
                Err(_) => {
                    self.circuit_breaker.record_failure(&agent_id).await;
                    let err = OrchestratorError::Timeout {
                        duration_ms: task_timeout.as_millis() as u64,
                        context: format!("agent {agent_id}, subtask {}", subtask.id),
                    };
                    error!(agent_id = %agent_id, attempt, "agent execution timed out");
                    last_error = Some(err);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| OrchestratorError::agent(&agent_id, "all retries exhausted")))
    }

    /// Compute retry delay using exponential backoff.
    fn compute_retry_delay(&self, attempt: u32) -> Duration {
        let base_ms = self.retry_policy.base_delay.as_millis() as f64;
        let delay_ms = base_ms
            * self
                .retry_policy
                .backoff_multiplier
                .powi(attempt as i32 - 1);
        let capped = Duration::from_millis(delay_ms as u64).min(self.retry_policy.max_delay);
        capped
    }
}

#[async_trait::async_trait]
impl Orchestrator for OrchestratorService {
    #[instrument(skip(self), fields(request_id = %request.id, session_id = %request.session_id))]
    async fn process(&self, request: UserRequest) -> Result<AggregateResponse> {
        info!("processing user request");

        // 1. Policy check on user input.
        let policy_result = self.policy_guard.check_user_input(&request.message)?;
        if !policy_result.allowed {
            return Err(OrchestratorError::PolicyViolation(policy_result.reason));
        }

        // 2. Check token budget.
        let current_tokens = self
            .context_manager
            .estimate_tokens(&request.session_id)
            .await?;
        if let Some(max) = request.max_tokens {
            if current_tokens > max {
                return Err(OrchestratorError::TokenBudgetExceeded {
                    used: current_tokens,
                    limit: max,
                });
            }
        }

        // 3. Add user message to context.
        let msg = crate::domain::context::Message {
            role: crate::domain::context::MessageRole::User,
            content: request.message.clone(),
            token_estimate: self.token_counter.count_tokens(&request.message),
            timestamp: chrono::Utc::now().timestamp(),
        };
        self.context_manager
            .add_message(&request.session_id, msg)
            .await?;

        // 4. Plan subtasks.
        let plan = self.planner.plan(&request).await?;
        info!(subtask_count = plan.subtasks.len(), "plan generated");

        // 5. Route and dispatch subtasks in parallel.
        let mut handles = Vec::new();

        for subtask in &plan.subtasks {
            let agent_id = self
                .router
                .route(subtask.required_capability.name())
                .await?;
            let agent = self.router.get_agent(&agent_id).await.ok_or_else(|| {
                OrchestratorError::agent(&agent_id, "agent registered but not found")
            })?;

            let semaphore = self.concurrency_semaphore.clone();
            let subtask_for_handle = subtask.clone();
            let subtask = subtask.clone();
            let session_id = request.session_id.clone();

            // We need to pass `self` logic into the spawned task.
            // We clone the Arc-wrapped components instead.
            let circuit_breaker = self.circuit_breaker.clone();
            let cache = self.cache.clone();
            let context_manager = self.context_manager.clone();
            let retry_policy = self.retry_policy.clone();
            let timeout_policy = self.timeout_policy.clone();
            let config_version = self.config_version.clone();

            let handle = tokio::spawn(async move {
                // Acquire semaphore permit for concurrency control.
                let _permit = semaphore
                    .acquire()
                    .await
                    .map_err(|_| OrchestratorError::NotImplemented("semaphore closed".into()))?;

                // Inline the execute logic since we can't pass `self` to spawn.
                let agent_id_str = agent.info().id.clone();

                // Check circuit breaker.
                circuit_breaker.check(&agent_id_str).await?;

                // Check cache.
                let cache_key =
                    CacheKey::compute(&subtask.prompt, "", &agent_id_str, &config_version);

                if let Ok(Some(cached)) = cache.get(&cache_key).await {
                    if let Ok(response) = serde_json::from_str::<AgentResponse>(&cached.value) {
                        return Ok(AgentResponse {
                            cache_hit: true,
                            ..response
                        });
                    }
                }

                // Build request.
                let system_prompt = context_manager
                    .build_system_prompt(&session_id)
                    .await
                    .unwrap_or_default();

                let agent_request = AgentRequest {
                    request_id: subtask.id.clone(),
                    system_prompt,
                    user_prompt: subtask.prompt.clone(),
                    context: String::new(),
                    allowed_tools: Vec::new(),
                    max_response_tokens: subtask.max_tokens,
                };

                // Execute with retries.
                let mut last_error = None;
                for attempt in 0..=retry_policy.max_retries {
                    if attempt > 0 {
                        let base_ms = retry_policy.base_delay.as_millis() as f64;
                        let delay_ms =
                            base_ms * retry_policy.backoff_multiplier.powi(attempt as i32 - 1);
                        let capped =
                            Duration::from_millis(delay_ms as u64).min(retry_policy.max_delay);
                        tokio::time::sleep(capped).await;
                    }

                    let task_timeout = subtask.timeout.unwrap_or(timeout_policy.default_timeout);

                    let result = timeout(task_timeout, agent.execute(agent_request.clone())).await;

                    match result {
                        Ok(Ok(response)) => {
                            circuit_breaker.record_success(&agent_id_str).await;

                            // Cache the response.
                            if let Ok(serialized) = serde_json::to_string(&response) {
                                let entry = CacheEntry {
                                    value: serialized,
                                    created_at: chrono::Utc::now().timestamp(),
                                    hit_count: 0,
                                    byte_size: response.content.len(),
                                };
                                let _ = cache.put(cache_key, entry).await;
                            }

                            return Ok(response);
                        }
                        Ok(Err(e)) => {
                            circuit_breaker.record_failure(&agent_id_str).await;
                            last_error = Some(e);
                        }
                        Err(_) => {
                            circuit_breaker.record_failure(&agent_id_str).await;
                            last_error = Some(OrchestratorError::Timeout {
                                duration_ms: task_timeout.as_millis() as u64,
                                context: format!("agent {agent_id_str}"),
                            });
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| {
                    OrchestratorError::agent(&agent_id_str, "all retries exhausted")
                }))
            });

            handles.push((subtask_for_handle, handle));
        }

        // 6. Collect results.
        let mut responses = Vec::new();
        for (subtask, handle) in handles {
            match handle.await {
                Ok(Ok(response)) => {
                    responses.push(response);
                }
                Ok(Err(e)) => {
                    error!(subtask_id = %subtask.id, error = %e, "subtask failed");
                    // Create an error response for traceability.
                    responses.push(AgentResponse {
                        request_id: subtask.id.clone(),
                        agent_id: subtask
                            .assigned_agent
                            .unwrap_or_else(|| "unknown".to_string()),
                        content: format!("[ERROR] {e}"),
                        structured_data: None,
                        estimated_tokens: 0,
                        duration: Duration::ZERO,
                        cache_hit: false,
                        warnings: vec![e.to_string()],
                    });
                }
                Err(join_err) => {
                    error!(subtask_id = %subtask.id, "subtask panicked: {join_err}");
                    responses.push(AgentResponse {
                        request_id: subtask.id.clone(),
                        agent_id: "unknown".to_string(),
                        content: format!("[PANIC] task join error: {join_err}"),
                        structured_data: None,
                        estimated_tokens: 0,
                        duration: Duration::ZERO,
                        cache_hit: false,
                        warnings: vec![join_err.to_string()],
                    });
                }
            }
        }

        // 7. Aggregate.
        let aggregate = self.aggregator.aggregate(&request.id, responses).await?;

        // 8. Add assistant response to context.
        let assistant_msg = crate::domain::context::Message {
            role: crate::domain::context::MessageRole::Assistant,
            content: aggregate.content.clone(),
            token_estimate: self.token_counter.count_tokens(&aggregate.content),
            timestamp: chrono::Utc::now().timestamp(),
        };
        self.context_manager
            .add_message(&request.session_id, assistant_msg)
            .await?;

        // 9. Trim context if needed.
        self.context_manager
            .trim_context(&request.session_id)
            .await?;

        info!(
            total_tokens = aggregate.total_tokens,
            cache_hits = aggregate.cache_stats.hits,
            cache_misses = aggregate.cache_stats.misses,
            "request processing complete"
        );

        Ok(aggregate)
    }

    async fn active_agents(&self) -> Result<Vec<String>> {
        let agents = self.router.all_agent_info().await;
        Ok(agents
            .into_iter()
            .filter(|a| a.enabled)
            .map(|a| a.id)
            .collect())
    }

    async fn health_check(&self) -> Result<Vec<(String, bool)>> {
        let agents = self.router.all_agent_info().await;
        let mut results = Vec::new();
        for info in &agents {
            if let Some(agent) = self.router.get_agent(&info.id).await {
                let healthy = agent.health_check().await.is_ok();
                results.push((info.id.clone(), healthy));
            } else {
                results.push((info.id.clone(), false));
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::aggregator::ConcatAggregator;
    use crate::application::circuit_breaker::CircuitBreakerConfig;
    use crate::application::context_service::InMemoryContextManager;
    use crate::application::planner::DeterministicPlanner;
    use crate::domain::agent::*;
    use crate::domain::cache::*;
    use crate::domain::context::ContextConfig;
    use crate::domain::error::Result as OrcResult;
    use crate::domain::policy::*;
    use crate::domain::token::SimpleTokenCounter;
    use std::collections::HashSet;

    // --- Fakes ---

    struct FakeAgent {
        info: AgentInfo,
        response_content: String,
    }

    impl FakeAgent {
        fn new(id: &str, caps: &[&str], content: &str) -> Self {
            Self {
                info: AgentInfo {
                    id: id.to_string(),
                    display_name: id.to_string(),
                    capabilities: caps
                        .iter()
                        .map(|c| AgentCapability::new(*c))
                        .collect::<HashSet<_>>(),
                    max_concurrency: 2,
                    default_timeout: Duration::from_secs(60),
                    enabled: true,
                    priority: 1,
                },
                response_content: content.to_string(),
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

    struct FakeCache;

    #[async_trait::async_trait]
    impl Cache for FakeCache {
        async fn get(&self, _key: &CacheKey) -> OrcResult<Option<CacheEntry>> {
            Ok(None)
        }
        async fn put(&self, _key: CacheKey, _entry: CacheEntry) -> OrcResult<()> {
            Ok(())
        }
        async fn remove(&self, _key: &CacheKey) -> OrcResult<()> {
            Ok(())
        }
        async fn clear(&self) -> OrcResult<()> {
            Ok(())
        }
        async fn len(&self) -> OrcResult<usize> {
            Ok(0)
        }
        async fn byte_size(&self) -> OrcResult<usize> {
            Ok(0)
        }
    }

    struct FakePolicyGuard;

    impl PolicyGuard for FakePolicyGuard {
        fn check_user_input(&self, _input: &str) -> OrcResult<PolicyCheckResult> {
            Ok(PolicyCheckResult::pass())
        }
        fn check_tool_request(
            &self,
            _command: &str,
            _args: &[String],
        ) -> OrcResult<PolicyCheckResult> {
            Ok(PolicyCheckResult::pass())
        }
        fn check_agent_output(&self, _output: &str) -> OrcResult<PolicyCheckResult> {
            Ok(PolicyCheckResult::pass())
        }
        fn redact(&self, text: &str) -> String {
            text.to_string()
        }
    }

    async fn build_test_orchestrator(
        agent_content: &str,
    ) -> (OrchestratorService, Arc<CapabilityRouter>) {
        let token_counter: Arc<dyn TokenCounter> = Arc::new(SimpleTokenCounter);
        let planner: Arc<dyn Planner> = Arc::new(DeterministicPlanner::new(
            token_counter.clone(),
            "code-generation".to_string(),
        ));
        let router = Arc::new(CapabilityRouter::new());
        let aggregator: Arc<dyn Aggregator> = Arc::new(ConcatAggregator::default());
        let context_manager: Arc<dyn ContextManager> = Arc::new(InMemoryContextManager::new(
            ContextConfig::default(),
            token_counter.clone(),
        ));
        let cache: Arc<dyn Cache> = Arc::new(FakeCache);
        let policy_guard: Arc<dyn PolicyGuard> = Arc::new(FakePolicyGuard);

        let agent = Arc::new(FakeAgent::new(
            "test-agent",
            &["code-generation"],
            agent_content,
        ));
        router.register(agent).await;

        let service = OrchestratorServiceBuilder::new()
            .planner(planner)
            .router(router.clone())
            .aggregator(aggregator)
            .context_manager(context_manager)
            .cache(cache)
            .token_counter(token_counter)
            .policy_guard(policy_guard)
            .circuit_breaker(CircuitBreaker::new(CircuitBreakerConfig::default()))
            .max_concurrency(4)
            .build()
            .expect("builder should succeed");

        (service, router)
    }

    #[tokio::test]
    async fn test_orchestrator_end_to_end() {
        let (service, _router) = build_test_orchestrator("Hello from agent!").await;

        let request = UserRequest {
            id: "req-1".to_string(),
            session_id: "session-1".to_string(),
            message: "Write hello world".to_string(),
            preferred_capability: None,
            max_tokens: Some(10_000),
        };

        let result = service.process(request).await.expect("should succeed");
        assert_eq!(result.content, "Hello from agent!");
        assert_eq!(result.request_id, "req-1");
    }

    #[tokio::test]
    async fn test_orchestrator_active_agents() {
        let (service, _router) = build_test_orchestrator("test").await;
        let agents = service.active_agents().await.expect("should succeed");
        assert_eq!(agents, vec!["test-agent"]);
    }

    #[tokio::test]
    async fn test_orchestrator_health_check() {
        let (service, _router) = build_test_orchestrator("test").await;
        let health = service.health_check().await.expect("should succeed");
        assert_eq!(health.len(), 1);
        assert_eq!(health[0], ("test-agent".to_string(), true));
    }

    #[tokio::test]
    async fn test_builder_missing_components() {
        let result = OrchestratorServiceBuilder::new().build();
        assert!(result.is_err());
    }
}
