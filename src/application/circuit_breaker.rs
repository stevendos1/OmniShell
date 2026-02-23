//! # Circuit Breaker
//!
//! A per-agent circuit breaker that pauses dispatching when
//! consecutive failures exceed a threshold.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::domain::error::{OrchestratorError, Result};

/// Circuit breaker state for a single agent.
#[derive(Debug, Clone)]
struct AgentCircuit {
    /// Consecutive failure count.
    failures: u32,
    /// When the circuit was opened (if open).
    opened_at: Option<Instant>,
    /// Current state.
    state: CircuitState,
}

/// Possible states of a circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    /// Normal operation.
    Closed,
    /// Too many failures; requests are rejected.
    Open,
    /// Allowing a single probe request to test recovery.
    HalfOpen,
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
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// How long the circuit stays open before moving to half-open.
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
/// Thread-safe: uses an `Arc<RwLock<...>>` internally.
///
/// # Usage
/// ```
/// use omnishell_orchestrator::application::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
///
/// let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
/// // cb.check("agent-1") -> Ok(())   (circuit closed by default)
/// // cb.record_failure("agent-1") -> increments failure count
/// // cb.record_success("agent-1") -> resets failures
/// ```
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    circuits: Arc<RwLock<HashMap<String, AgentCircuit>>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            circuits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check whether requests to the given agent are allowed.
    ///
    /// # Errors
    /// Returns `OrchestratorError::CircuitBreakerOpen` if the circuit is open.
    pub async fn check(&self, agent_id: &str) -> Result<()> {
        let mut circuits = self.circuits.write().await;
        let circuit = circuits.entry(agent_id.to_string()).or_default();

        match circuit.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if recovery timeout has elapsed.
                if let Some(opened_at) = circuit.opened_at {
                    if opened_at.elapsed() >= self.config.recovery_timeout {
                        info!(agent_id, "circuit breaker moving to half-open");
                        circuit.state = CircuitState::HalfOpen;
                        Ok(())
                    } else {
                        Err(OrchestratorError::CircuitBreakerOpen {
                            agent_id: agent_id.to_string(),
                            reason: format!(
                                "{} consecutive failures; retry after {:?}",
                                circuit.failures,
                                self.config
                                    .recovery_timeout
                                    .checked_sub(opened_at.elapsed())
                                    .unwrap_or_default()
                            ),
                        })
                    }
                } else {
                    // Should not happen, but fail safe.
                    circuit.state = CircuitState::Closed;
                    Ok(())
                }
            }
            CircuitState::HalfOpen => {
                // Allow one probe request.
                Ok(())
            }
        }
    }

    /// Record a successful request, resetting the circuit.
    pub async fn record_success(&self, agent_id: &str) {
        let mut circuits = self.circuits.write().await;
        let circuit = circuits.entry(agent_id.to_string()).or_default();
        if circuit.state != CircuitState::Closed {
            info!(agent_id, "circuit breaker closing (recovered)");
        }
        circuit.failures = 0;
        circuit.opened_at = None;
        circuit.state = CircuitState::Closed;
    }

    /// Record a failed request, potentially opening the circuit.
    pub async fn record_failure(&self, agent_id: &str) {
        let mut circuits = self.circuits.write().await;
        let circuit = circuits.entry(agent_id.to_string()).or_default();
        circuit.failures += 1;

        if circuit.failures >= self.config.failure_threshold {
            if circuit.state != CircuitState::Open {
                warn!(
                    agent_id,
                    failures = circuit.failures,
                    "circuit breaker opening"
                );
                circuit.state = CircuitState::Open;
                circuit.opened_at = Some(Instant::now());
            }
        }
    }

    /// Reset the circuit for a specific agent.
    pub async fn reset(&self, agent_id: &str) {
        let mut circuits = self.circuits.write().await;
        circuits.remove(agent_id);
    }
}
