//! # Capability-based Router
//!
//! Routes subtasks to agents based on declared capabilities and priority.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::debug;

use crate::domain::agent::{AgentInfo, AiAgent};
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::orchestrator::Router;

/// A capability-based router that selects agents by matching
/// requested capabilities to agent declarations.
///
/// When multiple agents match, the one with the highest `priority`
/// (and that is enabled) is selected.
///
/// # Thread safety
/// The agent registry is behind an `RwLock` so agents can be
/// added/removed at runtime.
///
/// # Example
/// ```
/// use omnishell_orchestrator::application::router::CapabilityRouter;
///
/// let router = CapabilityRouter::new();
/// ```
pub struct CapabilityRouter {
    agents: Arc<RwLock<HashMap<String, Arc<dyn AiAgent>>>>,
}

impl CapabilityRouter {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an agent with the router.
    pub async fn register(&self, agent: Arc<dyn AiAgent>) {
        let id = agent.info().id.clone();
        debug!(agent_id = %id, "registering agent with router");
        self.agents.write().await.insert(id, agent);
    }

    /// Unregister an agent by ID.
    pub async fn unregister(&self, agent_id: &str) {
        self.agents.write().await.remove(agent_id);
    }

    /// Get info for all registered agents.
    pub async fn all_agent_info(&self) -> Vec<AgentInfo> {
        let agents = self.agents.read().await;
        agents.values().map(|a| a.info().clone()).collect()
    }

    /// Get a reference to an agent by ID.
    pub async fn get_agent(&self, agent_id: &str) -> Option<Arc<dyn AiAgent>> {
        let agents = self.agents.read().await;
        agents.get(agent_id).cloned()
    }
}

impl Default for CapabilityRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Router for CapabilityRouter {
    async fn route(&self, capability: &str) -> Result<String> {
        let agents = self.agents.read().await;
        let mut best: Option<(&str, u32)> = None;

        for agent in agents.values() {
            let info = agent.info();
            if info.enabled && info.has_capability(capability) {
                match best {
                    Some((_, best_prio)) if info.priority > best_prio => {
                        best = Some((&info.id, info.priority));
                    }
                    None => {
                        best = Some((&info.id, info.priority));
                    }
                    _ => {}
                }
            }
        }

        match best {
            Some((id, _)) => Ok(id.to_string()),
            None => Err(OrchestratorError::InvalidConfig(format!(
                "no enabled agent found for capability '{capability}'"
            ))),
        }
    }

    async fn all_matching(&self, capability: &str) -> Result<Vec<String>> {
        let agents = self.agents.read().await;
        let mut matching: Vec<(String, u32)> = agents
            .values()
            .filter_map(|a| {
                let info = a.info();
                if info.enabled && info.has_capability(capability) {
                    Some((info.id.clone(), info.priority))
                } else {
                    None
                }
            })
            .collect();

        // Sort by priority descending.
        matching.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(matching.into_iter().map(|(id, _)| id).collect())
    }
}

#[cfg(test)]
mod tests {
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
                    id: id.to_string(),
                    display_name: id.to_string(),
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
        async fn execute(&self, _req: AgentRequest) -> OrcResult<AgentResponse> {
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
            .register(Arc::new(FakeAgent::new("low-pri", &["code-gen"], 1, true)))
            .await;
        router
            .register(Arc::new(FakeAgent::new(
                "high-pri",
                &["code-gen"],
                10,
                true,
            )))
            .await;

        let result = router.route("code-gen").await.expect("should find agent");
        assert_eq!(result, "high-pri");
    }

    #[tokio::test]
    async fn test_route_skips_disabled() {
        let router = CapabilityRouter::new();
        router
            .register(Arc::new(FakeAgent::new(
                "disabled",
                &["code-gen"],
                100,
                false,
            )))
            .await;
        router
            .register(Arc::new(FakeAgent::new("enabled", &["code-gen"], 1, true)))
            .await;

        let result = router.route("code-gen").await.expect("should find agent");
        assert_eq!(result, "enabled");
    }

    #[tokio::test]
    async fn test_route_no_match() {
        let router = CapabilityRouter::new();
        let result = router.route("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_all_matching_sorted_by_priority() {
        let router = CapabilityRouter::new();
        router
            .register(Arc::new(FakeAgent::new("a", &["code-gen"], 5, true)))
            .await;
        router
            .register(Arc::new(FakeAgent::new("b", &["code-gen"], 10, true)))
            .await;
        router
            .register(Arc::new(FakeAgent::new("c", &["code-gen"], 1, true)))
            .await;

        let result = router
            .all_matching("code-gen")
            .await
            .expect("should find agents");
        assert_eq!(result, vec!["b", "a", "c"]);
    }
}
