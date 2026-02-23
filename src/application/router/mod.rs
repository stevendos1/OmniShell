//! Capability-based router.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::debug;

use crate::domain::agent::{AgentInfo, AiAgent};
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::orchestrator::Router;

/// Routes subtasks to agents based on declared capabilities and priority.
pub struct CapabilityRouter {
    agents: Arc<RwLock<HashMap<String, Arc<dyn AiAgent>>>>,
}

impl CapabilityRouter {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn register(&self, agent: Arc<dyn AiAgent>) {
        let id = agent.info().id.clone();
        debug!(agent_id = %id, "registering agent");
        self.agents.write().await.insert(id, agent);
    }
    pub async fn unregister(&self, agent_id: &str) {
        self.agents.write().await.remove(agent_id);
    }
    pub async fn all_agent_info(&self) -> Vec<AgentInfo> {
        self.agents
            .read()
            .await
            .values()
            .map(|a| a.info().clone())
            .collect()
    }
    pub async fn get_agent(&self, agent_id: &str) -> Option<Arc<dyn AiAgent>> {
        self.agents.read().await.get(agent_id).cloned()
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
                    Some((_, bp)) if info.priority > bp => best = Some((&info.id, info.priority)),
                    None => best = Some((&info.id, info.priority)),
                    _ => {}
                }
            }
        }
        best.map(|(id, _)| id.to_string())
            .ok_or_else(|| OrchestratorError::InvalidConfig(format!("no agent for '{capability}'")))
    }

    async fn all_matching(&self, capability: &str) -> Result<Vec<String>> {
        let agents = self.agents.read().await;
        let mut m: Vec<(String, u32)> = agents
            .values()
            .filter_map(|a| {
                let i = a.info();
                if i.enabled && i.has_capability(capability) {
                    Some((i.id.clone(), i.priority))
                } else {
                    None
                }
            })
            .collect();
        m.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(m.into_iter().map(|(id, _)| id).collect())
    }
}
