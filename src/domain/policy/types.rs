//! Policy check result, violation, and config types.

use serde::{Deserialize, Serialize};

use crate::domain::error::Severity;

/// The result of a policy check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCheckResult {
    pub allowed: bool,
    pub severity: Severity,
    pub reason: String,
    pub violations: Vec<PolicyViolation>,
}

impl PolicyCheckResult {
    pub fn pass() -> Self {
        Self { allowed: true, severity: Severity::Info, reason: "passed all checks".to_string(), violations: Vec::new() }
    }

    pub fn fail(severity: Severity, reason: impl Into<String>) -> Self {
        Self { allowed: false, severity, reason: reason.into(), violations: Vec::new() }
    }

    pub fn with_violation(mut self, violation: PolicyViolation) -> Self {
        self.violations.push(violation);
        self
    }
}

/// A specific policy violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub kind: ViolationKind,
    pub description: String,
    pub redacted_content: Option<String>,
}

/// Categories of policy violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationKind {
    PromptInjection,
    ToolDenied,
    DataExfiltration,
    SuspiciousPattern,
    SizeExceeded,
}

/// Configuration for the policy guard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub enabled: bool,
    pub max_input_bytes: usize,
    pub suspicious_patterns: Vec<String>,
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
