//! # Configuration Loader
//!
//! Loads orchestrator configuration from TOML or YAML files.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::domain::cache::CacheConfig;
use crate::domain::context::ContextConfig;
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::policy::PolicyConfig;
use crate::domain::task::{RetryPolicy, TimeoutPolicy};
use crate::domain::token::TokenBudgetConfig;
use crate::domain::tool::ToolExecutorConfig;
use crate::infrastructure::cli_adapter::CliAgentConfig;

/// Top-level orchestrator configuration.
///
/// Loaded from a TOML or YAML file. All fields have defaults so
/// the config file only needs to specify overrides.
///
/// # Example (TOML)
/// ```toml
/// config_version = "v1"
/// max_concurrency = 8
///
/// [[agents]]
/// id = "claude-cli"
/// binary = "claude"
/// # ... see CliAgentConfig
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Config version string, used in cache key computation.
    #[serde(default = "default_config_version")]
    pub config_version: String,

    /// Maximum global concurrency.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,

    /// Agent configurations.
    #[serde(default)]
    pub agents: Vec<CliAgentConfig>,

    /// Cache configuration.
    #[serde(default)]
    pub cache: CacheConfig,

    /// Context manager configuration.
    #[serde(default)]
    pub context: ContextConfig,

    /// Token budget configuration.
    #[serde(default)]
    pub token_budget: TokenBudgetConfig,

    /// Retry policy.
    #[serde(default)]
    pub retry_policy: RetryPolicy,

    /// Timeout policy.
    #[serde(default)]
    pub timeout_policy: TimeoutPolicy,

    /// Tool executor configuration.
    #[serde(default)]
    pub tool_executor: ToolExecutorConfig,

    /// Policy guard configuration.
    #[serde(default)]
    pub policy: PolicyConfig,
}

fn default_config_version() -> String {
    "v1".to_string()
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

impl OrchestratorConfig {
    /// Load configuration from a file (TOML or YAML, detected by extension).
    ///
    /// # Errors
    /// Returns `InvalidConfig` if the file cannot be read or parsed.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            OrchestratorError::InvalidConfig(format!(
                "failed to read config file '{}': {e}",
                path.display()
            ))
        })?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("toml");

        let config: Self = match ext {
            "yaml" | "yml" => serde_yaml::from_str(&content)
                .map_err(|e| OrchestratorError::InvalidConfig(format!("YAML parse error: {e}")))?,
            "toml" => toml::from_str(&content)
                .map_err(|e| OrchestratorError::InvalidConfig(format!("TOML parse error: {e}")))?,
            other => {
                return Err(OrchestratorError::InvalidConfig(format!(
                    "unsupported config format: '{other}' (use .toml or .yaml)"
                )))
            }
        };

        info!(path = %path.display(), "configuration loaded");
        Ok(config)
    }

    /// Load configuration from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str)
            .map_err(|e| OrchestratorError::InvalidConfig(format!("TOML parse error: {e}")))
    }

    /// Load configuration from a YAML string.
    pub fn from_yaml(yaml_str: &str) -> Result<Self> {
        serde_yaml::from_str(yaml_str)
            .map_err(|e| OrchestratorError::InvalidConfig(format!("YAML parse error: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_concurrency, 10);
        assert!(config.agents.is_empty());
        assert!(config.cache.enabled);
    }

    #[test]
    fn test_parse_minimal_toml() {
        let toml_str = r#"
            config_version = "v2"
            max_concurrency = 4
        "#;
        let config = OrchestratorConfig::from_toml(toml_str).expect("should parse");
        assert_eq!(config.config_version, "v2");
        assert_eq!(config.max_concurrency, 4);
    }

    #[test]
    fn test_parse_toml_with_agent() {
        let toml_str = r#"
            config_version = "v1"

            [[agents]]
            id = "test-agent"
            display_name = "Test"
            binary = "/usr/bin/echo"
            base_args = ["hello"]
            input_mode = "stdin"
            output_format = "text"
            timeout_seconds = 30
            max_concurrency = 2
            priority = 5
            capabilities = ["code-generation"]
            enabled = true
            env_vars = []
        "#;
        let config = OrchestratorConfig::from_toml(toml_str).expect("should parse");
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents[0].id, "test-agent");
    }

    #[test]
    fn test_parse_yaml() {
        let yaml_str = r#"
config_version: "v1"
max_concurrency: 6
agents: []
"#;
        let config = OrchestratorConfig::from_yaml(yaml_str).expect("should parse");
        assert_eq!(config.max_concurrency, 6);
    }

    #[test]
    fn test_invalid_toml() {
        let result = OrchestratorConfig::from_toml("{{invalid");
        assert!(result.is_err());
    }
}
