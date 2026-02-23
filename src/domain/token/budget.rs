//! Token budget types and configuration.

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;

/// A token budget for a single request or session.
///
/// # Example
/// ```
/// use omnishell_orchestrator::domain::token::TokenBudget;
/// let budget = TokenBudget::new(4000);
/// assert_eq!(budget.remaining(), 4000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    pub limit: u64,
    pub used: u64,
}

impl TokenBudget {
    pub fn new(limit: u64) -> Self {
        Self { limit, used: 0 }
    }

    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    pub fn is_exhausted(&self) -> bool {
        self.used >= self.limit
    }

    /// Try to consume `amount` tokens.
    pub fn consume(&mut self, amount: u64) -> Result<()> {
        if self.used + amount > self.limit {
            return Err(crate::domain::error::OrchestratorError::TokenBudgetExceeded { used: self.used + amount, limit: self.limit });
        }
        self.used += amount;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.used = 0;
    }
}

/// Token budget configuration for the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetConfig {
    pub per_request_limit: u64,
    pub per_session_limit: u64,
}

impl Default for TokenBudgetConfig {
    fn default() -> Self {
        Self { per_request_limit: 8000, per_session_limit: 100_000 }
    }
}
