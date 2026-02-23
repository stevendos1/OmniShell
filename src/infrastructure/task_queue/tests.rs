//! Task queue unit tests.

use super::*;
use crate::domain::agent::AgentCapability;
use crate::domain::task::TaskStatus;

fn make_subtask(id: &str) -> SubTask {
    SubTask {
        id: id.into(),
        parent_request_id: "req-1".into(),
        description: "test".into(),
        required_capability: AgentCapability::new("test"),
        prompt: "do something".into(),
        max_tokens: None,
        timeout: None,
        status: TaskStatus::Pending,
        depends_on: Vec::new(),
        retry_count: 0,
        assigned_agent: None,
    }
}

#[tokio::test]
async fn test_enqueue_dequeue() {
    let q = BoundedTaskQueue::new("test", 10);
    q.enqueue(make_subtask("t1")).await.unwrap();
    assert_eq!(q.pending_count().await.unwrap(), 1);
    let t = q.dequeue(Duration::from_secs(1)).await.unwrap().unwrap();
    assert_eq!(t.id, "t1");
}

#[tokio::test]
async fn test_backpressure() {
    let q = BoundedTaskQueue::new("test", 2);
    q.enqueue(make_subtask("t1")).await.unwrap();
    q.enqueue(make_subtask("t2")).await.unwrap();
    assert!(q.enqueue(make_subtask("t3")).await.is_err());
}

#[tokio::test]
async fn test_dequeue_timeout() {
    let q = BoundedTaskQueue::new("test", 10);
    assert!(q
        .dequeue(Duration::from_millis(50))
        .await
        .unwrap()
        .is_none());
}
