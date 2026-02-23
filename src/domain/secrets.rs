//! # Secrets Provider Domain
//!
//! Defines the port for retrieving secrets (API keys, tokens, etc.).
//! Secrets are NEVER hardcoded. The default implementation reads from
//! environment variables.

use crate::domain::error::Result;

/// Port for retrieving secrets.
///
/// Implementations may read from environment variables, a vault,
/// a keyring, or any other secure store.
///
/// # Security
/// - Secrets must never be logged, cached, or serialized.
/// - Implementations must treat secret values as opaque strings.
///
/// # Example
/// ```
/// use omnishell_orchestrator::domain::secrets::SecretsProvider;
/// use omnishell_orchestrator::domain::error::Result;
///
/// struct EnvSecrets;
///
/// impl SecretsProvider for EnvSecrets {
///     fn get_secret(&self, key: &str) -> Result<String> {
///         std::env::var(key).map_err(|_| {
///             omnishell_orchestrator::domain::error::OrchestratorError::SecretsError(
///                 format!("secret '{}' not found in environment", key)
///             )
///         })
///     }
/// }
/// ```
pub trait SecretsProvider: Send + Sync {
    /// Retrieve a secret by key.
    ///
    /// # Errors
    /// Returns `OrchestratorError::SecretsError` if the secret is not found
    /// or cannot be retrieved.
    fn get_secret(&self, key: &str) -> Result<String>;

    /// Check whether a secret exists without returning its value.
    fn has_secret(&self, key: &str) -> bool {
        self.get_secret(key).is_ok()
    }
}
