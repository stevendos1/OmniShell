//! CLI interface using clap.

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// OmniShell Orchestrator — multi-agent AI task orchestrator.
#[derive(Debug, Parser)]
#[command(name = "omnishell", version, about = "Multi-agent AI orchestrator")]
pub struct Cli {
    #[arg(short, long, default_value = "config/orchestrator.toml")]
    pub config: PathBuf,
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    #[arg(long, default_value = "text")]
    pub format: OutputFormat,
    #[command(subcommand)]
    pub command: Command,
}

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Send a task to the orchestrator.
    Run {
        #[arg()]
        message: String,
        #[arg(short, long, default_value = "default")]
        session: String,
        #[arg(long)]
        capability: Option<String>,
        #[arg(long)]
        max_tokens: Option<u64>,
    },
    /// List active agents.
    Agents,
    /// Run health checks.
    Health,
    /// Show current configuration.
    Config,
    /// Interactive setup to generate role-based agent mapping.
    Setup {
        /// Write generated config to this output file.
        #[arg(long, default_value = "config/orchestrator-quickstart.toml")]
        output: PathBuf,
    },
}

/// Parse CLI arguments.
///
/// # Example
/// ```no_run
/// use omnishell_orchestrator::interface::cli::parse_args;
/// let cli = parse_args();
/// ```
pub fn parse_args() -> Cli {
    Cli::parse()
}
