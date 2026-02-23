//! # Bounded Task Queue
//!
//! Implementation of the `TaskQueue` port using a tokio bounded mpsc channel.
//! Provides backpressure when the queue is full.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::domain::error::{OrchestratorError, Result};
use crate::domain::task::{SubTask, TaskQueue};

/// A bounded task queue backed by a tokio mpsc channel.
///
/// When the queue is full, `enqueue` returns `QueueFull` (backpressure).
/// `dequeue` blocks up to the specified timeout.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::task_queue::BoundedTaskQueue;
///
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
    /// Create a new bounded task queue.
    ///
    /// # Arguments
    /// - `name`: Queue identifier (for logging and error messages).
    /// - `capacity`: Maximum number of pending tasks.
    pub fn new(name: impl Into<String>, capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self {
            name: name.into(),
            queue_capacity: capacity,
            tx,
            rx: tokio::sync::Mutex::new(rx),
            pending: AtomicUsize::new(0),
        }
    }
}

#[async_trait::async_trait]
impl TaskQueue for BoundedTaskQueue {
    async fn enqueue(&self, task: SubTask) -> Result<()> {
        self.tx.try_send(task).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => OrchestratorError::QueueFull {
                queue_name: self.name.clone(),
                capacity: self.queue_capacity,
            },
            mpsc::error::TrySendError::Closed(_) => {
                OrchestratorError::NotImplemented("queue channel closed".into())
            }
        })?;
        self.pending.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn dequeue(&self, wait_timeout: Duration) -> Result<Option<SubTask>> {
        let mut rx = self.rx.lock().await;
        match timeout(wait_timeout, rx.recv()).await {
            Ok(Some(task)) => {
                self.pending.fetch_sub(1, Ordering::SeqCst);
                Ok(Some(task))
            }
            Ok(None) => Ok(None), // Channel closed.
            Err(_) => Ok(None),   // Timeout.
        }
    }

    async fn pending_count(&self) -> Result<usize> {
        Ok(self.pending.load(Ordering::SeqCst))
    }

    fn capacity(&self) -> usize {
        self.queue_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::AgentCapability;
    use crate::domain::task::TaskStatus;

    fn make_subtask(id: &str) -> SubTask {
        SubTask {
            id: id.to_string(),
            parent_request_id: "req-1".to_string(),
            description: "test task".to_string(),
            required_capability: AgentCapability::new("test"),
            prompt: "do something".to_string(),
            max_tokens: None,
            timeout: None,
            status: TaskStatus::Pending,
            depends_on: Vec::new(),
            retry_count: 0,
            assigned_agent: None,
        }
    }

    #[tokio::test]
    async fn test_enqueue_and_dequeue() {
        let queue = BoundedTaskQueue::new("test-queue", 10);
        queue
            .enqueue(make_subtask("t1"))
            .await
            .expect("enqueue should succeed");

        let count = queue.pending_count().await.expect("count");
        assert_eq!(count, 1);

        let task = queue
            .dequeue(Duration::from_secs(1))
            .await
            .expect("dequeue should succeed")
            .expect("should have a task");
        assert_eq!(task.id, "t1");
    }

    #[tokio::test]
    async fn test_backpressure() {
        let queue = BoundedTaskQueue::new("test-queue", 2);
        queue.enqueue(make_subtask("t1")).await.expect("ok");
        queue.enqueue(make_subtask("t2")).await.expect("ok");

        // Third should fail.
        let result = queue.enqueue(make_subtask("t3")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dequeue_timeout() {
        let queue = BoundedTaskQueue::new("test-queue", 10);
        let result = queue
            .dequeue(Duration::from_millis(50))
            .await
            .expect("should not error");
        assert!(result.is_none());
    }
}
