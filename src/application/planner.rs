//! # Deterministic Planner
//!
//! A rule-based planner that splits user requests into subtasks.
//! This is the default planner; it can be swapped for an AI-based one
//! by implementing the `Planner` trait.

use uuid::Uuid;

use crate::domain::agent::AgentCapability;
use crate::domain::error::Result;
use crate::domain::orchestrator::{Planner, UserRequest};
use crate::domain::task::{ExecutionPlan, SubTask, TaskStatus};
use crate::domain::token::TokenCounter;

/// A deterministic planner that creates a single subtask from the user request.
///
/// This is the simplest possible planner: it maps the user message
/// directly to one subtask. More sophisticated planners (rule-based
/// or AI-based) can implement `Planner` to produce multi-step plans.
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
    /// Create a new deterministic planner.
    ///
    /// # Arguments
    /// - `token_counter`: Used to estimate subtask token costs.
    /// - `default_capability`: Capability to assign when the user
    ///   does not specify one.
    pub fn new(
        token_counter: std::sync::Arc<dyn TokenCounter>,
        default_capability: String,
    ) -> Self {
        Self {
            token_counter,
            default_capability,
        }
    }
}

#[async_trait::async_trait]
impl Planner for DeterministicPlanner {
    async fn plan(&self, request: &UserRequest) -> Result<ExecutionPlan> {
        let capability = request
            .preferred_capability
            .as_deref()
            .unwrap_or(&self.default_capability);

        let estimated_tokens = self.token_counter.count_tokens(&request.message);

        let subtask = SubTask {
            id: Uuid::new_v4().to_string(),
            parent_request_id: request.id.clone(),
            description: format!("Process user request: {}", truncate(&request.message, 80)),
            required_capability: AgentCapability::new(capability),
            prompt: request.message.clone(),
            max_tokens: request.max_tokens,
            timeout: None,
            status: TaskStatus::Pending,
            depends_on: Vec::new(),
            retry_count: 0,
            assigned_agent: None,
        };

        Ok(ExecutionPlan {
            request_id: request.id.clone(),
            subtasks: vec![subtask],
            estimated_total_tokens: estimated_tokens,
        })
    }
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max_len)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::token::SimpleTokenCounter;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_deterministic_planner_produces_single_subtask() {
        let planner =
            DeterministicPlanner::new(Arc::new(SimpleTokenCounter), "code-generation".to_string());

        let request = UserRequest {
            id: "req-1".to_string(),
            session_id: "session-1".to_string(),
            message: "Write a hello world in Rust".to_string(),
            preferred_capability: None,
            max_tokens: Some(4000),
        };

        let plan = planner
            .plan(&request)
            .await
            .expect("planning should succeed");
        assert_eq!(plan.subtasks.len(), 1);
        assert_eq!(plan.request_id, "req-1");
        assert_eq!(
            plan.subtasks[0].required_capability,
            AgentCapability::new("code-generation")
        );
        assert_eq!(plan.subtasks[0].status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn test_planner_uses_preferred_capability() {
        let planner =
            DeterministicPlanner::new(Arc::new(SimpleTokenCounter), "default-cap".to_string());

        let request = UserRequest {
            id: "req-2".to_string(),
            session_id: "session-1".to_string(),
            message: "Summarize this doc".to_string(),
            preferred_capability: Some("summarization".to_string()),
            max_tokens: None,
        };

        let plan = planner
            .plan(&request)
            .await
            .expect("planning should succeed");
        assert_eq!(
            plan.subtasks[0].required_capability,
            AgentCapability::new("summarization")
        );
    }
}
