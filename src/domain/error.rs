//! # Domain Errors
//!
//! Typed error hierarchy for the entire orchestrator.
//! Uses `thiserror` for ergonomic error definitions.
//! Production code must never use `.unwrap()` or `.expect()`.

use std::fmt;

/// Top-level error type for the orchestrator domain.
///
/// Every subsystem maps its errors into this enum so callers
/// always work with a single `Result<T, OrchestratorError>`.
///
/// # Invariants
/// - All variants carry enough context to diagnose the problem.
/// - No variant silently swallows inner errors.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    /// A feature or code path is not yet implemented.
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// Configuration is missing or invalid.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// An AI agent returned an error or could not be reached.
    #[error("agent error [{agent_id}]: {message}")]
    AgentError {
        /// Identifier of the failing agent.
        agent_id: String,
        /// Human-readable description.
        message: String,
        /// Optional underlying cause.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Token budget exceeded for a request or session.
    #[error("token budget exceeded: used {used}, limit {limit}")]
    TokenBudgetExceeded {
        /// Tokens consumed so far.
        used: u64,
        /// Configured limit.
        limit: u64,
    },

    /// The context window is too large and could not be trimmed.
    #[error("context overflow: {0}")]
    ContextOverflow(String),

    /// A tool execution was denied by policy or failed.
    #[error("tool execution error: {0}")]
    ToolExecutionError(String),

    /// A tool command was denied by the policy guard.
    #[error("tool denied by policy: {command}")]
    ToolDenied {
        /// The command that was denied.
        command: String,
    },

    /// Prompt injection or policy violation detected.
    #[error("policy violation: {0}")]
    PolicyViolation(String),

    /// Secrets could not be retrieved.
    #[error("secrets error: {0}")]
    SecretsError(String),

    /// A task timed out.
    #[error("timeout after {duration_ms}ms: {context}")]
    Timeout {
        /// How long we waited.
        duration_ms: u64,
        /// What was being done.
        context: String,
    },

    /// Circuit breaker tripped for an agent.
    #[error("circuit breaker open for agent {agent_id}: {reason}")]
    CircuitBreakerOpen {
        /// Which agent is paused.
        agent_id: String,
        /// Why.
        reason: String,
    },

    /// Cache error (serialization, capacity, etc.).
    #[error("cache error: {0}")]
    CacheError(String),

    /// Task queue is full (backpressure).
    #[error("task queue full for {queue_name}: capacity {capacity}")]
    QueueFull {
        /// Queue identifier.
        queue_name: String,
        /// Maximum capacity.
        capacity: usize,
    },

    /// Serialization / deserialization failure.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// I/O error wrapper.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Parse error for agent responses.
    #[error("parse error for agent {agent_id}: {message}")]
    ParseError {
        /// Which agent produced unparseable output.
        agent_id: String,
        /// What went wrong.
        message: String,
    },

    /// Rate limit hit for an agent.
    #[error("rate limited: agent {agent_id}")]
    RateLimited {
        /// Which agent.
        agent_id: String,
    },
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

/// Severity level for policy violations, used by the PolicyGuard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    /// Informational — logged but not blocking.
    Info,
    /// Warning — logged, may trigger rate limit.
    Warning,
    /// Critical — request is rejected.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}
