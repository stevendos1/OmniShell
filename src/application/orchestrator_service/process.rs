//! Orchestrator trait implementation — the main request pipeline.

use std::time::Duration;

use tracing::{error, info, instrument};

use crate::domain::agent::AgentResponse;
use crate::domain::context::{Message, MessageRole};
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::orchestrator::*;
use crate::domain::task::SubTask;

use super::OrchestratorService;

#[async_trait::async_trait]
impl Orchestrator for OrchestratorService {
    #[instrument(skip(self), fields(request_id = %request.id, session_id = %request.session_id))]
    async fn process(&self, request: UserRequest) -> Result<AggregateResponse> {
        info!("processing user request");
        let pr = self.policy_guard.check_user_input(&request.message)?;
        if !pr.allowed {
            return Err(OrchestratorError::PolicyViolation(pr.reason));
        }
        let cur = self.context_manager.estimate_tokens(&request.session_id).await?;
        if let Some(max) = request.max_tokens {
            if cur > max {
                return Err(OrchestratorError::TokenBudgetExceeded { used: cur, limit: max });
            }
        }
        let msg = Message { role: MessageRole::User, content: request.message.clone(), token_estimate: self.token_counter.count_tokens(&request.message), timestamp: chrono::Utc::now().timestamp() };
        self.context_manager.add_message(&request.session_id, msg).await?;
        let plan = self.planner.plan(&request).await?;
        info!(subtask_count = plan.subtasks.len(), "plan generated");

        let mut handles = Vec::new();
        for subtask in &plan.subtasks {
            let agent_id = self.router.route(subtask.required_capability.name()).await?;
            let agent = self.router.get_agent(&agent_id).await.ok_or_else(|| OrchestratorError::agent(&agent_id, "registered but not found"))?;
            let (sem, cb, cache) = (self.concurrency_semaphore.clone(), self.circuit_breaker.clone(), self.cache.clone());
            let (ctx_mgr, rp, tp) = (self.context_manager.clone(), self.retry_policy.clone(), self.timeout_policy.clone());
            let (cfg_ver, sid, st) = (self.config_version.clone(), request.session_id.clone(), subtask.clone());
            handles.push((
                subtask.clone(),
                tokio::spawn(async move {
                    let _permit = sem.acquire().await.map_err(|_| OrchestratorError::NotImplemented("semaphore closed".into()))?;
                    super::worker::execute_spawned(agent, st, sid, cb, cache, ctx_mgr, rp, tp, cfg_ver).await
                }),
            ));
        }

        let mut responses = Vec::new();
        for (st, handle) in handles {
            match handle.await {
                Ok(Ok(r)) => responses.push(r),
                Ok(Err(e)) => {
                    error!(subtask_id = %st.id, error = %e, "subtask failed");
                    responses.push(err_resp(&st, format!("[ERROR] {e}")));
                }
                Err(je) => {
                    error!(subtask_id = %st.id, "subtask panicked: {je}");
                    responses.push(err_resp(&st, format!("[PANIC] {je}")));
                }
            }
        }

        let agg = self.aggregator.aggregate(&request.id, responses).await?;
        let amsg = Message { role: MessageRole::Assistant, content: agg.content.clone(), token_estimate: self.token_counter.count_tokens(&agg.content), timestamp: chrono::Utc::now().timestamp() };
        self.context_manager.add_message(&request.session_id, amsg).await?;
        self.context_manager.trim_context(&request.session_id).await?;
        info!(total_tokens = agg.total_tokens, cache_hits = agg.cache_stats.hits, "request complete");
        Ok(agg)
    }

    async fn active_agents(&self) -> Result<Vec<String>> {
        Ok(self.router.all_agent_info().await.into_iter().filter(|a| a.enabled).map(|a| a.id).collect())
    }

    async fn health_check(&self) -> Result<Vec<(String, bool)>> {
        let agents = self.router.all_agent_info().await;
        let mut results = Vec::new();
        for info in &agents {
            let ok = if let Some(a) = self.router.get_agent(&info.id).await { a.health_check().await.is_ok() } else { false };
            results.push((info.id.clone(), ok));
        }
        Ok(results)
    }
}

fn err_resp(st: &SubTask, msg: String) -> AgentResponse {
    AgentResponse {
        request_id: st.id.clone(),
        agent_id: st.assigned_agent.clone().unwrap_or_else(|| "unknown".into()),
        content: msg.clone(),
        structured_data: None,
        estimated_tokens: 0,
        duration: Duration::ZERO,
        cache_hit: false,
        warnings: vec![msg],
    }
}
