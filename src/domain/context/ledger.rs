//! Structured ledger for inter-agent communication.

use serde::{Deserialize, Serialize};

/// A structured fact stored in the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub kind: LedgerEntryKind,
    pub key: String,
    pub value: String,
    pub timestamp: i64,
    pub source_agent: Option<String>,
}

/// Categories for ledger entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerEntryKind {
    Fact,
    Decision,
    Constraint,
    Summary,
}

/// The full ledger for a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub entries: Vec<LedgerEntry>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, entry: LedgerEntry) {
        self.entries.push(entry);
    }

    pub fn by_kind(&self, kind: LedgerEntryKind) -> Vec<&LedgerEntry> {
        self.entries.iter().filter(|e| e.kind == kind).collect()
    }

    pub fn byte_size(&self) -> usize {
        self.entries.iter().map(|e| e.key.len() + e.value.len() + 64).sum()
    }
}
