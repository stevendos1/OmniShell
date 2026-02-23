//! Message types for conversation history.

use serde::{Deserialize, Serialize};

/// Role of a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub token_estimate: u64,
    pub timestamp: i64,
}

impl Message {
    /// Byte size of the content (for RAM budgeting).
    pub fn byte_size(&self) -> usize {
        self.content.len()
    }
}
