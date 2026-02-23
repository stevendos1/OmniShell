//! ToolExecutor port (trait).

use crate::domain::error::Result;

use super::{ToolExecutorConfig, ToolRequest, ToolResponse};

/// Port for executing local tools securely.
///
/// # Security
/// - Deny by default: only `allowed_commands` can run.
/// - Arguments are passed as a `Vec<String>`, never through a shell.
/// - Output is size-limited.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::tool_executor::SecureToolExecutor;
/// use omnishell_orchestrator::domain::tool::{ToolExecutor, ToolExecutorConfig};
///
/// let config = ToolExecutorConfig::default();
/// let executor = SecureToolExecutor::new(config);
/// assert!(!executor.is_allowed("rm"));
/// ```
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResponse>;
    fn is_allowed(&self, command: &str) -> bool;
    fn config(&self) -> &ToolExecutorConfig;
}
