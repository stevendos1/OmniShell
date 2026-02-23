//! # OmniShell Orchestrator — Entry Point
//!
//! Wires all layers together and runs the CLI.

use std::sync::Arc;

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use omnishell_orchestrator::application::aggregator::ConcatAggregator;
use omnishell_orchestrator::application::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use omnishell_orchestrator::application::context_service::InMemoryContextManager;
use omnishell_orchestrator::application::orchestrator_service::OrchestratorServiceBuilder;
use omnishell_orchestrator::application::planner::DeterministicPlanner;
use omnishell_orchestrator::application::router::CapabilityRouter;
use omnishell_orchestrator::domain::cache::Cache;
use omnishell_orchestrator::domain::context::ContextManager;
use omnishell_orchestrator::domain::error::OrchestratorError;
use omnishell_orchestrator::domain::orchestrator::{
    Aggregator, Orchestrator, Planner, UserRequest,
};
use omnishell_orchestrator::domain::policy::PolicyGuard;
use omnishell_orchestrator::domain::token::{SimpleTokenCounter, TokenCounter};
use omnishell_orchestrator::infrastructure::cli_adapter::CliAgent;
use omnishell_orchestrator::infrastructure::config::OrchestratorConfig;
use omnishell_orchestrator::infrastructure::lru_cache::LruCacheImpl;
use omnishell_orchestrator::infrastructure::policy_guard::DefaultPolicyGuard;
use omnishell_orchestrator::interface::cli::{self, Command, OutputFormat};

#[tokio::main]
async fn main() {
    let cli = cli::parse_args();

    // Initialize tracing.
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_target(true)
        .init();

    info!("OmniShell Orchestrator starting");

    // Load config.
    let config = if cli.config.exists() {
        match OrchestratorConfig::load_from_file(&cli.config) {
            Ok(c) => c,
            Err(e) => {
                error!("failed to load config: {e}");
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        info!("config file not found, using defaults");
        OrchestratorConfig::default()
    };

    // Execute command.
    if let Err(e) = run(cli, config).await {
        error!("command failed: {e}");
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: cli::Cli, config: OrchestratorConfig) -> Result<(), OrchestratorError> {
    match cli.command {
        Command::Config => {
            let serialized = toml::to_string_pretty(&config).map_err(|e| {
                OrchestratorError::SerializationError(format!("failed to serialize config: {e}"))
            })?;
            println!("{serialized}");
            Ok(())
        }
        Command::Agents | Command::Health | Command::Run { .. } => {
            // Build the orchestrator.
            let token_counter: Arc<dyn TokenCounter> = Arc::new(SimpleTokenCounter);
            let planner: Arc<dyn Planner> = Arc::new(DeterministicPlanner::new(
                token_counter.clone(),
                "code-generation".to_string(),
            ));
            let router = Arc::new(CapabilityRouter::new());
            let aggregator: Arc<dyn Aggregator> = Arc::new(ConcatAggregator::default());
            let context_manager: Arc<dyn ContextManager> = Arc::new(InMemoryContextManager::new(
                config.context.clone(),
                token_counter.clone(),
            ));
            let cache: Arc<dyn Cache> = Arc::new(LruCacheImpl::new(config.cache.clone()));
            let policy_guard: Arc<dyn PolicyGuard> =
                Arc::new(DefaultPolicyGuard::new(config.policy.clone())?);

            // Register agents from config.
            for agent_config in &config.agents {
                match CliAgent::new(agent_config.clone()) {
                    Ok(agent) => {
                        router.register(Arc::new(agent)).await;
                        info!(agent_id = %agent_config.id, "agent registered");
                    }
                    Err(e) => {
                        error!(agent_id = %agent_config.id, "failed to create agent: {e}");
                    }
                }
            }

            let orchestrator = OrchestratorServiceBuilder::new()
                .planner(planner)
                .router(router)
                .aggregator(aggregator)
                .context_manager(context_manager)
                .cache(cache)
                .token_counter(token_counter)
                .policy_guard(policy_guard)
                .circuit_breaker(CircuitBreaker::new(CircuitBreakerConfig::default()))
                .retry_policy(config.retry_policy.clone())
                .timeout_policy(config.timeout_policy.clone())
                .max_concurrency(config.max_concurrency)
                .config_version(config.config_version.clone())
                .build()?;

            match cli.command {
                Command::Run {
                    message,
                    session,
                    capability,
                    max_tokens,
                } => {
                    let request = UserRequest {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session,
                        message,
                        preferred_capability: capability,
                        max_tokens,
                    };

                    let response = orchestrator.process(request).await?;

                    match cli.format {
                        OutputFormat::Text => {
                            println!("{}", response.content);
                            if !response.worker_results.is_empty() {
                                eprintln!(
                                    "\n--- Stats: {} tokens, {:?}, cache {}/{} ---",
                                    response.total_tokens,
                                    response.total_duration,
                                    response.cache_stats.hits,
                                    response.cache_stats.hits + response.cache_stats.misses,
                                );
                            }
                        }
                        OutputFormat::Json => {
                            let json = serde_json::to_string_pretty(&response).map_err(|e| {
                                OrchestratorError::SerializationError(e.to_string())
                            })?;
                            println!("{json}");
                        }
                    }
                    Ok(())
                }
                Command::Agents => {
                    let agents = orchestrator.active_agents().await?;
                    if agents.is_empty() {
                        println!("No active agents configured.");
                    } else {
                        println!("Active agents:");
                        for id in agents {
                            println!("  - {id}");
                        }
                    }
                    Ok(())
                }
                Command::Health => {
                    let results = orchestrator.health_check().await?;
                    for (id, healthy) in results {
                        let status = if healthy { "OK" } else { "FAIL" };
                        println!("  {id}: {status}");
                    }
                    Ok(())
                }
                Command::Config => unreachable!(),
            }
        }
    }
}
