//! Output parsing and argument building for CLI agents.

use crate::domain::error::{OrchestratorError, Result};

use super::{InputMode, OutputFormat};

/// Parse raw CLI output based on configured format.
pub(crate) fn parse_output(raw: &str, format: OutputFormat, agent_id: &str, json_content_path: Option<&str>) -> Result<String> {
    match format {
        OutputFormat::Text => Ok(raw.to_string()),
        OutputFormat::Json => parse_json_output(raw, agent_id, json_content_path),
        OutputFormat::Auto => parse_json_output(raw, agent_id, json_content_path).or_else(|_| Ok(raw.to_string())),
    }
}

fn parse_json_output(raw: &str, agent_id: &str, path: Option<&str>) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|e| OrchestratorError::ParseError { agent_id: agent_id.into(), message: format!("invalid JSON: {e}") })?;

    if let Some(path) = path {
        let mut current = &value;
        for key in path.split('.') {
            current = current.get(key).ok_or_else(|| OrchestratorError::ParseError { agent_id: agent_id.into(), message: format!("JSON path '{path}' not found at '{key}'") })?;
        }
        match current {
            serde_json::Value::String(s) => Ok(s.clone()),
            other => Ok(other.to_string()),
        }
    } else {
        Ok(raw.to_string())
    }
}

/// Build the argument list, substituting the prompt if needed.
pub(crate) fn build_args(base_args: &[String], input_mode: InputMode, prompt: &str, placeholder: Option<&str>) -> Vec<String> {
    if input_mode == InputMode::Arg {
        if let Some(ph) = placeholder {
            return base_args.iter().map(|a| a.replace(ph, prompt)).collect();
        }
    }
    base_args.to_vec()
}
