//! Response aggregator.

#[cfg(test)]
mod tests;

use crate::domain::agent::AgentResponse;
use crate::domain::error::Result;
use crate::domain::orchestrator::{AggregateResponse, Aggregator, CacheStats, WorkerResult};

/// Default aggregator that concatenates agent responses.
///
/// # Example
/// ```
/// use omnishell_orchestrator::application::aggregator::ConcatAggregator;
/// let aggregator = ConcatAggregator::new("\n---\n");
/// ```
pub struct ConcatAggregator {
    separator: String,
}

impl ConcatAggregator {
    pub fn new(separator: impl Into<String>) -> Self {
        Self { separator: separator.into() }
    }
}

impl Default for ConcatAggregator {
    fn default() -> Self {
        Self::new("\n\n")
    }
}

#[async_trait::async_trait]
impl Aggregator for ConcatAggregator {
    async fn aggregate(&self, request_id: &str, responses: Vec<AgentResponse>) -> Result<AggregateResponse> {
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
                subtask_description: String::new(),
                duration: resp.duration,
                estimated_tokens: resp.estimated_tokens,
                cache_hit: resp.cache_hit,
                errors: resp.warnings.clone(),
            });
            contents.push(resp.content.clone());
        }

        Ok(AggregateResponse { request_id: request_id.to_string(), content: contents.join(&self.separator), structured_data: None, worker_results, total_tokens, total_duration, cache_stats })
    }
}
