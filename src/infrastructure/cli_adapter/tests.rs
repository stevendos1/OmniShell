//! CLI adapter unit tests.

use super::*;
use crate::domain::agent::AiAgent;

fn make_config(im: InputMode, of: OutputFormat) -> CliAgentConfig {
    CliAgentConfig {
        id: "test-agent".into(),
        display_name: "Test".into(),
        binary: "echo".into(),
        base_args: vec!["hello".into()],
        input_mode: im,
        prompt_placeholder: Some("{PROMPT}".into()),
        output_format: of,
        json_content_path: Some("result".into()),
        timeout_seconds: 30,
        max_concurrency: 1,
        priority: 1,
        capabilities: vec!["test".into()],
        enabled: true,
        env_vars: Vec::new(),
    }
}

#[test]
fn test_creation() {
    let agent = CliAgent::new(make_config(InputMode::Stdin, OutputFormat::Text)).unwrap();
    assert_eq!(agent.info().id, "test-agent");
    assert!(agent.info().has_capability("test"));
}

#[test]
fn test_arg_needs_placeholder() {
    let mut c = make_config(InputMode::Arg, OutputFormat::Text);
    c.prompt_placeholder = None;
    assert!(CliAgent::new(c).is_err());
}

#[test]
fn test_parse_text() {
    let r = parse::parse_output("hello", OutputFormat::Text, "a", None).unwrap();
    assert_eq!(r, "hello");
}

#[test]
fn test_parse_json() {
    let r = parse::parse_output(r#"{"result": "extracted"}"#, OutputFormat::Json, "a", Some("result")).unwrap();
    assert_eq!(r, "extracted");
}

#[test]
fn test_parse_auto_fallback() {
    let r = parse::parse_output("not json", OutputFormat::Auto, "a", Some("x")).unwrap();
    assert_eq!(r, "not json");
}

#[test]
fn test_build_args_placeholder() {
    let args = parse::build_args(&["--prompt".into(), "{PROMPT}".into()], InputMode::Arg, "hi", Some("{PROMPT}"));
    assert_eq!(args, vec!["--prompt", "hi"]);
}
