//! # CLI Interface
//!
//! Defines the command-line interface using `clap` derive macros.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// OmniShell Orchestrator — multi-agent AI task orchestrator.
///
/// Routes tasks to AI CLI agents (Claude, Codex, Gemini, etc.)
/// with caching, token budgets, and context management.
#[derive(Debug, Parser)]
#[command(
    name = "omnishell",
    version,
    about = "Multi-agent AI orchestrator",
    long_about = "OmniShell Orchestrator: routes tasks to AI CLI agents with \
                  caching, token budgets, context management, and secure tool execution."
)]
pub struct Cli {
    /// Path to the configuration file (TOML or YAML).
    #[arg(short, long, default_value = "config/orchestrator.toml")]
    pub config: PathBuf,

    /// Logging verbosity (repeat for more: -v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Output format.
    #[arg(long, default_value = "text")]
    pub format: OutputFormat,

    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Plain text.
    Text,
    /// JSON.
    Json,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Send a task to the orchestrator.
    Run {
        /// The task message / prompt.
        #[arg()]
        message: String,

        /// Session ID (for context continuity).
        #[arg(short, long, default_value = "default")]
        session: String,

        /// Preferred agent capability.
        #[arg(long)]
        capability: Option<String>,

        /// Maximum tokens to spend.
        #[arg(long)]
        max_tokens: Option<u64>,
    },

    /// List active agents and their capabilities.
    Agents,

    /// Run health checks on all registered agents.
    Health,

    /// Show the current configuration.
    Config,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_run() {
        let cli = Cli::parse_from(["omnishell", "--config", "test.toml", "run", "Hello world"]);
        assert_eq!(cli.config, PathBuf::from("test.toml"));
        match cli.command {
            Command::Run { ref message, .. } => {
                assert_eq!(message, "Hello world");
            }
            _ => panic!("expected Run command"),
        }
    }

    #[test]
    fn test_cli_parse_agents() {
        let cli = Cli::parse_from(["omnishell", "agents"]);
        assert!(matches!(cli.command, Command::Agents));
    }

    #[test]
    fn test_cli_parse_health() {
        let cli = Cli::parse_from(["omnishell", "health"]);
        assert!(matches!(cli.command, Command::Health));
    }
}
