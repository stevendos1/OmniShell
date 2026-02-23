//! Circuit breaker implementation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::domain::error::{OrchestratorError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
struct AgentCircuit {
    failures: u32,
    opened_at: Option<Instant>,
    state: CircuitState,
}
impl Default for AgentCircuit {
    fn default() -> Self {
        Self {
            failures: 0,
            opened_at: None,
            state: CircuitState::Closed,
        }
    }
}

/// Configuration for the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
}
impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker tracking state for all agents.
///
/// # Example
/// ```
/// use omnishell_orchestrator::application::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
/// let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
/// ```
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    circuits: Arc<RwLock<HashMap<String, AgentCircuit>>>,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            circuits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check(&self, agent_id: &str) -> Result<()> {
        let mut circuits = self.circuits.write().await;
        let c = circuits.entry(agent_id.to_string()).or_default();
        match c.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                if let Some(at) = c.opened_at {
                    if at.elapsed() >= self.config.recovery_timeout {
                        info!(agent_id, "circuit → half-open");
                        c.state = CircuitState::HalfOpen;
                        Ok(())
                    } else {
                        let remaining = self
                            .config
                            .recovery_timeout
                            .checked_sub(at.elapsed())
                            .unwrap_or_default();
                        Err(OrchestratorError::CircuitBreakerOpen {
                            agent_id: agent_id.into(),
                            reason: format!("{} failures; retry in {remaining:?}", c.failures),
                        })
                    }
                } else {
                    c.state = CircuitState::Closed;
                    Ok(())
                }
            }
            CircuitState::HalfOpen => Ok(()),
        }
    }

    pub async fn record_success(&self, agent_id: &str) {
        let mut circuits = self.circuits.write().await;
        let c = circuits.entry(agent_id.to_string()).or_default();
        if c.state != CircuitState::Closed {
            info!(agent_id, "circuit closing (recovered)");
        }
        c.failures = 0;
        c.opened_at = None;
        c.state = CircuitState::Closed;
    }

    pub async fn record_failure(&self, agent_id: &str) {
        let mut circuits = self.circuits.write().await;
        let c = circuits.entry(agent_id.to_string()).or_default();
        c.failures += 1;
        if c.failures >= self.config.failure_threshold && c.state != CircuitState::Open {
            warn!(agent_id, failures = c.failures, "circuit opening");
            c.state = CircuitState::Open;
            c.opened_at = Some(Instant::now());
        }
    }

    pub async fn reset(&self, agent_id: &str) {
        self.circuits.write().await.remove(agent_id);
    }
}
