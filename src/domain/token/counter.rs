//! TokenCounter port and SimpleTokenCounter implementation.

/// Port for estimating token counts.
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
    fn count_tokens(&self, text: &str) -> u64;
}

/// A simple heuristic token counter: roughly 1 token per 4 characters.
#[derive(Debug, Clone, Default)]
pub struct SimpleTokenCounter;

impl TokenCounter for SimpleTokenCounter {
    fn count_tokens(&self, text: &str) -> u64 {
        let len = text.len() as u64;
        len.div_ceil(4)
    }
}
