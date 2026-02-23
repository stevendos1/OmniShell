//! Spawned-task worker: executes a subtask with retries and caching.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use crate::application::circuit_breaker::CircuitBreaker;
use crate::domain::agent::*;
use crate::domain::cache::*;
use crate::domain::context::ContextManager;
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::task::{RetryPolicy, SubTask, TimeoutPolicy};

pub(crate) struct WorkerParams {
    pub cb: CircuitBreaker,
    pub cache: Arc<dyn Cache>,
    pub ctx_mgr: Arc<dyn ContextManager>,
    pub rp: RetryPolicy,
    pub tp: TimeoutPolicy,
    pub cfg_ver: String,
}

pub(crate) async fn execute_spawned(
    agent: Arc<dyn AiAgent>,
    subtask: SubTask,
    session_id: String,
    params: WorkerParams,
) -> Result<AgentResponse> {
    let WorkerParams {
        cb,
        cache,
        ctx_mgr,
        rp,
        tp,
        cfg_ver,
    } = params;
    let aid = agent.info().id.clone();
    cb.check(&aid).await?;
    let ck = CacheKey::compute(&subtask.prompt, "", &aid, &cfg_ver);
    if let Ok(Some(cached)) = cache.get(&ck).await {
        if let Ok(r) = serde_json::from_str::<AgentResponse>(&cached.value) {
            return Ok(AgentResponse {
                cache_hit: true,
                ..r
            });
        }
    }
    let sp = ctx_mgr
        .build_system_prompt(&session_id)
        .await
        .unwrap_or_default();
    let ar = AgentRequest {
        request_id: subtask.id.clone(),
        system_prompt: sp,
        user_prompt: subtask.prompt.clone(),
        context: String::new(),
        allowed_tools: Vec::new(),
        max_response_tokens: subtask.max_tokens,
    };
    let mut last_err = None;
    for attempt in 0..=rp.max_retries {
        if attempt > 0 {
            let d = Duration::from_millis(
                (rp.base_delay.as_millis() as f64 * rp.backoff_multiplier.powi(attempt as i32 - 1))
                    as u64,
            )
            .min(rp.max_delay);
            tokio::time::sleep(d).await;
        }
        let t = subtask.timeout.unwrap_or(tp.default_timeout);
        match timeout(t, agent.execute(ar.clone())).await {
            Ok(Ok(resp)) => {
                cb.record_success(&aid).await;
                if let Ok(s) = serde_json::to_string(&resp) {
                    let entry = CacheEntry {
                        value: s,
                        created_at: chrono::Utc::now().timestamp(),
                        hit_count: 0,
                        byte_size: resp.content.len(),
                    };
                    let _ = cache.put(ck, entry).await;
                }
                return Ok(resp);
            }
            Ok(Err(e)) => {
                cb.record_failure(&aid).await;
                last_err = Some(e);
            }
            Err(_) => {
                cb.record_failure(&aid).await;
                last_err = Some(OrchestratorError::Timeout {
                    duration_ms: t.as_millis() as u64,
                    context: format!("agent {aid}"),
                });
            }
        }
    }
    Err(last_err.unwrap_or_else(|| OrchestratorError::agent(&aid, "all retries exhausted")))
}
