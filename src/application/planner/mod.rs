//! Deterministic planner.

#[cfg(test)]
mod tests;

use uuid::Uuid;

use crate::domain::agent::AgentCapability;
use crate::domain::error::Result;
use crate::domain::orchestrator::{Planner, UserRequest};
use crate::domain::task::{ExecutionPlan, SubTask, TaskStatus};
use crate::domain::token::TokenCounter;

/// A deterministic planner that creates a single subtask.
///
/// # Example
/// ```
/// use omnishell_orchestrator::application::planner::DeterministicPlanner;
/// use omnishell_orchestrator::domain::token::SimpleTokenCounter;
/// use std::sync::Arc;
///
/// let planner = DeterministicPlanner::new(
///     Arc::new(SimpleTokenCounter),
///     "code-generation".to_string(),
/// );
/// ```
pub struct DeterministicPlanner {
    token_counter: std::sync::Arc<dyn TokenCounter>,
    default_capability: String,
}

impl DeterministicPlanner {
    pub fn new(token_counter: std::sync::Arc<dyn TokenCounter>, default_capability: String) -> Self {
        Self { token_counter, default_capability }
    }
}

#[async_trait::async_trait]
impl Planner for DeterministicPlanner {
    async fn plan(&self, request: &UserRequest) -> Result<ExecutionPlan> {
        let capability = request.preferred_capability.as_deref().unwrap_or(&self.default_capability);
        let estimated_tokens = self.token_counter.count_tokens(&request.message);
        let subtask = SubTask {
            id: Uuid::new_v4().to_string(),
            parent_request_id: request.id.clone(),
            description: format!("Process: {}", truncate(&request.message, 80)),
            required_capability: AgentCapability::new(capability),
            prompt: request.message.clone(),
            max_tokens: request.max_tokens,
            timeout: None,
            status: TaskStatus::Pending,
            depends_on: Vec::new(),
            retry_count: 0,
            assigned_agent: None,
        };
        Ok(ExecutionPlan { request_id: request.id.clone(), subtasks: vec![subtask], estimated_total_tokens: estimated_tokens })
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = s.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}
