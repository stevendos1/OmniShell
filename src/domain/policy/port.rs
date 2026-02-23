//! PolicyGuard port (trait).

use crate::domain::error::Result;

use super::PolicyCheckResult;

/// Port for the policy guard.
///
/// Checks user input, agent output, and tool requests against
/// configured policies.
pub trait PolicyGuard: Send + Sync {
    /// Validate user input before it reaches an agent.
    fn check_user_input(&self, input: &str) -> Result<PolicyCheckResult>;

    /// Validate a tool execution request.
    fn check_tool_request(&self, command: &str, args: &[String]) -> Result<PolicyCheckResult>;

    /// Validate agent output before returning to the user.
    fn check_agent_output(&self, output: &str) -> Result<PolicyCheckResult>;

    /// Redact sensitive data from a string (for logging).
    fn redact(&self, text: &str) -> String;
}
