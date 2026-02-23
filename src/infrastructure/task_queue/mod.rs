//! Bounded task queue.

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::domain::error::{OrchestratorError, Result};
use crate::domain::task::{SubTask, TaskQueue};

/// Bounded task queue backed by tokio mpsc.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::task_queue::BoundedTaskQueue;
/// let queue = BoundedTaskQueue::new("worker-1", 100);
/// ```
pub struct BoundedTaskQueue {
    name: String,
    queue_capacity: usize,
    tx: mpsc::Sender<SubTask>,
    rx: tokio::sync::Mutex<mpsc::Receiver<SubTask>>,
    pending: AtomicUsize,
}

impl BoundedTaskQueue {
    pub fn new(name: impl Into<String>, capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self { name: name.into(), queue_capacity: capacity, tx, rx: tokio::sync::Mutex::new(rx), pending: AtomicUsize::new(0) }
    }
}

#[async_trait::async_trait]
impl TaskQueue for BoundedTaskQueue {
    async fn enqueue(&self, task: SubTask) -> Result<()> {
        self.tx.try_send(task).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => OrchestratorError::QueueFull { queue_name: self.name.clone(), capacity: self.queue_capacity },
            mpsc::error::TrySendError::Closed(_) => OrchestratorError::NotImplemented("channel closed".into()),
        })?;
        self.pending.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn dequeue(&self, wait_timeout: Duration) -> Result<Option<SubTask>> {
        let mut rx = self.rx.lock().await;
        match timeout(wait_timeout, rx.recv()).await {
            Ok(Some(t)) => {
                self.pending.fetch_sub(1, Ordering::SeqCst);
                Ok(Some(t))
            }
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }
    async fn pending_count(&self) -> Result<usize> {
        Ok(self.pending.load(Ordering::SeqCst))
    }
    fn capacity(&self) -> usize {
        self.queue_capacity
    }
}
