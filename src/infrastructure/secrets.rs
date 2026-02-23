//! # Environment Secrets Provider
//!
//! Default implementation of `SecretsProvider` that reads from
//! environment variables. Never logs or caches secret values.

use tracing::debug;

use crate::domain::error::{OrchestratorError, Result};
use crate::domain::secrets::SecretsProvider;

/// Secrets provider that reads from environment variables.
///
/// # Security
/// - Values are never logged or serialized.
/// - Only the key name is logged at debug level.
///
/// # Example
/// ```
/// use omnishell_orchestrator::infrastructure::secrets::EnvSecretsProvider;
/// use omnishell_orchestrator::domain::secrets::SecretsProvider;
///
/// let provider = EnvSecretsProvider;
/// // provider.get_secret("MY_API_KEY") reads $MY_API_KEY
/// ```
pub struct EnvSecretsProvider;

impl SecretsProvider for EnvSecretsProvider {
    fn get_secret(&self, key: &str) -> Result<String> {
        debug!(key, "reading secret from environment");
        std::env::var(key).map_err(|_| {
            OrchestratorError::SecretsError(format!("secret '{key}' not found in environment"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_secret_not_found() {
        let provider = EnvSecretsProvider;
        let result = provider.get_secret("DEFINITELY_NOT_SET_12345");
        assert!(result.is_err());
    }

    #[test]
    fn test_env_secret_found() {
        // PATH is almost always set.
        let provider = EnvSecretsProvider;
        let result = provider.get_secret("PATH");
        assert!(result.is_ok());
    }

    #[test]
    fn test_has_secret() {
        let provider = EnvSecretsProvider;
        assert!(provider.has_secret("PATH"));
        assert!(!provider.has_secret("DEFINITELY_NOT_SET_12345"));
    }
}
