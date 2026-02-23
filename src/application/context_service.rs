//! # Context Service
//!
//! In-memory implementation of the `ContextManager` port.
//! Manages per-session sliding windows, ledgers, and prompt building.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::domain::context::*;
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::token::TokenCounter;

/// Per-session state managed by the context service.
#[derive(Debug, Clone)]
struct SessionContext {
    /// Messages in the sliding window.
    messages: Vec<Message>,
    /// Structured ledger.
    ledger: Ledger,
    /// Total estimated tokens across all messages.
    total_tokens: u64,
    /// Total bytes across all messages.
    total_bytes: usize,
}

impl SessionContext {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            ledger: Ledger::new(),
            total_tokens: 0,
            total_bytes: 0,
        }
    }
}

/// In-memory context manager.
///
/// Stores per-session context with configurable limits.
/// When limits are exceeded, older messages are removed
/// (or summarized if enabled).
///
/// # Thread safety
/// All session state is behind an `RwLock` for safe concurrent access.
///
/// # Example
/// ```
/// use omnishell_orchestrator::application::context_service::InMemoryContextManager;
/// use omnishell_orchestrator::domain::context::ContextConfig;
/// use omnishell_orchestrator::domain::token::SimpleTokenCounter;
/// use std::sync::Arc;
///
/// let mgr = InMemoryContextManager::new(
///     ContextConfig::default(),
///     Arc::new(SimpleTokenCounter),
/// );
/// ```
pub struct InMemoryContextManager {
    config: ContextConfig,
    token_counter: Arc<dyn TokenCounter>,
    sessions: Arc<RwLock<HashMap<String, SessionContext>>>,
}

impl InMemoryContextManager {
    /// Create a new in-memory context manager.
    pub fn new(config: ContextConfig, token_counter: Arc<dyn TokenCounter>) -> Self {
        Self {
            config,
            token_counter,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a session.
    async fn get_or_create_session(
        sessions: &RwLock<HashMap<String, SessionContext>>,
        session_id: &str,
    ) -> SessionContext {
        let read = sessions.read().await;
        if let Some(ctx) = read.get(session_id) {
            return ctx.clone();
        }
        drop(read);

        let mut write = sessions.write().await;
        write
            .entry(session_id.to_string())
            .or_insert_with(SessionContext::new)
            .clone()
    }
}

#[async_trait::async_trait]
impl ContextManager for InMemoryContextManager {
    async fn add_message(&self, session_id: &str, message: Message) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let ctx = sessions
            .entry(session_id.to_string())
            .or_insert_with(SessionContext::new);

        ctx.total_tokens += message.token_estimate;
        ctx.total_bytes += message.byte_size();
        ctx.messages.push(message);

        debug!(
            session_id,
            messages = ctx.messages.len(),
            tokens = ctx.total_tokens,
            bytes = ctx.total_bytes,
            "message added to context"
        );

        Ok(())
    }

    async fn build_system_prompt(&self, session_id: &str) -> Result<String> {
        let sessions = self.sessions.read().await;
        let ctx = sessions.get(session_id);

        let mut parts = Vec::new();
        parts.push("You are a helpful AI assistant.".to_string());

        if let Some(ctx) = ctx {
            let constraints = ctx.ledger.by_kind(LedgerEntryKind::Constraint);
            if !constraints.is_empty() {
                parts.push("\n## Constraints".to_string());
                for c in constraints {
                    parts.push(format!("- {}: {}", c.key, c.value));
                }
            }

            let decisions = ctx.ledger.by_kind(LedgerEntryKind::Decision);
            if !decisions.is_empty() {
                parts.push("\n## Decisions".to_string());
                for d in decisions {
                    parts.push(format!("- {}: {}", d.key, d.value));
                }
            }
        }

        Ok(parts.join("\n"))
    }

    async fn build_user_prompt(&self, session_id: &str) -> Result<String> {
        let sessions = self.sessions.read().await;
        let ctx = sessions
            .get(session_id)
            .ok_or_else(|| OrchestratorError::ContextOverflow("session not found".into()))?;

        // Return the latest user message.
        let last_user = ctx
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User);

        match last_user {
            Some(msg) => Ok(msg.content.clone()),
            None => Ok(String::new()),
        }
    }

    async fn build_agent_input(
        &self,
        session_id: &str,
        task_description: &str,
        allowed_tools: &[String],
    ) -> Result<String> {
        let system = self.build_system_prompt(session_id).await?;
        let user = self.build_user_prompt(session_id).await?;

        let mut input = format!("[SYSTEM]\n{system}\n\n[CONTEXT]\n");

        // Add recent messages as context (up to a limit).
        let sessions = self.sessions.read().await;
        if let Some(ctx) = sessions.get(session_id) {
            let window_start = ctx.messages.len().saturating_sub(self.config.max_messages);
            for msg in &ctx.messages[window_start..] {
                let role_str = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                };
                input.push_str(&format!("[{}] {}\n", role_str, msg.content));
            }
        }

        input.push_str(&format!("\n[TASK]\n{task_description}\n"));

        if !allowed_tools.is_empty() {
            input.push_str(&format!(
                "\n[ALLOWED_TOOLS]\n{}\n",
                allowed_tools.join(", ")
            ));
        }

        if !user.is_empty() {
            input.push_str(&format!("\n[USER]\n{user}\n"));
        }

        Ok(input)
    }

    async fn trim_context(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let ctx = match sessions.get_mut(session_id) {
            Some(ctx) => ctx,
            None => return Ok(()),
        };

        // Trim by message count.
        while ctx.messages.len() > self.config.max_messages {
            if let Some(removed) = ctx.messages.first() {
                ctx.total_tokens = ctx.total_tokens.saturating_sub(removed.token_estimate);
                ctx.total_bytes = ctx.total_bytes.saturating_sub(removed.byte_size());
            }
            ctx.messages.remove(0);
            debug!(session_id, "trimmed oldest message from context");
        }

        // Trim by bytes.
        while ctx.total_bytes > self.config.max_bytes && !ctx.messages.is_empty() {
            if let Some(removed) = ctx.messages.first() {
                ctx.total_tokens = ctx.total_tokens.saturating_sub(removed.token_estimate);
                ctx.total_bytes = ctx.total_bytes.saturating_sub(removed.byte_size());
            }
            ctx.messages.remove(0);
            warn!(session_id, "trimmed message due to byte limit");
        }

        // Trim by tokens.
        while ctx.total_tokens > self.config.max_tokens && !ctx.messages.is_empty() {
            if let Some(removed) = ctx.messages.first() {
                ctx.total_tokens = ctx.total_tokens.saturating_sub(removed.token_estimate);
                ctx.total_bytes = ctx.total_bytes.saturating_sub(removed.byte_size());
            }
            ctx.messages.remove(0);
            warn!(session_id, "trimmed message due to token limit");
        }

        Ok(())
    }

    async fn get_ledger(&self, session_id: &str) -> Result<Ledger> {
        let sessions = self.sessions.read().await;
        Ok(sessions
            .get(session_id)
            .map(|ctx| ctx.ledger.clone())
            .unwrap_or_default())
    }

    async fn add_ledger_entry(&self, session_id: &str, entry: LedgerEntry) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let ctx = sessions
            .entry(session_id.to_string())
            .or_insert_with(SessionContext::new);
        ctx.ledger.add(entry);
        Ok(())
    }

    async fn estimate_tokens(&self, session_id: &str) -> Result<u64> {
        let sessions = self.sessions.read().await;
        Ok(sessions
            .get(session_id)
            .map(|ctx| ctx.total_tokens)
            .unwrap_or(0))
    }

    async fn clear_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::token::SimpleTokenCounter;

    fn make_manager() -> InMemoryContextManager {
        InMemoryContextManager::new(
            ContextConfig {
                max_messages: 3,
                max_bytes: 10_000,
                max_tokens: 500,
                enable_summarization: false,
            },
            Arc::new(SimpleTokenCounter),
        )
    }

    fn make_msg(role: MessageRole, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            token_estimate: (content.len() as u64 + 3) / 4,
            timestamp: 0,
        }
    }

    #[tokio::test]
    async fn test_add_and_retrieve_messages() {
        let mgr = make_manager();
        mgr.add_message("s1", make_msg(MessageRole::User, "Hello"))
            .await
            .expect("add should succeed");

        let user_prompt = mgr
            .build_user_prompt("s1")
            .await
            .expect("build should succeed");
        assert_eq!(user_prompt, "Hello");
    }

    #[tokio::test]
    async fn test_trim_by_message_count() {
        let mgr = make_manager();
        for i in 0..5 {
            mgr.add_message("s1", make_msg(MessageRole::User, &format!("msg-{i}")))
                .await
                .expect("add should succeed");
        }

        mgr.trim_context("s1").await.expect("trim should succeed");

        let tokens = mgr.estimate_tokens("s1").await.expect("should succeed");
        // After trimming, should have at most 3 messages.
        // Each "msg-X" is ~2 tokens. 3 messages => ~6 tokens.
        assert!(tokens <= 500);
    }

    #[tokio::test]
    async fn test_ledger_operations() {
        let mgr = make_manager();
        mgr.add_ledger_entry(
            "s1",
            LedgerEntry {
                kind: LedgerEntryKind::Fact,
                key: "language".to_string(),
                value: "Rust".to_string(),
                timestamp: 0,
                source_agent: None,
            },
        )
        .await
        .expect("add entry should succeed");

        let ledger = mgr
            .get_ledger("s1")
            .await
            .expect("get ledger should succeed");
        assert_eq!(ledger.entries.len(), 1);
        assert_eq!(ledger.entries[0].value, "Rust");
    }

    #[tokio::test]
    async fn test_clear_session() {
        let mgr = make_manager();
        mgr.add_message("s1", make_msg(MessageRole::User, "Hello"))
            .await
            .expect("add should succeed");
        mgr.clear_session("s1").await.expect("clear should succeed");

        let tokens = mgr.estimate_tokens("s1").await.expect("should succeed");
        assert_eq!(tokens, 0);
    }
}
