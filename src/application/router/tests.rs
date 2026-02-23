//! Router unit tests.

use super::*;
use crate::domain::agent::*;
use crate::domain::error::Result as OrcResult;
use std::collections::HashSet;
use std::time::Duration;

struct FakeAgent {
    info: AgentInfo,
}
impl FakeAgent {
    fn new(id: &str, caps: &[&str], priority: u32, enabled: bool) -> Self {
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
                enabled,
                priority,
            },
        }
    }
}

#[async_trait::async_trait]
impl AiAgent for FakeAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }
    async fn execute(&self, _: AgentRequest) -> OrcResult<AgentResponse> {
        Err(crate::domain::error::OrchestratorError::NotImplemented(
            "fake".into(),
        ))
    }
    async fn health_check(&self) -> OrcResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_route_selects_highest_priority() {
    let router = CapabilityRouter::new();
    router
        .register(Arc::new(FakeAgent::new("low", &["code-gen"], 1, true)))
        .await;
    router
        .register(Arc::new(FakeAgent::new("high", &["code-gen"], 10, true)))
        .await;
    assert_eq!(router.route("code-gen").await.unwrap(), "high");
}

#[tokio::test]
async fn test_route_skips_disabled() {
    let router = CapabilityRouter::new();
    router
        .register(Arc::new(FakeAgent::new("off", &["code-gen"], 100, false)))
        .await;
    router
        .register(Arc::new(FakeAgent::new("on", &["code-gen"], 1, true)))
        .await;
    assert_eq!(router.route("code-gen").await.unwrap(), "on");
}

#[tokio::test]
async fn test_route_no_match() {
    let router = CapabilityRouter::new();
    assert!(router.route("nonexistent").await.is_err());
}

#[tokio::test]
async fn test_all_matching_sorted() {
    let router = CapabilityRouter::new();
    router
        .register(Arc::new(FakeAgent::new("a", &["cg"], 5, true)))
        .await;
    router
        .register(Arc::new(FakeAgent::new("b", &["cg"], 10, true)))
        .await;
    router
        .register(Arc::new(FakeAgent::new("c", &["cg"], 1, true)))
        .await;
    assert_eq!(
        router.all_matching("cg").await.unwrap(),
        vec!["b", "a", "c"]
    );
}
