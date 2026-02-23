//! Context management and memory store ports.

use crate::domain::error::Result;

use super::{Ledger, LedgerEntry, Message};

/// Port for managing conversation context.
#[async_trait::async_trait]
pub trait ContextManager: Send + Sync {
    /// Add a message to the current session context.
    async fn add_message(&self, session_id: &str, message: Message) -> Result<()>;

    /// Build the system prompt from policy + ledger + constraints.
    async fn build_system_prompt(&self, session_id: &str) -> Result<String>;

    /// Build the user prompt from the latest user message.
    async fn build_user_prompt(&self, session_id: &str) -> Result<String>;

    /// Build the full agent input combining system, context, and task.
    async fn build_agent_input(&self, session_id: &str, task_description: &str, allowed_tools: &[String]) -> Result<String>;

    /// Trim the context to fit within configured limits.
    async fn trim_context(&self, session_id: &str) -> Result<()>;

    /// Get the current ledger for a session.
    async fn get_ledger(&self, session_id: &str) -> Result<Ledger>;

    /// Add a ledger entry.
    async fn add_ledger_entry(&self, session_id: &str, entry: LedgerEntry) -> Result<()>;

    /// Estimate total tokens for the current context.
    async fn estimate_tokens(&self, session_id: &str) -> Result<u64>;

    /// Clear all context for a session.
    async fn clear_session(&self, session_id: &str) -> Result<()>;
}

/// Port for long-term memory storage (pluggable).
#[async_trait::async_trait]
pub trait MemoryStore: Send + Sync {
    async fn store(&self, namespace: &str, key: &str, value: &str) -> Result<()>;
    async fn retrieve(&self, namespace: &str, key: &str) -> Result<Option<String>>;
    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>>;
    async fn delete(&self, namespace: &str, key: &str) -> Result<()>;
}
