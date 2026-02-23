//! In-memory context manager implementation.

mod manager;
#[cfg(test)]
mod tests;

pub use manager::InMemoryContextManager;

use crate::domain::context::*;

/// Per-session state managed by the context service.
#[derive(Debug, Clone)]
pub(crate) struct SessionContext {
    pub messages: Vec<Message>,
    pub ledger: Ledger,
    pub total_tokens: u64,
    pub total_bytes: usize,
}

impl SessionContext {
    pub fn new() -> Self {
        Self { messages: Vec::new(), ledger: Ledger::new(), total_tokens: 0, total_bytes: 0 }
    }
}
