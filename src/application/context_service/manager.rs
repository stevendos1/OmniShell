//! InMemoryContextManager — ContextManager port implementation.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::domain::context::*;
use crate::domain::error::{OrchestratorError, Result};
use crate::domain::token::TokenCounter;

use super::SessionContext;

/// In-memory context manager with sliding window and ledger.
pub struct InMemoryContextManager {
    config: ContextConfig,
    #[allow(dead_code)]
    token_counter: Arc<dyn TokenCounter>,
    sessions: Arc<RwLock<HashMap<String, SessionContext>>>,
}

impl InMemoryContextManager {
    pub fn new(config: ContextConfig, token_counter: Arc<dyn TokenCounter>) -> Self {
        Self {
            config,
            token_counter,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ContextManager for InMemoryContextManager {
    async fn add_message(&self, sid: &str, message: Message) -> Result<()> {
        let mut s = self.sessions.write().await;
        let ctx = s.entry(sid.to_string()).or_insert_with(SessionContext::new);
        ctx.total_tokens += message.token_estimate;
        ctx.total_bytes += message.byte_size();
        ctx.messages.push(message);
        debug!(
            session_id = sid,
            messages = ctx.messages.len(),
            "message added"
        );
        Ok(())
    }
    async fn build_system_prompt(&self, sid: &str) -> Result<String> {
        let s = self.sessions.read().await;
        let mut parts = vec!["You are a helpful AI assistant.".to_string()];
        if let Some(ctx) = s.get(sid) {
            for c in ctx.ledger.by_kind(LedgerEntryKind::Constraint) {
                parts.push(format!("[constraint] {}: {}", c.key, c.value));
            }
            for d in ctx.ledger.by_kind(LedgerEntryKind::Decision) {
                parts.push(format!("[decision] {}: {}", d.key, d.value));
            }
        }
        Ok(parts.join("\n"))
    }
    async fn build_user_prompt(&self, sid: &str) -> Result<String> {
        let s = self.sessions.read().await;
        let ctx = s
            .get(sid)
            .ok_or_else(|| OrchestratorError::ContextOverflow("session not found".into()))?;
        Ok(ctx
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default())
    }
    async fn build_agent_input(&self, sid: &str, task: &str, tools: &[String]) -> Result<String> {
        let (system, user) = (
            self.build_system_prompt(sid).await?,
            self.build_user_prompt(sid).await?,
        );
        let mut input = format!("[SYSTEM]\n{system}\n\n[CONTEXT]\n");
        let s = self.sessions.read().await;
        if let Some(ctx) = s.get(sid) {
            let start = ctx.messages.len().saturating_sub(self.config.max_messages);
            for msg in &ctx.messages[start..] {
                let r = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                };
                input.push_str(&format!("[{r}] {}\n", msg.content));
            }
        }
        input.push_str(&format!("\n[TASK]\n{task}\n"));
        if !tools.is_empty() {
            input.push_str(&format!("\n[ALLOWED_TOOLS]\n{}\n", tools.join(", ")));
        }
        if !user.is_empty() {
            input.push_str(&format!("\n[USER]\n{user}\n"));
        }
        Ok(input)
    }
    async fn trim_context(&self, sid: &str) -> Result<()> {
        let mut s = self.sessions.write().await;
        let ctx = match s.get_mut(sid) {
            Some(c) => c,
            None => return Ok(()),
        };
        while ctx.messages.len() > self.config.max_messages {
            trim_first(ctx);
            debug!(session_id = sid, "trimmed message");
        }
        while ctx.total_bytes > self.config.max_bytes && !ctx.messages.is_empty() {
            trim_first(ctx);
            warn!(session_id = sid, "trimmed (bytes)");
        }
        while ctx.total_tokens > self.config.max_tokens && !ctx.messages.is_empty() {
            trim_first(ctx);
            warn!(session_id = sid, "trimmed (tokens)");
        }
        Ok(())
    }
    async fn get_ledger(&self, sid: &str) -> Result<Ledger> {
        Ok(self
            .sessions
            .read()
            .await
            .get(sid)
            .map(|c| c.ledger.clone())
            .unwrap_or_default())
    }
    async fn add_ledger_entry(&self, sid: &str, entry: LedgerEntry) -> Result<()> {
        self.sessions
            .write()
            .await
            .entry(sid.to_string())
            .or_insert_with(SessionContext::new)
            .ledger
            .add(entry);
        Ok(())
    }
    async fn estimate_tokens(&self, sid: &str) -> Result<u64> {
        Ok(self
            .sessions
            .read()
            .await
            .get(sid)
            .map(|c| c.total_tokens)
            .unwrap_or(0))
    }
    async fn clear_session(&self, sid: &str) -> Result<()> {
        self.sessions.write().await.remove(sid);
        Ok(())
    }
}

fn trim_first(ctx: &mut SessionContext) {
    if let Some(r) = ctx.messages.first() {
        ctx.total_tokens = ctx.total_tokens.saturating_sub(r.token_estimate);
        ctx.total_bytes = ctx.total_bytes.saturating_sub(r.byte_size());
    }
    ctx.messages.remove(0);
}
