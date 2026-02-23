//! SecureToolExecutor — ToolExecutor port implementation.

use std::process::Stdio;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::domain::error::{OrchestratorError, Result};
use crate::domain::tool::*;

/// Secure tool executor enforcing all configured constraints.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::tool_executor::SecureToolExecutor;
/// use omnishell_orchestrator::domain::tool::{ToolExecutor, ToolExecutorConfig};
/// let executor = SecureToolExecutor::new(ToolExecutorConfig::default());
/// assert!(!executor.is_allowed("rm"));
/// ```
pub struct SecureToolExecutor { config: ToolExecutorConfig }

impl SecureToolExecutor {
    pub fn new(config: ToolExecutorConfig) -> Self { Self { config } }
    pub(super) fn sanitize_arg(arg: &str) -> Result<()> {
        for ch in &['|', ';', '&', '$', '`', '(', ')', '{', '}', '<', '>', '\n', '\r'] {
            if arg.contains(*ch) { return Err(OrchestratorError::ToolExecutionError(format!("dangerous char '{ch}': {arg}"))); }
        }
        Ok(())
    }
    fn basename(command: &str) -> &str { command.rsplit('/').next().unwrap_or(command) }
}

#[async_trait::async_trait]
impl ToolExecutor for SecureToolExecutor {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResponse> {
        if !self.config.enabled { return Err(OrchestratorError::ToolExecutionError("disabled".into())); }
        let bn = Self::basename(&request.command);
        if !self.is_allowed(bn) { return Err(OrchestratorError::ToolDenied { command: request.command.clone() }); }
        for arg in &request.args { Self::sanitize_arg(arg)?; }
        if self.config.dry_run {
            info!(command = %request.command, args = ?request.args, "dry-run");
            return Ok(ToolResponse { exit_code: 0, stdout: format!("[dry-run] {} {}", request.command, request.args.join(" ")), stderr: String::new(), duration: std::time::Duration::ZERO, truncated: false, dry_run: true });
        }
        let wd = request.working_dir.as_ref().unwrap_or(&self.config.working_dir);
        let ct = request.timeout.unwrap_or(self.config.timeout);
        debug!(command = %request.command, args = ?request.args, "executing");
        let start = std::time::Instant::now();
        let mut cmd = tokio::process::Command::new(&request.command);
        cmd.args(&request.args).current_dir(wd).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
        let child = cmd.spawn().map_err(|e| OrchestratorError::ToolExecutionError(format!("spawn '{}': {e}", request.command)))?;
        let output = timeout(ct, child.wait_with_output()).await
            .map_err(|_| OrchestratorError::Timeout { duration_ms: ct.as_millis() as u64, context: format!("tool '{}'", request.command) })?
            .map_err(|e| OrchestratorError::ToolExecutionError(format!("process: {e}")))?;
        let duration = start.elapsed();
        let (rs, re) = (String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let mut t = false;
        let so = if rs.len() > self.config.max_stdout_bytes { t = true; warn!("stdout truncated"); rs[..self.config.max_stdout_bytes].to_string() } else { rs.to_string() };
        let se = if re.len() > self.config.max_stderr_bytes { t = true; warn!("stderr truncated"); re[..self.config.max_stderr_bytes].to_string() } else { re.to_string() };
        Ok(ToolResponse { exit_code: output.status.code().unwrap_or(-1), stdout: so, stderr: se, duration, truncated: t, dry_run: false })
    }

    fn is_allowed(&self, command: &str) -> bool {
        let bn = Self::basename(command);
        if self.config.denied_commands.contains(bn) { return false; }
        self.config.allowed_commands.contains(bn)
    }

    fn config(&self) -> &ToolExecutorConfig { &self.config }
}
