//! Config unit tests.

use super::*;

#[test]
fn test_default() {
    let c = OrchestratorConfig::default();
    assert_eq!(c.max_concurrency, 10);
    assert!(c.agents.is_empty());
    assert!(c.cache.enabled);
}

#[test]
fn test_minimal_toml() {
    let c = OrchestratorConfig::from_toml(
        r#"config_version = "v2"
max_concurrency = 4"#,
    )
    .unwrap();
    assert_eq!(c.config_version, "v2");
    assert_eq!(c.max_concurrency, 4);
}

#[test]
fn test_toml_with_agent() {
    let c = OrchestratorConfig::from_toml(
        r#"
config_version = "v1"
[[agents]]
id = "test"
display_name = "Test"
binary = "/usr/bin/echo"
base_args = ["hello"]
input_mode = "stdin"
output_format = "text"
timeout_seconds = 30
max_concurrency = 2
priority = 5
capabilities = ["code-generation"]
enabled = true
env_vars = []
"#,
    )
    .unwrap();
    assert_eq!(c.agents.len(), 1);
    assert_eq!(c.agents[0].id, "test");
}

#[test]
fn test_yaml() {
    let c = OrchestratorConfig::from_yaml("config_version: v1\nmax_concurrency: 6\nagents: []\n").unwrap();
    assert_eq!(c.max_concurrency, 6);
}

#[test]
fn test_invalid_toml() {
    assert!(OrchestratorConfig::from_toml("{{invalid").is_err());
}
