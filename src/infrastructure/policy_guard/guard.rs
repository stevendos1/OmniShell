//! DefaultPolicyGuard — PolicyGuard port implementation.

use regex::Regex;
use tracing::warn;

use crate::domain::error::{OrchestratorError, Result, Severity};
use crate::domain::policy::*;

/// Default policy guard implementation.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::policy_guard::DefaultPolicyGuard;
/// use omnishell_orchestrator::domain::policy::PolicyConfig;
/// let guard = DefaultPolicyGuard::new(PolicyConfig::default()).expect("ok");
/// ```
pub struct DefaultPolicyGuard {
    config: PolicyConfig,
    compiled_patterns: Vec<Regex>,
    allowed_tools: Vec<String>,
}

impl DefaultPolicyGuard {
    pub fn new(config: PolicyConfig) -> Result<Self> {
        let mut compiled = Vec::new();
        for p in &config.suspicious_patterns {
            compiled
                .push(Regex::new(p).map_err(|e| {
                    OrchestratorError::InvalidConfig(format!("pattern '{p}': {e}"))
                })?);
        }
        Ok(Self {
            config,
            compiled_patterns: compiled,
            allowed_tools: Vec::new(),
        })
    }
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }
}

impl PolicyGuard for DefaultPolicyGuard {
    fn check_user_input(&self, input: &str) -> Result<PolicyCheckResult> {
        if !self.config.enabled {
            return Ok(PolicyCheckResult::pass());
        }
        if input.len() > self.config.max_input_bytes {
            return Ok(PolicyCheckResult::fail(
                Severity::Critical,
                format!("size {} > {}", input.len(), self.config.max_input_bytes),
            )
            .with_violation(PolicyViolation {
                kind: ViolationKind::SizeExceeded,
                description: "input too large".into(),
                redacted_content: None,
            }));
        }
        for (i, re) in self.compiled_patterns.iter().enumerate() {
            if re.is_match(input) {
                warn!(pattern_index = i, "suspicious pattern in input");
                return Ok(PolicyCheckResult::fail(
                    Severity::Critical,
                    "potential prompt injection",
                )
                .with_violation(PolicyViolation {
                    kind: ViolationKind::PromptInjection,
                    description: format!("pattern #{i}"),
                    redacted_content: Some(self.redact(input)),
                }));
            }
        }
        Ok(PolicyCheckResult::pass())
    }

    fn check_tool_request(&self, command: &str, _args: &[String]) -> Result<PolicyCheckResult> {
        if !self.config.enabled {
            return Ok(PolicyCheckResult::pass());
        }
        if !self.allowed_tools.is_empty() && !self.allowed_tools.contains(&command.to_string()) {
            return Ok(PolicyCheckResult::fail(
                Severity::Critical,
                format!("tool '{command}' denied"),
            )
            .with_violation(PolicyViolation {
                kind: ViolationKind::ToolDenied,
                description: format!("'{command}' denied"),
                redacted_content: None,
            }));
        }
        Ok(PolicyCheckResult::pass())
    }

    fn check_agent_output(&self, output: &str) -> Result<PolicyCheckResult> {
        if !self.config.enabled {
            return Ok(PolicyCheckResult::pass());
        }
        if output.len() > self.config.max_input_bytes * 2 {
            return Ok(
                PolicyCheckResult::fail(Severity::Warning, "output too large").with_violation(
                    PolicyViolation {
                        kind: ViolationKind::SizeExceeded,
                        description: "output exceeds size".into(),
                        redacted_content: None,
                    },
                ),
            );
        }
        Ok(PolicyCheckResult::pass())
    }

    fn redact(&self, text: &str) -> String {
        if !self.config.enable_redaction {
            return text.to_string();
        }
        if text.len() > 50 {
            format!("{}...[REDACTED]", &text[..50])
        } else {
            text.to_string()
        }
    }
}
