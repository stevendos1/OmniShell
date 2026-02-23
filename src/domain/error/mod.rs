//! Domain errors — typed error hierarchy.

mod severity;

pub use severity::Severity;

/// Top-level error type for the orchestrator domain.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("agent error [{agent_id}]: {message}")]
    AgentError {
        agent_id: String,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    #[error("token budget exceeded: used {used}, limit {limit}")]
    TokenBudgetExceeded { used: u64, limit: u64 },
    #[error("context overflow: {0}")]
    ContextOverflow(String),
    #[error("tool execution error: {0}")]
    ToolExecutionError(String),
    #[error("tool denied by policy: {command}")]
    ToolDenied { command: String },
    #[error("policy violation: {0}")]
    PolicyViolation(String),
    #[error("secrets error: {0}")]
    SecretsError(String),
    #[error("timeout after {duration_ms}ms: {context}")]
    Timeout { duration_ms: u64, context: String },
    #[error("circuit breaker open for agent {agent_id}: {reason}")]
    CircuitBreakerOpen { agent_id: String, reason: String },
    #[error("cache error: {0}")]
    CacheError(String),
    #[error("task queue full for {queue_name}: capacity {capacity}")]
    QueueFull { queue_name: String, capacity: usize },
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("parse error for agent {agent_id}: {message}")]
    ParseError { agent_id: String, message: String },
    #[error("rate limited: agent {agent_id}")]
    RateLimited { agent_id: String },
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, OrchestratorError>;

impl OrchestratorError {
    /// Create an `AgentError` without an underlying source.
    pub fn agent(agent_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::AgentError {
            agent_id: agent_id.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Create an `AgentError` with an underlying source.
    pub fn agent_with_source(
        agent_id: impl Into<String>,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::AgentError {
            agent_id: agent_id.into(),
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}
