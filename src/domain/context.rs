//! # Context Management Domain
//!
//! Defines the ports and types for conversation context, memory,
//! and the structured ledger used for inter-agent communication.

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// Role of a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    /// System instruction (not user-generated).
    System,
    /// User input.
    User,
    /// Assistant (agent) response.
    Assistant,
    /// Tool invocation result.
    Tool,
}

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message author.
    pub role: MessageRole,
    /// Textual content.
    pub content: String,
    /// Estimated token count for this message.
    pub token_estimate: u64,
    /// Unix timestamp (seconds).
    pub timestamp: i64,
}

impl Message {
    /// Byte size of the content (for RAM budgeting).
    pub fn byte_size(&self) -> usize {
        self.content.len()
    }
}

// ---------------------------------------------------------------------------
// Ledger (structured facts)
// ---------------------------------------------------------------------------

/// A structured fact stored in the ledger.
///
/// The ledger is an append-only list of typed entries that agents
/// can reference for grounding (anti-hallucination).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Category of the entry.
    pub kind: LedgerEntryKind,
    /// Human-readable key (e.g. `"project-language"`).
    pub key: String,
    /// The value.
    pub value: String,
    /// Unix timestamp.
    pub timestamp: i64,
    /// Which agent produced this entry (if any).
    pub source_agent: Option<String>,
}

/// Categories for ledger entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerEntryKind {
    /// A verified fact.
    Fact,
    /// A decision made during the session.
    Decision,
    /// A constraint or requirement.
    Constraint,
    /// A summary of previous context.
    Summary,
}

/// The full ledger for a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    /// Ordered list of entries.
    pub entries: Vec<LedgerEntry>,
}

impl Ledger {
    /// Create an empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry to the ledger.
    pub fn add(&mut self, entry: LedgerEntry) {
        self.entries.push(entry);
    }

    /// Retrieve entries by kind.
    pub fn by_kind(&self, kind: LedgerEntryKind) -> Vec<&LedgerEntry> {
        self.entries.iter().filter(|e| e.kind == kind).collect()
    }

    /// Estimated byte size of the ledger.
    pub fn byte_size(&self) -> usize {
        self.entries
            .iter()
            .map(|e| e.key.len() + e.value.len() + 64)
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Context Manager Configuration
// ---------------------------------------------------------------------------

/// Configuration for the context manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Maximum number of messages in the sliding window.
    pub max_messages: usize,
    /// Maximum total bytes for the message history.
    pub max_bytes: usize,
    /// Maximum token estimate for the context window.
    pub max_tokens: u64,
    /// Whether to enable incremental summarization.
    pub enable_summarization: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_messages: 100,
            max_bytes: 512 * 1024, // 512 KiB
            max_tokens: 8000,
            enable_summarization: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Ports
// ---------------------------------------------------------------------------

/// Port for managing conversation context.
///
/// Implementations handle sliding window, summarization, and ledger updates.
///
/// # Errors
/// Returns `OrchestratorError::ContextOverflow` if context cannot be trimmed
/// to fit within limits.
#[async_trait::async_trait]
pub trait ContextManager: Send + Sync {
    /// Add a message to the current session context.
    async fn add_message(&self, session_id: &str, message: Message) -> Result<()>;

    /// Build the system prompt from policy + ledger + constraints.
    ///
    /// Returns a rendered string. The system prompt is **typed** (built
    /// from structured data, not free-form user input).
    async fn build_system_prompt(&self, session_id: &str) -> Result<String>;

    /// Build the user prompt from the latest user message + task context.
    async fn build_user_prompt(&self, session_id: &str) -> Result<String>;

    /// Build the full agent input combining system, context, and task.
    async fn build_agent_input(
        &self,
        session_id: &str,
        task_description: &str,
        allowed_tools: &[String],
    ) -> Result<String>;

    /// Trim the context to fit within configured limits.
    /// May summarize older messages if `enable_summarization` is true.
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
///
/// This is optional — the orchestrator works without it, using only
/// short-term (in-session) memory. Implementations might persist to
/// disk, a database, or any other backend.
#[async_trait::async_trait]
pub trait MemoryStore: Send + Sync {
    /// Store a key-value pair in long-term memory.
    async fn store(&self, namespace: &str, key: &str, value: &str) -> Result<()>;

    /// Retrieve a value by key.
    async fn retrieve(&self, namespace: &str, key: &str) -> Result<Option<String>>;

    /// List all keys in a namespace.
    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>>;

    /// Delete a key.
    async fn delete(&self, namespace: &str, key: &str) -> Result<()>;
}
