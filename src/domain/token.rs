//! # Token Counting Domain
//!
//! Defines the token counting port and budget types.

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;

/// Port for estimating token counts.
///
/// Implementations may use heuristic (e.g. chars/4) or call
/// a real tokenizer. The trait is intentionally simple so that
/// adapters can plug in any estimation strategy.
///
/// # Example
/// ```
/// use omnishell_orchestrator::domain::token::{TokenCounter, SimpleTokenCounter};
///
/// let counter = SimpleTokenCounter;
/// let count = counter.count_tokens("Hello, world!");
/// assert!(count > 0);
/// ```
pub trait TokenCounter: Send + Sync {
    /// Estimate the number of tokens in the given text.
    fn count_tokens(&self, text: &str) -> u64;
}

/// A simple heuristic token counter: roughly 1 token per 4 characters.
///
/// This is a reasonable approximation for English text and most LLM tokenizers.
/// For production accuracy, replace with a real tokenizer adapter.
#[derive(Debug, Clone, Default)]
pub struct SimpleTokenCounter;

impl TokenCounter for SimpleTokenCounter {
    fn count_tokens(&self, text: &str) -> u64 {
        // ~4 chars per token is a common heuristic.
        let len = text.len() as u64;
        (len + 3) / 4 // ceiling division
    }
}

/// A token budget for a single request or session.
///
/// # Invariants
/// - `used <= limit` is enforced by the context manager.
/// - If `used` would exceed `limit`, the context manager must
///   summarize or truncate before proceeding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Maximum tokens allowed.
    pub limit: u64,
    /// Tokens consumed so far.
    pub used: u64,
}

impl TokenBudget {
    /// Create a new budget with the given limit.
    ///
    /// # Example
    /// ```
    /// use omnishell_orchestrator::domain::token::TokenBudget;
    /// let budget = TokenBudget::new(4000);
    /// assert_eq!(budget.remaining(), 4000);
    /// ```
    pub fn new(limit: u64) -> Self {
        Self { limit, used: 0 }
    }

    /// How many tokens remain.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Whether the budget is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.used >= self.limit
    }

    /// Try to consume `amount` tokens.
    ///
    /// # Errors
    /// Returns `OrchestratorError::TokenBudgetExceeded` if there is
    /// not enough remaining budget.
    pub fn consume(&mut self, amount: u64) -> Result<()> {
        if self.used + amount > self.limit {
            return Err(
                crate::domain::error::OrchestratorError::TokenBudgetExceeded {
                    used: self.used + amount,
                    limit: self.limit,
                },
            );
        }
        self.used += amount;
        Ok(())
    }

    /// Reset usage to zero.
    pub fn reset(&mut self) {
        self.used = 0;
    }
}

/// Token budget configuration for the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetConfig {
    /// Maximum tokens per individual request.
    pub per_request_limit: u64,
    /// Maximum tokens per session.
    pub per_session_limit: u64,
}

impl Default for TokenBudgetConfig {
    fn default() -> Self {
        Self {
            per_request_limit: 8000,
            per_session_limit: 100_000,
        }
    }
}
