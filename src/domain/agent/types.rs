//! Agent capability, metadata, request, and response types.

use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// A declared capability that an agent can fulfill.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentCapability {
    name: String,
}

impl AgentCapability {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Static metadata describing an agent worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub display_name: String,
    pub capabilities: HashSet<AgentCapability>,
    pub max_concurrency: usize,
    pub default_timeout: Duration,
    pub enabled: bool,
    pub priority: u32,
}

impl AgentInfo {
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.contains(&AgentCapability::new(cap))
    }
}

/// A request sent to an AI agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub request_id: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub context: String,
    pub allowed_tools: Vec<String>,
    pub max_response_tokens: Option<u64>,
}

/// The response received from an AI agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub request_id: String,
    pub agent_id: String,
    pub content: String,
    pub structured_data: Option<serde_json::Value>,
    pub estimated_tokens: u64,
    pub duration: Duration,
    pub cache_hit: bool,
    pub warnings: Vec<String>,
}
