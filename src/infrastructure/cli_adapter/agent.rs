//! CliAgent — AiAgent trait implementation.

use std::collections::HashSet;
use std::process::Stdio;
use std::time::Duration;

use tracing::{debug, instrument};

use crate::domain::agent::*;
use crate::domain::error::{OrchestratorError, Result};

use super::parse;
use super::{CliAgentConfig, InputMode};

/// A CLI-based AI agent adapter.
pub struct CliAgent {
    config: CliAgentConfig,
    info: AgentInfo,
}

impl CliAgent {
    pub fn new(config: CliAgentConfig) -> Result<Self> {
        if config.input_mode == InputMode::Arg && config.prompt_placeholder.is_none() {
            return Err(OrchestratorError::InvalidConfig("input_mode 'arg' requires prompt_placeholder".into()));
        }
        let info = AgentInfo {
            id: config.id.clone(),
            display_name: config.display_name.clone(),
            capabilities: config.capabilities.iter().map(|c| AgentCapability::new(c.as_str())).collect::<HashSet<_>>(),
            max_concurrency: config.max_concurrency,
            default_timeout: Duration::from_secs(config.timeout_seconds),
            enabled: config.enabled,
            priority: config.priority,
        };
        Ok(Self { config, info })
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
        let args = parse::build_args(&self.config.base_args, self.config.input_mode, &request.user_prompt, self.config.prompt_placeholder.as_deref());
        debug!(binary = %self.config.binary, args = ?args, "spawning CLI");
        let mut cmd = tokio::process::Command::new(&self.config.binary);
        cmd.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());
        for env in &self.config.env_vars {
            let v = if env.value.starts_with('$') { std::env::var(&env.value[1..]).unwrap_or_default() } else { env.value.clone() };
            cmd.env(&env.name, v);
        }
        if self.config.input_mode == InputMode::Stdin {
            cmd.stdin(Stdio::piped());
        }
        let mut child =
            cmd.spawn().map_err(|e| OrchestratorError::AgentError { agent_id: self.info.id.clone(), message: format!("spawn '{}': {e}", self.config.binary), source: Some(Box::new(e)) })?;
        if self.config.input_mode == InputMode::Stdin {
            if let Some(stdin) = child.stdin.as_mut() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(request.user_prompt.as_bytes()).await.map_err(|e| OrchestratorError::agent(&self.info.id, format!("stdin: {e}")))?;
                drop(child.stdin.take());
            }
        }
        let output = child.wait_with_output().await.map_err(|e| OrchestratorError::agent(&self.info.id, format!("process: {e}")))?;
        let duration = start.elapsed();
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::agent(&self.info.id, format!("exit {:?}: {}", output.status.code(), stderr.chars().take(500).collect::<String>())));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let content = parse::parse_output(&stdout, self.config.output_format, &self.config.id, self.config.json_content_path.as_deref())?;
        Ok(AgentResponse {
            request_id: request.request_id,
            agent_id: self.info.id.clone(),
            content,
            structured_data: None,
            estimated_tokens: (stdout.len() as u64 + 3) / 4,
            duration,
            cache_hit: false,
            warnings: Vec::new(),
        })
    }

    async fn health_check(&self) -> Result<()> {
        let r = tokio::process::Command::new("which").arg(&self.config.binary).stdout(Stdio::null()).stderr(Stdio::null()).status().await;
        match r {
            Ok(s) if s.success() => Ok(()),
            Ok(_) => Err(OrchestratorError::agent(&self.info.id, format!("'{}' not in PATH", self.config.binary))),
            Err(e) => Err(OrchestratorError::agent(&self.info.id, format!("health check: {e}"))),
        }
    }
}
