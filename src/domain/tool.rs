//! # Tool Execution Domain
//!
//! Defines the port for executing local tools (shell commands)
//! and the associated security configuration.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;

/// Configuration for the tool executor security sandbox.
///
/// Deny-by-default: only commands in `allowed_commands` can run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutorConfig {
    /// Whether tool execution is enabled at all.
    pub enabled: bool,
    /// Allowlist of command basenames (e.g. `["ls", "cat", "git"]`).
    pub allowed_commands: HashSet<String>,
    /// Denylist (overrides allowlist).
    pub denied_commands: HashSet<String>,
    /// Restricted working directory. Commands run inside this dir.
    pub working_dir: PathBuf,
    /// Maximum execution time per command.
    pub timeout: Duration,
    /// Maximum stdout size in bytes.
    pub max_stdout_bytes: usize,
    /// Maximum stderr size in bytes.
    pub max_stderr_bytes: usize,
    /// If true, commands are logged but not actually executed.
    pub dry_run: bool,
}

impl Default for ToolExecutorConfig {
    fn default() -> Self {
        Self {
            enabled: false, // deny by default
            allowed_commands: HashSet::new(),
            denied_commands: HashSet::new(),
            working_dir: PathBuf::from("."),
            timeout: Duration::from_secs(30),
            max_stdout_bytes: 1024 * 1024, // 1 MiB
            max_stderr_bytes: 256 * 1024,  // 256 KiB
            dry_run: false,
        }
    }
}

/// A request to execute a local tool/command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// The command basename (e.g. `"git"`).
    pub command: String,
    /// Arguments (each element is one argument; NO shell concatenation).
    pub args: Vec<String>,
    /// Optional working directory override.
    pub working_dir: Option<PathBuf>,
    /// Optional timeout override.
    pub timeout: Option<Duration>,
}

/// The result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// Exit code of the process.
    pub exit_code: i32,
    /// Captured stdout (truncated to max_stdout_bytes).
    pub stdout: String,
    /// Captured stderr (truncated to max_stderr_bytes).
    pub stderr: String,
    /// Wall-clock duration.
    pub duration: Duration,
    /// Whether the output was truncated.
    pub truncated: bool,
    /// Whether this was a dry-run (command not actually executed).
    pub dry_run: bool,
}

/// Port for executing local tools securely.
///
/// Implementations enforce the allowlist, denylist, timeouts,
/// output limits, and working directory restrictions.
///
/// # Security
/// - Deny by default: only `allowed_commands` can run.
/// - Arguments are passed as a `Vec<String>`, never concatenated into a shell string.
/// - Working directory is restricted.
/// - Output is size-limited.
///
/// # Errors
/// - `ToolDenied` if the command is not in the allowlist or is in the denylist.
/// - `ToolExecutionError` if the command fails.
/// - `Timeout` if the command exceeds the configured timeout.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool request.
    async fn execute(&self, request: ToolRequest) -> Result<ToolResponse>;

    /// Check whether a command would be allowed (without executing it).
    fn is_allowed(&self, command: &str) -> bool;

    /// Get the current configuration.
    fn config(&self) -> &ToolExecutorConfig;
}
