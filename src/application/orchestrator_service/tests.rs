//! Orchestrator service unit tests.

use super::test_fakes::*;
use crate::domain::orchestrator::{Orchestrator, UserRequest};

#[tokio::test]
async fn test_orchestrator_end_to_end() {
    let (service, _) = build_test_orchestrator("Hello from agent!").await;
    let request = UserRequest { id: "req-1".into(), session_id: "session-1".into(), message: "Write hello world".into(), preferred_capability: None, max_tokens: Some(10_000) };
    let result = service.process(request).await.expect("should succeed");
    assert_eq!(result.content, "Hello from agent!");
    assert_eq!(result.request_id, "req-1");
}

#[tokio::test]
async fn test_orchestrator_active_agents() {
    let (service, _) = build_test_orchestrator("test").await;
    let agents = service.active_agents().await.expect("should succeed");
    assert_eq!(agents, vec!["test-agent"]);
}

#[tokio::test]
async fn test_orchestrator_health_check() {
    let (service, _) = build_test_orchestrator("test").await;
    let health = service.health_check().await.expect("should succeed");
    assert_eq!(health.len(), 1);
    assert_eq!(health[0], ("test-agent".to_string(), true));
}

#[tokio::test]
async fn test_builder_missing_components() {
    use super::OrchestratorServiceBuilder;
    let result = OrchestratorServiceBuilder::new().build();
    assert!(result.is_err());
}
