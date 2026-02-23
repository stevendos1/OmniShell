//! CLI unit tests.

use super::*;

#[test]
fn test_parse_run() {
    let cli = Cli::parse_from(["omnishell", "--config", "test.toml", "run", "Hello"]);
    assert_eq!(cli.config, PathBuf::from("test.toml"));
    match cli.command {
        Command::Run { ref message, .. } => assert_eq!(message, "Hello"),
        _ => panic!("expected Run"),
    }
}

#[test]
fn test_parse_agents() {
    let cli = Cli::parse_from(["omnishell", "agents"]);
    assert!(matches!(cli.command, Command::Agents));
}

#[test]
fn test_parse_health() {
    let cli = Cli::parse_from(["omnishell", "health"]);
    assert!(matches!(cli.command, Command::Health));
}
