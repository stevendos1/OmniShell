//! Minimal interactive wizard to generate agent configuration.

use std::io::{self, Write};

use crate::infrastructure::cli_adapter::{CliAgentConfig, EnvVarConfig, InputMode, OutputFormat};

const PLACEHOLDER: &str = "{PROMPT}";

/// Build a set of opinionated default agents using simple role-to-provider prompts.
pub fn run_wizard() -> io::Result<Vec<CliAgentConfig>> {
    println!("\n=== OmniShell Quickstart ===");
    println!("Configura en segundos qué proveedor usará cada rol del orquestador.\n");

    let backend = ask_provider("Backend", "codex")?;
    let frontend = ask_provider("Frontend", "codex")?;
    let tests = ask_provider("Tests", "open-code")?;
    let docs = ask_provider("Documentación", "claude")?;

    let agents = vec![
        agent_for_role("backend", "Backend", &backend),
        agent_for_role("frontend", "Frontend", &frontend),
        agent_for_role("tests", "Tests", &tests),
        agent_for_role("docs", "Documentación", &docs),
    ];

    println!("\nListo. Se generaron {} agentes:\n", agents.len());
    for agent in &agents {
        println!(
            "- {} -> {} [{}]",
            agent.display_name,
            agent.binary,
            agent.capabilities.join(", ")
        );
    }
    println!();

    Ok(agents)
}

fn ask_provider(role: &str, default: &str) -> io::Result<String> {
    print!("Proveedor para {role} (default: {default}): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn agent_for_role(id: &str, role: &str, provider: &str) -> CliAgentConfig {
    CliAgentConfig {
        id: format!("{provider}-{id}"),
        display_name: format!("{role} ({provider})"),
        binary: provider.to_string(),
        base_args: vec![PLACEHOLDER.to_string()],
        input_mode: InputMode::Arg,
        prompt_placeholder: Some(PLACEHOLDER.to_string()),
        output_format: OutputFormat::Text,
        json_content_path: None,
        timeout_seconds: 120,
        max_concurrency: 4,
        priority: 10,
        capabilities: capabilities_for_role(id),
        enabled: true,
        env_vars: default_env_vars(provider),
    }
}

fn capabilities_for_role(role_id: &str) -> Vec<String> {
    match role_id {
        "backend" => vec!["backend".into(), "code-generation".into()],
        "frontend" => vec!["frontend".into(), "ui".into(), "code-generation".into()],
        "tests" => vec!["tests".into(), "qa".into(), "code-review".into()],
        "docs" => vec!["docs".into(), "analysis".into(), "general".into()],
        _ => vec!["general".into()],
    }
}

fn default_env_vars(provider: &str) -> Vec<EnvVarConfig> {
    let key_hint = match provider {
        "codex" => Some("OPENAI_API_KEY"),
        "claude" => Some("ANTHROPIC_API_KEY"),
        "gemini" => Some("GEMINI_API_KEY"),
        _ => None,
    };
    key_hint
        .map(|name| {
            vec![EnvVarConfig {
                name: name.to_string(),
                value: format!("${name}"),
            }]
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_profile() {
        let a = agent_for_role("backend", "Backend", "codex");
        assert_eq!(a.binary, "codex");
        assert!(a.capabilities.contains(&"backend".to_string()));
        assert_eq!(a.input_mode, InputMode::Arg);
        assert_eq!(a.prompt_placeholder.as_deref(), Some(PLACEHOLDER));
    }

    #[test]
    fn test_provider_env_vars() {
        let vars = default_env_vars("claude");
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "ANTHROPIC_API_KEY");
        assert_eq!(vars[0].value, "$ANTHROPIC_API_KEY");
    }
}
