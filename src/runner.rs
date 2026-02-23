//! Command dispatch and orchestrator setup.

use std::sync::Arc;
use tracing::{error, info};

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
use omnishell_orchestrator::interface::quickstart;

pub async fn run(cli: cli::Cli, config: OrchestratorConfig) -> Result<(), OrchestratorError> {
    match cli.command {
        Command::Config => {
            let s = toml::to_string_pretty(&config)
                .map_err(|e| OrchestratorError::SerializationError(format!("{e}")))?;
            println!("{s}");
            Ok(())
        }
        Command::Setup { output } => {
            let mut generated = config;
            generated.agents = quickstart::run_wizard().map_err(OrchestratorError::from)?;
            let body = toml::to_string_pretty(&generated)
                .map_err(|e| OrchestratorError::SerializationError(format!("{e}")))?;
            std::fs::write(&output, body).map_err(OrchestratorError::from)?;
            println!("Configuración guardada en {}", output.display());
            println!("Tip: ejecuta `omnishell -c {} agents`", output.display());
            Ok(())
        }
        _ => run_orchestrator(cli, config).await,
    }
}

async fn run_orchestrator(
    cli: cli::Cli,
    config: OrchestratorConfig,
) -> Result<(), OrchestratorError> {
    let tc: Arc<dyn TokenCounter> = Arc::new(SimpleTokenCounter);
    let planner: Arc<dyn Planner> = Arc::new(DeterministicPlanner::new(
        tc.clone(),
        "code-generation".into(),
    ));
    let router = Arc::new(CapabilityRouter::new());
    let agg: Arc<dyn Aggregator> = Arc::new(ConcatAggregator::default());
    let ctx: Arc<dyn ContextManager> = Arc::new(InMemoryContextManager::new(
        config.context.clone(),
        tc.clone(),
    ));
    let cache: Arc<dyn Cache> = Arc::new(LruCacheImpl::new(config.cache.clone()));
    let pg: Arc<dyn PolicyGuard> = Arc::new(DefaultPolicyGuard::new(config.policy.clone())?);
    for ac in &config.agents {
        match CliAgent::new(ac.clone()) {
            Ok(a) => {
                router.register(Arc::new(a)).await;
                info!(agent_id = %ac.id, "registered");
            }
            Err(e) => {
                error!(agent_id = %ac.id, "failed: {e}");
            }
        }
    }
    let orch = OrchestratorServiceBuilder::new()
        .planner(planner)
        .router(router)
        .aggregator(agg)
        .context_manager(ctx)
        .cache(cache)
        .token_counter(tc)
        .policy_guard(pg)
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
            let req = UserRequest {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session,
                message,
                preferred_capability: capability,
                max_tokens,
            };
            let resp = orch.process(req).await?;
            match cli.format {
                OutputFormat::Text => {
                    println!("{}", resp.content);
                    if !resp.worker_results.is_empty() {
                        eprintln!(
                            "\n--- {} tokens, {:?}, cache {}/{} ---",
                            resp.total_tokens,
                            resp.total_duration,
                            resp.cache_stats.hits,
                            resp.cache_stats.hits + resp.cache_stats.misses
                        );
                    }
                }
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&resp)
                            .map_err(|e| OrchestratorError::SerializationError(e.to_string()))?
                    );
                }
            }
            Ok(())
        }
        Command::Agents => {
            let agents = orch.active_agents().await?;
            if agents.is_empty() {
                println!("No active agents.");
            } else {
                println!("Active agents:");
                for id in agents {
                    println!("  - {id}");
                }
            }
            Ok(())
        }
        Command::Health => {
            for (id, ok) in orch.health_check().await? {
                println!("  {id}: {}", if ok { "OK" } else { "FAIL" });
            }
            Ok(())
        }
        Command::Config | Command::Setup { .. } => unreachable!(),
    }
}
