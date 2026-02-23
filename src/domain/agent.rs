//! # AI Agent Domain
//!
//! Defines the port (trait) for interacting with any AI agent backend,
//! plus the associated request/response/capability types.
//!
//! Adapters (infrastructure layer) implement `AiAgent` for each
//! concrete CLI or API backend.

use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

/// A declared capability that an agent can fulfill.
///
/// Capabilities are free-form strings (e.g. `"code-generation"`,
/// `"summarization"`, `"code-review"`). The router uses them
/// to match tasks to suitable workers.
///
/// # Example
/// ```
/// use omnishell_orchestrator::domain::agent::AgentCapability;
/// let cap = AgentCapability::new("code-generation");
/// assert_eq!(cap.name(), "code-generation");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentCapability {
    name: String,
}

impl AgentCapability {
    /// Create a new capability with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// The capability name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Agent metadata
// ---------------------------------------------------------------------------

/// Static metadata describing an agent worker.
///
/// This is used by the router to decide which agent to dispatch to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Unique identifier (e.g. `"claude-cli"`, `"codex-cli"`).
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Set of capabilities this agent declares.
    pub capabilities: HashSet<AgentCapability>,
    /// Maximum concurrent requests allowed for this agent.
    pub max_concurrency: usize,
    /// Default timeout per request.
    pub default_timeout: Duration,
    /// Whether this agent is currently enabled.
    pub enabled: bool,
    /// Priority weight (higher = preferred when multiple agents match).
    pub priority: u32,
}

impl AgentInfo {
    /// Check whether this agent declares a given capability.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.contains(&AgentCapability::new(cap))
    }
}

// ---------------------------------------------------------------------------
// Request / Response
// ---------------------------------------------------------------------------

/// A request sent to an AI agent.
///
/// The orchestrator builds this from the user's task plus context.
/// Adapters translate it into whatever format the underlying CLI expects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// Unique request ID for tracing.
    pub request_id: String,
    /// System-level instructions (policy, role, constraints).
    /// Kept separate from user content to mitigate prompt injection.
    pub system_prompt: String,
    /// The actual user task / message.
    pub user_prompt: String,
    /// Serialized context (previous conversation, facts, etc.).
    pub context: String,
    /// Which tools the agent is allowed to invoke, if any.
    pub allowed_tools: Vec<String>,
    /// Maximum tokens the response should consume (advisory).
    pub max_response_tokens: Option<u64>,
}

/// The response received from an AI agent.
///
/// Adapters parse the raw CLI output into this structure.
/// If parsing fails the adapter should return `OrchestratorError::ParseError`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// The request ID this is replying to.
    pub request_id: String,
    /// Agent that produced this response.
    pub agent_id: String,
    /// The main textual content.
    pub content: String,
    /// Structured data (if the agent returned JSON).
    pub structured_data: Option<serde_json::Value>,
    /// Estimated token count of the response.
    pub estimated_tokens: u64,
    /// Wall-clock duration the agent took.
    pub duration: Duration,
    /// Whether this was served from cache.
    pub cache_hit: bool,
    /// Optional error/warning messages from the agent.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Trait (Port)
// ---------------------------------------------------------------------------

/// Port for interacting with an AI agent backend.
///
/// Each concrete CLI adapter implements this trait.
/// The orchestrator only depends on this trait, never on concrete adapters.
///
/// # Errors
/// Returns `OrchestratorError::AgentError` if the agent fails,
/// `OrchestratorError::Timeout` if the request exceeds the configured timeout,
/// `OrchestratorError::ParseError` if the output cannot be parsed.
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

    /// Lightweight health check (e.g. verify CLI binary exists).
    async fn health_check(&self) -> Result<()>;
}
