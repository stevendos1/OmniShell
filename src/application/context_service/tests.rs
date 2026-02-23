//! Context service unit tests.

use std::sync::Arc;

use crate::domain::context::*;
use crate::domain::token::SimpleTokenCounter;

use super::InMemoryContextManager;

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
        content: content.into(),
        token_estimate: (content.len() as u64).div_ceil(4),
        timestamp: 0,
    }
}

#[tokio::test]
async fn test_add_and_retrieve() {
    let mgr = make_manager();
    mgr.add_message("s1", make_msg(MessageRole::User, "Hello"))
        .await
        .unwrap();
    assert_eq!(mgr.build_user_prompt("s1").await.unwrap(), "Hello");
}

#[tokio::test]
async fn test_trim_by_count() {
    let mgr = make_manager();
    for i in 0..5 {
        mgr.add_message("s1", make_msg(MessageRole::User, &format!("msg-{i}")))
            .await
            .unwrap();
    }
    mgr.trim_context("s1").await.unwrap();
    assert!(mgr.estimate_tokens("s1").await.unwrap() <= 500);
}

#[tokio::test]
async fn test_ledger() {
    let mgr = make_manager();
    mgr.add_ledger_entry(
        "s1",
        LedgerEntry {
            kind: LedgerEntryKind::Fact,
            key: "lang".into(),
            value: "Rust".into(),
            timestamp: 0,
            source_agent: None,
        },
    )
    .await
    .unwrap();
    let ledger = mgr.get_ledger("s1").await.unwrap();
    assert_eq!(ledger.entries.len(), 1);
    assert_eq!(ledger.entries[0].value, "Rust");
}

#[tokio::test]
async fn test_clear_session() {
    let mgr = make_manager();
    mgr.add_message("s1", make_msg(MessageRole::User, "Hello"))
        .await
        .unwrap();
    mgr.clear_session("s1").await.unwrap();
    assert_eq!(mgr.estimate_tokens("s1").await.unwrap(), 0);
}
