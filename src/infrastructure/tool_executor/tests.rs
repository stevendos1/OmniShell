//! Tool executor unit tests.

use super::*;
use crate::domain::tool::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

fn make_config(allowed: &[&str], denied: &[&str]) -> ToolExecutorConfig {
    ToolExecutorConfig {
        enabled: true,
        allowed_commands: allowed
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<_>>(),
        denied_commands: denied.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
        working_dir: PathBuf::from("/tmp"),
        timeout: Duration::from_secs(5),
        max_stdout_bytes: 1024,
        max_stderr_bytes: 256,
        dry_run: false,
    }
}

#[test]
fn test_is_allowed() {
    let e = SecureToolExecutor::new(make_config(&["ls", "cat"], &["rm"]));
    assert!(e.is_allowed("ls"));
    assert!(e.is_allowed("cat"));
    assert!(!e.is_allowed("rm"));
    assert!(!e.is_allowed("wget"));
}

#[test]
fn test_denylist_overrides() {
    assert!(!SecureToolExecutor::new(make_config(&["rm"], &["rm"])).is_allowed("rm"));
}

#[test]
fn test_sanitize_clean() {
    assert!(SecureToolExecutor::sanitize_arg("hello").is_ok());
    assert!(SecureToolExecutor::sanitize_arg("-la").is_ok());
    assert!(SecureToolExecutor::sanitize_arg("/tmp/file.txt").is_ok());
}

#[test]
fn test_sanitize_dangerous() {
    assert!(SecureToolExecutor::sanitize_arg("hello; rm -rf /").is_err());
    assert!(SecureToolExecutor::sanitize_arg("$(whoami)").is_err());
    assert!(SecureToolExecutor::sanitize_arg("hello | cat").is_err());
    assert!(SecureToolExecutor::sanitize_arg("`id`").is_err());
}

#[tokio::test]
async fn test_disabled() {
    let mut c = make_config(&["ls"], &[]);
    c.enabled = false;
    assert!(SecureToolExecutor::new(c)
        .execute(ToolRequest {
            command: "ls".into(),
            args: vec![],
            working_dir: None,
            timeout: None
        })
        .await
        .is_err());
}

#[tokio::test]
async fn test_dry_run() {
    let mut c = make_config(&["ls"], &[]);
    c.dry_run = true;
    let r = SecureToolExecutor::new(c)
        .execute(ToolRequest {
            command: "ls".into(),
            args: vec!["-la".into()],
            working_dir: None,
            timeout: None,
        })
        .await
        .unwrap();
    assert!(r.dry_run);
    assert!(r.stdout.contains("[dry-run]"));
}

#[tokio::test]
async fn test_denied() {
    assert!(SecureToolExecutor::new(make_config(&["ls"], &["rm"]))
        .execute(ToolRequest {
            command: "rm".into(),
            args: vec!["-rf".into()],
            working_dir: None,
            timeout: None
        })
        .await
        .is_err());
}

#[tokio::test]
async fn test_execute_ls() {
    let r = SecureToolExecutor::new(make_config(&["ls"], &[]))
        .execute(ToolRequest {
            command: "ls".into(),
            args: vec![],
            working_dir: Some(PathBuf::from("/tmp")),
            timeout: None,
        })
        .await
        .unwrap();
    assert_eq!(r.exit_code, 0);
    assert!(!r.dry_run);
}
