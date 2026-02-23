//! Configuration file loading (TOML / YAML).

use std::path::Path;

use tracing::info;

use crate::domain::error::{OrchestratorError, Result};

use super::OrchestratorConfig;

impl OrchestratorConfig {
    /// Load from a file (TOML or YAML, detected by extension).
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            OrchestratorError::InvalidConfig(format!("read '{}': {e}", path.display()))
        })?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("toml");
        let config: Self = match ext {
            "yaml" | "yml" => serde_yaml::from_str(&content)
                .map_err(|e| OrchestratorError::InvalidConfig(format!("YAML: {e}")))?,
            "toml" => toml::from_str(&content)
                .map_err(|e| OrchestratorError::InvalidConfig(format!("TOML: {e}")))?,
            other => {
                return Err(OrchestratorError::InvalidConfig(format!(
                    "unsupported: '{other}'"
                )))
            }
        };
        info!(path = %path.display(), "configuration loaded");
        Ok(config)
    }

    /// Load from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self> {
        toml::from_str(s).map_err(|e| OrchestratorError::InvalidConfig(format!("TOML: {e}")))
    }

    /// Load from a YAML string.
    pub fn from_yaml(s: &str) -> Result<Self> {
        serde_yaml::from_str(s).map_err(|e| OrchestratorError::InvalidConfig(format!("YAML: {e}")))
    }
}
