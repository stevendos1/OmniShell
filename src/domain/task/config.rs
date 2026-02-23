//! Retry/timeout policies and TaskQueue port.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;

use super::SubTask;

/// Retry policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub backoff_multiplier: f64,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_retries: 3, base_delay: Duration::from_secs(1), backoff_multiplier: 2.0, max_delay: Duration::from_secs(30) }
    }
}

/// Timeout policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutPolicy {
    pub default_timeout: Duration,
    pub max_timeout: Duration,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self { default_timeout: Duration::from_secs(120), max_timeout: Duration::from_secs(600) }
    }
}

/// Port for a bounded task queue.
#[async_trait::async_trait]
pub trait TaskQueue: Send + Sync {
    async fn enqueue(&self, task: SubTask) -> Result<()>;
    async fn dequeue(&self, timeout: Duration) -> Result<Option<SubTask>>;
    async fn pending_count(&self) -> Result<usize>;
    fn capacity(&self) -> usize;
}
