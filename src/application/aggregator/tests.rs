//! Aggregator unit tests.

use super::*;
use std::time::Duration;

fn make_response(agent_id: &str, content: &str, cache_hit: bool) -> AgentResponse {
    AgentResponse {
        request_id: "req-1".into(),
        agent_id: agent_id.into(),
        content: content.into(),
        structured_data: None,
        estimated_tokens: 100,
        duration: Duration::from_millis(50),
        cache_hit,
        warnings: Vec::new(),
    }
}

#[tokio::test]
async fn test_concat_aggregator() {
    let agg = ConcatAggregator::new(" | ");
    let responses = vec![
        make_response("a", "Hello", false),
        make_response("b", "World", true),
    ];
    let result = agg.aggregate("req-1", responses).await.expect("ok");
    assert_eq!(result.content, "Hello | World");
    assert_eq!(result.total_tokens, 200);
    assert_eq!(result.cache_stats.hits, 1);
    assert_eq!(result.cache_stats.misses, 1);
    assert_eq!(result.worker_results.len(), 2);
}

#[tokio::test]
async fn test_empty_responses() {
    let agg = ConcatAggregator::default();
    let result = agg.aggregate("req-2", Vec::new()).await.expect("ok");
    assert!(result.content.is_empty());
    assert_eq!(result.total_tokens, 0);
}
