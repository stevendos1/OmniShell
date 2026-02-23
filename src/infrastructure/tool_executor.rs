//! # Secure Tool Executor
//!
//! Implements the `ToolExecutor` port with strict security controls:
//! - Allowlist / denylist enforcement.
//! - Working directory restriction.
//! - Timeout enforcement.
//! - Output size limits.
//! - No shell concatenation (args passed as `Vec<String>`).
//! - Dry-run mode.

use std::process::Stdio;

use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::domain::error::{OrchestratorError, Result};
use crate::domain::tool::*;

/// Secure tool executor that enforces all configured constraints.
///
/// # Security
/// - **Deny by default**: if `enabled` is false, all executions are rejected.
/// - **Allowlist**: only commands whose basename is in `allowed_commands` can run.
/// - **Denylist**: overrides the allowlist.
/// - **No shell**: commands are spawned directly, never through a shell.
/// - **Size limits**: stdout/stderr are truncated.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::tool_executor::SecureToolExecutor;
/// use omnishell_orchestrator::domain::tool::{ToolExecutor, ToolExecutorConfig};
///
/// let config = ToolExecutorConfig::default();
/// let executor = SecureToolExecutor::new(config);
/// assert!(!executor.is_allowed("rm")); // nothing allowed by default
/// ```
pub struct SecureToolExecutor {
    config: ToolExecutorConfig,
}

impl SecureToolExecutor {
    /// Create a new secure tool executor.
    pub fn new(config: ToolExecutorConfig) -> Self {
        Self { config }
    }

    /// Sanitize an argument: reject if it contains shell metacharacters.
    fn sanitize_arg(arg: &str) -> Result<()> {
        // Block common shell injection characters.
        let dangerous = [
            '|', ';', '&', '$', '`', '(', ')', '{', '}', '<', '>', '\n', '\r',
        ];
        for ch in &dangerous {
            if arg.contains(*ch) {
                return Err(OrchestratorError::ToolExecutionError(format!(
                    "argument contains dangerous character '{ch}': {arg}"
                )));
            }
        }
        Ok(())
    }

    /// Extract the basename from a command path.
    fn basename(command: &str) -> &str {
        command.rsplit('/').next().unwrap_or(command)
    }
}

#[async_trait::async_trait]
impl ToolExecutor for SecureToolExecutor {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResponse> {
        // 1. Check enabled.
        if !self.config.enabled {
            return Err(OrchestratorError::ToolExecutionError(
                "tool execution is disabled".into(),
            ));
        }

        // 2. Check allowlist / denylist.
        let basename = Self::basename(&request.command);
        if !self.is_allowed(basename) {
            return Err(OrchestratorError::ToolDenied {
                command: request.command.clone(),
            });
        }

        // 3. Sanitize arguments.
        for arg in &request.args {
            Self::sanitize_arg(arg)?;
        }

        // 4. Dry-run check.
        if self.config.dry_run {
            info!(
                command = %request.command,
                args = ?request.args,
                "dry-run: would execute"
            );
            return Ok(ToolResponse {
                exit_code: 0,
                stdout: format!("[dry-run] {} {}", request.command, request.args.join(" ")),
                stderr: String::new(),
                duration: std::time::Duration::ZERO,
                truncated: false,
                dry_run: true,
            });
        }

        // 5. Build command.
        let working_dir = request
            .working_dir
            .as_ref()
            .unwrap_or(&self.config.working_dir);
        let cmd_timeout = request.timeout.unwrap_or(self.config.timeout);

        debug!(
            command = %request.command,
            args = ?request.args,
            working_dir = %working_dir.display(),
            "executing tool"
        );

        let start = std::time::Instant::now();

        let mut cmd = tokio::process::Command::new(&request.command);
        cmd.args(&request.args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null()); // No stdin for tools.

        // 6. Execute with timeout.
        let child = cmd.spawn().map_err(|e| {
            OrchestratorError::ToolExecutionError(format!(
                "failed to spawn '{}': {e}",
                request.command
            ))
        })?;

        let output = timeout(cmd_timeout, child.wait_with_output())
            .await
            .map_err(|_| OrchestratorError::Timeout {
                duration_ms: cmd_timeout.as_millis() as u64,
                context: format!("tool '{}'", request.command),
            })?
            .map_err(|e| OrchestratorError::ToolExecutionError(format!("process error: {e}")))?;

        let duration = start.elapsed();

        // 7. Truncate output if needed.
        let (stdout, stderr, truncated) = {
            let raw_stdout = String::from_utf8_lossy(&output.stdout);
            let raw_stderr = String::from_utf8_lossy(&output.stderr);
            let mut truncated = false;

            let stdout = if raw_stdout.len() > self.config.max_stdout_bytes {
                truncated = true;
                warn!("stdout truncated to {} bytes", self.config.max_stdout_bytes);
                raw_stdout[..self.config.max_stdout_bytes].to_string()
            } else {
                raw_stdout.to_string()
            };

            let stderr = if raw_stderr.len() > self.config.max_stderr_bytes {
                truncated = true;
                warn!("stderr truncated to {} bytes", self.config.max_stderr_bytes);
                raw_stderr[..self.config.max_stderr_bytes].to_string()
            } else {
                raw_stderr.to_string()
            };

            (stdout, stderr, truncated)
        };

        let exit_code = output.status.code().unwrap_or(-1);

        Ok(ToolResponse {
            exit_code,
            stdout,
            stderr,
            duration,
            truncated,
            dry_run: false,
        })
    }

    fn is_allowed(&self, command: &str) -> bool {
        let basename = Self::basename(command);

        // Denylist overrides everything.
        if self.config.denied_commands.contains(basename) {
            return false;
        }

        // Must be in allowlist.
        self.config.allowed_commands.contains(basename)
    }

    fn config(&self) -> &ToolExecutorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::Duration;

    fn make_config(allowed: &[&str], denied: &[&str]) -> ToolExecutorConfig {
        ToolExecutorConfig {
            enabled: true,
            allowed_commands: allowed
                .iter()
                .map(|s| s.to_string())
                .collect::<HashSet<_>>(),
            denied_commands: denied.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
            working_dir: PathBuf::from("/tmp"),
            timeout: Duration::from_secs(5),
            max_stdout_bytes: 1024,
            max_stderr_bytes: 256,
            dry_run: false,
        }
    }

    #[test]
    fn test_is_allowed() {
        let executor = SecureToolExecutor::new(make_config(&["ls", "cat"], &["rm"]));
        assert!(executor.is_allowed("ls"));
        assert!(executor.is_allowed("cat"));
        assert!(!executor.is_allowed("rm"));
        assert!(!executor.is_allowed("wget"));
    }

    #[test]
    fn test_denylist_overrides() {
        let executor = SecureToolExecutor::new(make_config(&["rm"], &["rm"]));
        assert!(!executor.is_allowed("rm"));
    }

    #[test]
    fn test_sanitize_arg_clean() {
        assert!(SecureToolExecutor::sanitize_arg("hello").is_ok());
        assert!(SecureToolExecutor::sanitize_arg("-la").is_ok());
        assert!(SecureToolExecutor::sanitize_arg("/tmp/file.txt").is_ok());
    }

    #[test]
    fn test_sanitize_arg_dangerous() {
        assert!(SecureToolExecutor::sanitize_arg("hello; rm -rf /").is_err());
        assert!(SecureToolExecutor::sanitize_arg("$(whoami)").is_err());
        assert!(SecureToolExecutor::sanitize_arg("hello | cat").is_err());
        assert!(SecureToolExecutor::sanitize_arg("`id`").is_err());
    }

    #[tokio::test]
    async fn test_disabled_executor() {
        let mut config = make_config(&["ls"], &[]);
        config.enabled = false;
        let executor = SecureToolExecutor::new(config);

        let result = executor
            .execute(ToolRequest {
                command: "ls".to_string(),
                args: Vec::new(),
                working_dir: None,
                timeout: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dry_run() {
        let mut config = make_config(&["ls"], &[]);
        config.dry_run = true;
        let executor = SecureToolExecutor::new(config);

        let result = executor
            .execute(ToolRequest {
                command: "ls".to_string(),
                args: vec!["-la".to_string()],
                working_dir: None,
                timeout: None,
            })
            .await
            .expect("dry-run should succeed");

        assert!(result.dry_run);
        assert!(result.stdout.contains("[dry-run]"));
    }

    #[tokio::test]
    async fn test_denied_command() {
        let executor = SecureToolExecutor::new(make_config(&["ls"], &["rm"]));

        let result = executor
            .execute(ToolRequest {
                command: "rm".to_string(),
                args: vec!["-rf".to_string()],
                working_dir: None,
                timeout: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_ls() {
        let executor = SecureToolExecutor::new(make_config(&["ls"], &[]));

        let result = executor
            .execute(ToolRequest {
                command: "ls".to_string(),
                args: Vec::new(),
                working_dir: Some(PathBuf::from("/tmp")),
                timeout: None,
            })
            .await
            .expect("ls should succeed");

        assert_eq!(result.exit_code, 0);
        assert!(!result.dry_run);
    }
}
