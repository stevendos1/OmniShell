//! OmniShell Orchestrator — Entry Point.

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use omnishell_orchestrator::infrastructure::config::OrchestratorConfig;
use omnishell_orchestrator::interface::cli;

mod runner;

#[tokio::main]
async fn main() {
    let cli = cli::parse_args();

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

    let config = if cli.config.exists() {
        match OrchestratorConfig::load_from_file(&cli.config) {
            Ok(c) => c,
            Err(e) => {
                error!("config: {e}");
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        info!("config not found, using defaults");
        OrchestratorConfig::default()
    };

    if let Err(e) = runner::run(cli, config).await {
        error!("command failed: {e}");
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
