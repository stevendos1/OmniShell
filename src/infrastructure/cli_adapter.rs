//! # CLI Agent Adapter
//!
//! A fully configurable adapter that wraps any AI CLI tool
//! (Claude, Codex, Gemini, etc.) as an `AiAgent`.
//!
//! **No flags or commands are hardcoded.** Everything is configured
//! through `CliAgentConfig`.

use std::collections::HashSet;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::domain::agent::*;
use crate::domain::error::{OrchestratorError, Result};

/// Configuration for a CLI-based AI agent.
///
/// All fields are user-configurable. No CLI-specific flags are assumed.
///
/// # Example (TOML)
/// ```toml
/// [agent]
/// id = "claude-cli"
/// display_name = "Claude CLI"
/// binary = "claude"
/// base_args = ["--output-format", "json"]
/// input_mode = "stdin"
/// output_format = "json"
/// timeout_seconds = 120
/// max_concurrency = 2
/// priority = 10
/// capabilities = ["code-generation", "summarization"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliAgentConfig {
    /// Unique agent identifier.
    pub id: String,
    /// Human-readable name.
    pub display_name: String,
    /// Path to the CLI binary.
    pub binary: String,
    /// Base arguments always passed to the CLI.
    pub base_args: Vec<String>,
    /// How to pass input: "stdin", "arg", or "file".
    pub input_mode: InputMode,
    /// Placeholder in `base_args` that gets replaced with the prompt.
    /// Only used when `input_mode` is `Arg`. E.g. `"{PROMPT}"`.
    pub prompt_placeholder: Option<String>,
    /// Expected output format: "json", "text", or "auto".
    pub output_format: OutputFormat,
    /// JSON path to extract the response content (e.g. `"result.text"`).
    /// Only used when `output_format` is `Json`.
    pub json_content_path: Option<String>,
    /// Timeout in seconds.
    pub timeout_seconds: u64,
    /// Maximum concurrent requests.
    pub max_concurrency: usize,
    /// Priority weight for routing.
    pub priority: u32,
    /// Declared capabilities.
    pub capabilities: Vec<String>,
    /// Whether this agent is enabled.
    pub enabled: bool,
    /// Environment variables to set for the CLI process.
    pub env_vars: Vec<EnvVarConfig>,
}

/// How the prompt is passed to the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMode {
    /// Pass the prompt via stdin.
    Stdin,
    /// Pass the prompt as an argument (using `prompt_placeholder`).
    Arg,
    /// Write the prompt to a temp file and pass the path as an argument.
    File,
}

/// Expected output format from the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Raw text output.
    Text,
    /// JSON output (parsed using `json_content_path`).
    Json,
    /// Try JSON first, fall back to text.
    Auto,
}

/// An environment variable to set for the CLI process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarConfig {
    /// Variable name.
    pub name: String,
    /// Variable value. If starts with `$`, it references another env var.
    pub value: String,
}

/// A CLI-based AI agent adapter.
///
/// Implements `AiAgent` by spawning the configured CLI binary
/// as a subprocess and parsing its output.
pub struct CliAgent {
    config: CliAgentConfig,
    info: AgentInfo,
}

impl CliAgent {
    /// Create a new CLI agent from configuration.
    ///
    /// # Errors
    /// Returns `InvalidConfig` if the configuration is inconsistent
    /// (e.g. `Arg` input mode without a `prompt_placeholder`).
    pub fn new(config: CliAgentConfig) -> Result<Self> {
        if config.input_mode == InputMode::Arg && config.prompt_placeholder.is_none() {
            return Err(OrchestratorError::InvalidConfig(
                "input_mode 'arg' requires a prompt_placeholder".into(),
            ));
        }

        let info = AgentInfo {
            id: config.id.clone(),
            display_name: config.display_name.clone(),
            capabilities: config
                .capabilities
                .iter()
                .map(|c| AgentCapability::new(c.as_str()))
                .collect::<HashSet<_>>(),
            max_concurrency: config.max_concurrency,
            default_timeout: Duration::from_secs(config.timeout_seconds),
            enabled: config.enabled,
            priority: config.priority,
        };

        Ok(Self { config, info })
    }

    /// Parse the CLI output into the response content string.
    fn parse_output(&self, raw: &str) -> Result<String> {
        match self.config.output_format {
            OutputFormat::Text => Ok(raw.to_string()),
            OutputFormat::Json => self.parse_json_output(raw),
            OutputFormat::Auto => {
                // Try JSON first, fall back to text.
                self.parse_json_output(raw).or_else(|_| Ok(raw.to_string()))
            }
        }
    }

    /// Extract content from JSON output using the configured path.
    fn parse_json_output(&self, raw: &str) -> Result<String> {
        let value: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| OrchestratorError::ParseError {
                agent_id: self.config.id.clone(),
                message: format!("invalid JSON: {e}"),
            })?;

        if let Some(path) = &self.config.json_content_path {
            let mut current = &value;
            for key in path.split('.') {
                current = current
                    .get(key)
                    .ok_or_else(|| OrchestratorError::ParseError {
                        agent_id: self.config.id.clone(),
                        message: format!("JSON path '{path}' not found at key '{key}'"),
                    })?;
            }
            match current {
                serde_json::Value::String(s) => Ok(s.clone()),
                other => Ok(other.to_string()),
            }
        } else {
            // Return the entire JSON as a string.
            Ok(raw.to_string())
        }
    }

    /// Build the argument list, substituting the prompt if needed.
    fn build_args(&self, prompt: &str) -> Vec<String> {
        if self.config.input_mode == InputMode::Arg {
            if let Some(placeholder) = &self.config.prompt_placeholder {
                return self
                    .config
                    .base_args
                    .iter()
                    .map(|arg| arg.replace(placeholder.as_str(), prompt))
                    .collect();
            }
        }
        self.config.base_args.clone()
    }
}

#[async_trait::async_trait]
impl AiAgent for CliAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    #[instrument(skip(self, request), fields(agent_id = %self.info.id))]
    async fn execute(&self, request: AgentRequest) -> Result<AgentResponse> {
        let start = std::time::Instant::now();

        let args = self.build_args(&request.user_prompt);

        debug!(
            binary = %self.config.binary,
            args = ?args,
            "spawning CLI process"
        );

        let mut cmd = tokio::process::Command::new(&self.config.binary);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables.
        for env in &self.config.env_vars {
            let value = if env.value.starts_with('$') {
                std::env::var(&env.value[1..]).unwrap_or_default()
            } else {
                env.value.clone()
            };
            cmd.env(&env.name, value);
        }

        // Pass prompt via stdin if applicable.
        if self.config.input_mode == InputMode::Stdin {
            cmd.stdin(Stdio::piped());
        }

        let mut child = cmd.spawn().map_err(|e| OrchestratorError::AgentError {
            agent_id: self.info.id.clone(),
            message: format!("failed to spawn '{}': {e}", self.config.binary),
            source: Some(Box::new(e)),
        })?;

        // Write prompt to stdin.
        if self.config.input_mode == InputMode::Stdin {
            if let Some(stdin) = child.stdin.as_mut() {
                use tokio::io::AsyncWriteExt;
                let prompt_bytes = request.user_prompt.as_bytes();
                stdin.write_all(prompt_bytes).await.map_err(|e| {
                    OrchestratorError::agent(&self.info.id, format!("failed to write stdin: {e}"))
                })?;
                // Drop stdin to signal EOF.
                drop(child.stdin.take());
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| OrchestratorError::agent(&self.info.id, format!("process error: {e}")))?;

        let duration = start.elapsed();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::agent(
                &self.info.id,
                format!(
                    "CLI exited with code {:?}: {}",
                    output.status.code(),
                    stderr.chars().take(500).collect::<String>()
                ),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let content = self.parse_output(&stdout)?;

        // Estimate tokens (rough heuristic).
        let estimated_tokens = (content.len() as u64 + 3) / 4;

        Ok(AgentResponse {
            request_id: request.request_id,
            agent_id: self.info.id.clone(),
            content,
            structured_data: None,
            estimated_tokens,
            duration,
            cache_hit: false,
            warnings: Vec::new(),
        })
    }

    async fn health_check(&self) -> Result<()> {
        // Check that the binary exists and is executable.
        let result = tokio::process::Command::new("which")
            .arg(&self.config.binary)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        match result {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err(OrchestratorError::agent(
                &self.info.id,
                format!("binary '{}' not found in PATH", self.config.binary),
            )),
            Err(e) => Err(OrchestratorError::agent(
                &self.info.id,
                format!("health check failed: {e}"),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(input_mode: InputMode, output_format: OutputFormat) -> CliAgentConfig {
        CliAgentConfig {
            id: "test-agent".to_string(),
            display_name: "Test Agent".to_string(),
            binary: "echo".to_string(),
            base_args: vec!["hello".to_string()],
            input_mode,
            prompt_placeholder: Some("{PROMPT}".to_string()),
            output_format,
            json_content_path: Some("result".to_string()),
            timeout_seconds: 30,
            max_concurrency: 1,
            priority: 1,
            capabilities: vec!["test".to_string()],
            enabled: true,
            env_vars: Vec::new(),
        }
    }

    #[test]
    fn test_cli_agent_creation() {
        let config = make_config(InputMode::Stdin, OutputFormat::Text);
        let agent = CliAgent::new(config).expect("should create agent");
        assert_eq!(agent.info().id, "test-agent");
        assert!(agent.info().has_capability("test"));
    }

    #[test]
    fn test_arg_mode_requires_placeholder() {
        let mut config = make_config(InputMode::Arg, OutputFormat::Text);
        config.prompt_placeholder = None;
        let result = CliAgent::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_text_output() {
        let config = make_config(InputMode::Stdin, OutputFormat::Text);
        let agent = CliAgent::new(config).expect("should create agent");
        let result = agent.parse_output("hello world").expect("should parse");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_parse_json_output() {
        let config = make_config(InputMode::Stdin, OutputFormat::Json);
        let agent = CliAgent::new(config).expect("should create agent");
        let json = r#"{"result": "extracted content"}"#;
        let result = agent.parse_output(json).expect("should parse");
        assert_eq!(result, "extracted content");
    }

    #[test]
    fn test_parse_auto_fallback() {
        let config = make_config(InputMode::Stdin, OutputFormat::Auto);
        let agent = CliAgent::new(config).expect("should create agent");
        let result = agent.parse_output("not json at all").expect("should parse");
        assert_eq!(result, "not json at all");
    }

    #[test]
    fn test_build_args_with_placeholder() {
        let mut config = make_config(InputMode::Arg, OutputFormat::Text);
        config.base_args = vec!["--prompt".to_string(), "{PROMPT}".to_string()];
        let agent = CliAgent::new(config).expect("should create agent");
        let args = agent.build_args("hello world");
        assert_eq!(args, vec!["--prompt", "hello world"]);
    }
}
