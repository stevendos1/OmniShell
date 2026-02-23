//! # Response Aggregator
//!
//! Combines multiple agent responses into a single `AggregateResponse`
//! with full traceability.

use crate::domain::agent::AgentResponse;
use crate::domain::error::Result;
use crate::domain::orchestrator::{AggregateResponse, Aggregator, CacheStats, WorkerResult};

/// Default aggregator that concatenates agent responses.
///
/// For more sophisticated aggregation (e.g. using an AI agent to
/// synthesize), implement `Aggregator` with a custom strategy.
///
/// # Example
/// ```
/// use omnishell_orchestrator::application::aggregator::ConcatAggregator;
///
/// let aggregator = ConcatAggregator::new("\n---\n");
/// ```
pub struct ConcatAggregator {
    separator: String,
}

impl ConcatAggregator {
    /// Create a new aggregator with the given separator between responses.
    pub fn new(separator: impl Into<String>) -> Self {
        Self {
            separator: separator.into(),
        }
    }
}

impl Default for ConcatAggregator {
    fn default() -> Self {
        Self::new("\n\n")
    }
}

#[async_trait::async_trait]
impl Aggregator for ConcatAggregator {
    async fn aggregate(
        &self,
        request_id: &str,
        responses: Vec<AgentResponse>,
    ) -> Result<AggregateResponse> {
        let mut total_tokens = 0u64;
        let mut total_duration = std::time::Duration::ZERO;
        let mut cache_stats = CacheStats::default();
        let mut worker_results = Vec::with_capacity(responses.len());
        let mut contents = Vec::with_capacity(responses.len());

        for resp in &responses {
            total_tokens = total_tokens.saturating_add(resp.estimated_tokens);
            total_duration += resp.duration;

            if resp.cache_hit {
                cache_stats.hits += 1;
            } else {
                cache_stats.misses += 1;
            }

            worker_results.push(WorkerResult {
                agent_id: resp.agent_id.clone(),
                subtask_description: String::new(), // filled by the orchestrator service
                duration: resp.duration,
                estimated_tokens: resp.estimated_tokens,
                cache_hit: resp.cache_hit,
                errors: resp.warnings.clone(),
            });

            contents.push(resp.content.clone());
        }

        Ok(AggregateResponse {
            request_id: request_id.to_string(),
            content: contents.join(&self.separator),
            structured_data: None,
            worker_results,
            total_tokens,
            total_duration,
            cache_stats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::AgentResponse;
    use std::time::Duration;

    fn make_response(agent_id: &str, content: &str, cache_hit: bool) -> AgentResponse {
        AgentResponse {
            request_id: "req-1".to_string(),
            agent_id: agent_id.to_string(),
            content: content.to_string(),
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

        let result = agg
            .aggregate("req-1", responses)
            .await
            .expect("aggregation should succeed");

        assert_eq!(result.content, "Hello | World");
        assert_eq!(result.total_tokens, 200);
        assert_eq!(result.cache_stats.hits, 1);
        assert_eq!(result.cache_stats.misses, 1);
        assert_eq!(result.worker_results.len(), 2);
    }

    #[tokio::test]
    async fn test_empty_responses() {
        let agg = ConcatAggregator::default();
        let result = agg
            .aggregate("req-2", Vec::new())
            .await
            .expect("empty aggregation should succeed");
        assert!(result.content.is_empty());
        assert_eq!(result.total_tokens, 0);
    }
}
