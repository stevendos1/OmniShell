//! AiAgent port (trait).

use crate::domain::error::Result;

use super::{AgentInfo, AgentRequest, AgentResponse};

/// Port for interacting with an AI agent backend.
///
/// # Example (test double)
/// ```
/// use omnishell_orchestrator::domain::agent::*;
/// use omnishell_orchestrator::domain::error::Result;
///
/// struct FakeAgent;
///
/// #[async_trait::async_trait]
/// impl AiAgent for FakeAgent {
///     fn info(&self) -> &AgentInfo { unimplemented!() }
///     async fn execute(&self, _req: AgentRequest) -> Result<AgentResponse> {
///         Err(omnishell_orchestrator::domain::error::OrchestratorError::NotImplemented(
///             "fake".into()
///         ))
///     }
///     async fn health_check(&self) -> Result<()> { Ok(()) }
/// }
/// ```
#[async_trait::async_trait]
pub trait AiAgent: Send + Sync {
    /// Return static metadata about this agent.
    fn info(&self) -> &AgentInfo;
    /// Execute a request and return the parsed response.
    async fn execute(&self, request: AgentRequest) -> Result<AgentResponse>;
    /// Lightweight health check.
    async fn health_check(&self) -> Result<()>;
}
