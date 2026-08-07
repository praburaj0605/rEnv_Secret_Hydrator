//! Error types that must never embed secret values.

use crate::validate::ValidationError;
use thiserror::Error;

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Library error. Messages may include key names / provider IDs, never values.
#[derive(Debug, Error)]
pub enum Error {
    /// Requested key was not found in any provider.
    #[error("secret key not found: {key}")]
    NotFound {
        /// Config / secret key name.
        key: String,
    },

    /// Provider is unavailable (offline, misconfigured, or not selected).
    #[error("provider unavailable: {provider}")]
    ProviderUnavailable {
        /// Provider identifier.
        provider: String,
    },

    /// Provider returned an operational failure (auth, timeout, 5xx, etc.).
    #[error("provider `{provider}` failed: {message}")]
    Provider {
        /// Provider identifier.
        provider: String,
        /// Safe error message (no secret material).
        message: String,
    },

    /// Configuration failed validation.
    #[error("configuration validation failed")]
    Validation(#[from] ValidationError),

    /// Typed deserialization failed.
    #[error("failed to deserialize configuration: {0}")]
    Deserialize(String),

    /// Cache backend failure.
    #[error("cache error: {0}")]
    Cache(String),

    /// Encryption / decryption failure.
    #[error("crypto error: {0}")]
    Crypto(String),

    /// Invalid builder / runtime configuration.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// I/O failure.
    #[error("I/O error: {0}")]
    Io(String),

    /// Unexpected internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Construct a provider error with a safe message.
    pub fn provider(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Provider {
            provider: provider.into(),
            message: message.into(),
        }
    }

    /// Construct a not-found error.
    pub fn not_found(key: impl Into<String>) -> Self {
        Self::NotFound { key: key.into() }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_does_not_include_secret_payloads() {
        let err = Error::provider("aws", "access denied");
        let s = err.to_string();
        assert!(s.contains("aws"));
        assert!(!s.contains("AKIA"));
    }
}
