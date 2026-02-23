//! Configuration loader.

mod loader;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::domain::cache::CacheConfig;
use crate::domain::context::ContextConfig;
use crate::domain::policy::PolicyConfig;
use crate::domain::task::{RetryPolicy, TimeoutPolicy};
use crate::domain::token::TokenBudgetConfig;
use crate::domain::tool::ToolExecutorConfig;
use crate::infrastructure::cli_adapter::CliAgentConfig;

/// Top-level orchestrator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    #[serde(default = "default_config_version")]
    pub config_version: String,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default)]
    pub agents: Vec<CliAgentConfig>,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub token_budget: TokenBudgetConfig,
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    #[serde(default)]
    pub timeout_policy: TimeoutPolicy,
    #[serde(default)]
    pub tool_executor: ToolExecutorConfig,
    #[serde(default)]
    pub policy: PolicyConfig,
}

fn default_config_version() -> String {
    "v1".into()
}
fn default_max_concurrency() -> usize {
    10
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            config_version: default_config_version(),
            max_concurrency: default_max_concurrency(),
            agents: Vec::new(),
            cache: CacheConfig::default(),
            context: ContextConfig::default(),
            token_budget: TokenBudgetConfig::default(),
            retry_policy: RetryPolicy::default(),
            timeout_policy: TimeoutPolicy::default(),
            tool_executor: ToolExecutorConfig::default(),
            policy: PolicyConfig::default(),
        }
    }
}
