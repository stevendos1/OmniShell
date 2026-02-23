//! Planner unit tests.

use super::*;
use crate::domain::token::SimpleTokenCounter;
use std::sync::Arc;

#[tokio::test]
async fn test_single_subtask() {
    let planner = DeterministicPlanner::new(Arc::new(SimpleTokenCounter), "code-generation".into());
    let request = UserRequest { id: "req-1".into(), session_id: "s1".into(), message: "Write a hello world in Rust".into(), preferred_capability: None, max_tokens: Some(4000) };
    let plan = planner.plan(&request).await.expect("should plan");
    assert_eq!(plan.subtasks.len(), 1);
    assert_eq!(plan.request_id, "req-1");
    assert_eq!(plan.subtasks[0].required_capability, AgentCapability::new("code-generation"));
    assert_eq!(plan.subtasks[0].status, TaskStatus::Pending);
}

#[tokio::test]
async fn test_preferred_capability() {
    let planner = DeterministicPlanner::new(Arc::new(SimpleTokenCounter), "default".into());
    let request = UserRequest { id: "req-2".into(), session_id: "s1".into(), message: "Summarize".into(), preferred_capability: Some("summarization".into()), max_tokens: None };
    let plan = planner.plan(&request).await.expect("should plan");
    assert_eq!(plan.subtasks[0].required_capability, AgentCapability::new("summarization"));
}
