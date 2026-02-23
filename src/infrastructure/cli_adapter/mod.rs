//! CLI agent adapter — fully configurable.

mod agent;
mod parse;
#[cfg(test)]
mod tests;

pub use agent::CliAgent;

use serde::{Deserialize, Serialize};

/// Configuration for a CLI-based AI agent (all user-configurable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliAgentConfig {
    pub id: String,
    pub display_name: String,
    pub binary: String,
    pub base_args: Vec<String>,
    pub input_mode: InputMode,
    pub prompt_placeholder: Option<String>,
    pub output_format: OutputFormat,
    pub json_content_path: Option<String>,
    pub timeout_seconds: u64,
    pub max_concurrency: usize,
    pub priority: u32,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub env_vars: Vec<EnvVarConfig>,
}

/// How the prompt is passed to the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMode {
    Stdin,
    Arg,
    File,
}

/// Expected output format from the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Text,
    Json,
    Auto,
}

/// An environment variable to set for the CLI process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarConfig {
    pub name: String,
    pub value: String,
}
