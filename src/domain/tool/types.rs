//! Tool executor configuration, request, and response types.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configuration for the tool executor security sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutorConfig {
    pub enabled: bool,
    pub allowed_commands: HashSet<String>,
    pub denied_commands: HashSet<String>,
    pub working_dir: PathBuf,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub dry_run: bool,
}

impl Default for ToolExecutorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_commands: HashSet::new(),
            denied_commands: HashSet::new(),
            working_dir: PathBuf::from("."),
            timeout: Duration::from_secs(30),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 256 * 1024,
            dry_run: false,
        }
    }
}

/// A request to execute a local tool/command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub timeout: Option<Duration>,
}

/// The result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub truncated: bool,
    pub dry_run: bool,
}
