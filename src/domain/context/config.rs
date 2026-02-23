//! Context manager configuration.

use serde::{Deserialize, Serialize};

/// Configuration for the context manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub max_messages: usize,
    pub max_bytes: usize,
    pub max_tokens: u64,
    pub enable_summarization: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_messages: 100,
            max_bytes: 512 * 1024,
            max_tokens: 8000,
            enable_summarization: true,
        }
    }
}
