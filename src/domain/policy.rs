//! # Policy Guard Domain
//!
//! Defines the port for prompt injection mitigation and tool execution guards.
//!
//! The policy guard acts as a security layer between user input and
//! agent execution, enforcing that:
//! - System instructions cannot be overridden by user content.
//! - Tool execution is explicitly allowed per policy.
//! - Potentially malicious patterns are detected and blocked.

use serde::{Deserialize, Serialize};

use crate::domain::error::{Result, Severity};

/// The result of a policy check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCheckResult {
    /// Whether the content passed the policy check.
    pub allowed: bool,
    /// Severity if not allowed.
    pub severity: Severity,
    /// Reason for the decision.
    pub reason: String,
    /// Specific violations found.
    pub violations: Vec<PolicyViolation>,
}

impl PolicyCheckResult {
    /// Create a passing result.
    pub fn pass() -> Self {
        Self {
            allowed: true,
            severity: Severity::Info,
            reason: "passed all checks".to_string(),
            violations: Vec::new(),
        }
    }

    /// Create a failing result.
    pub fn fail(severity: Severity, reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            severity,
            reason: reason.into(),
            violations: Vec::new(),
        }
    }

    /// Add a violation to the result.
    pub fn with_violation(mut self, violation: PolicyViolation) -> Self {
        self.violations.push(violation);
        self
    }
}

/// A specific policy violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    /// Type of violation.
    pub kind: ViolationKind,
    /// Description.
    pub description: String,
    /// The offending content (redacted if sensitive).
    pub redacted_content: Option<String>,
}

/// Categories of policy violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationKind {
    /// Attempt to override system instructions.
    PromptInjection,
    /// Attempt to execute a denied tool.
    ToolDenied,
    /// Attempt to exfiltrate data.
    DataExfiltration,
    /// Suspicious pattern in input.
    SuspiciousPattern,
    /// Input exceeds size limits.
    SizeExceeded,
}

/// Configuration for the policy guard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Whether the policy guard is enabled.
    pub enabled: bool,
    /// Maximum input size in bytes.
    pub max_input_bytes: usize,
    /// Patterns considered suspicious (regexes as strings).
    pub suspicious_patterns: Vec<String>,
    /// Whether to enable log redaction for sensitive data.
    pub enable_redaction: bool,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_input_bytes: 100_000,
            suspicious_patterns: vec![
                r"(?i)ignore\s+(all\s+)?(previous|above)\s+instructions".to_string(),
                r"(?i)system\s*:\s*you\s+are".to_string(),
                r"(?i)disregard\s+(all\s+)?(prior|previous)".to_string(),
            ],
            enable_redaction: true,
        }
    }
}

/// Port for the policy guard.
///
/// Checks user input, agent output, and tool requests against
/// configured policies.
///
/// # Security
/// - System prompts are typed structs, not user-generated strings.
/// - User content is validated before being sent to agents.
/// - Tool commands are validated against an explicit allowlist.
pub trait PolicyGuard: Send + Sync {
    /// Validate user input before it reaches an agent.
    ///
    /// # Errors
    /// Returns `OrchestratorError::PolicyViolation` if the check fails
    /// critically.
    fn check_user_input(&self, input: &str) -> Result<PolicyCheckResult>;

    /// Validate a tool execution request.
    fn check_tool_request(&self, command: &str, args: &[String]) -> Result<PolicyCheckResult>;

    /// Validate agent output before returning to the user.
    fn check_agent_output(&self, output: &str) -> Result<PolicyCheckResult>;

    /// Redact sensitive data from a string (for logging).
    fn redact(&self, text: &str) -> String;
}
