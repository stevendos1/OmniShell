//! Severity levels for policy violations.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Severity level for policy violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Informational — logged but not blocking.
    Info,
    /// Warning — logged, may trigger rate limit.
    Warning,
    /// Critical — request is rejected.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}
