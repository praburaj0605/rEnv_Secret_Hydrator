//! Provider trait and metadata.

use crate::error::Result;
use crate::value::{ConfigMap, ConfigValue};
use async_trait::async_trait;
use std::fmt;

/// Stable provider identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProviderId(pub String);

impl ProviderId {
    /// Create a provider id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow as string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ProviderId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Capability flags advertised by a provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderCapability {
    /// Can list / fetch many keys in one call.
    pub bulk: bool,
    /// Supports versioned secrets.
    pub versioned: bool,
    /// Suitable as offline / local fallback.
    pub local: bool,
}

/// Static provider metadata.
#[derive(Clone, Debug)]
pub struct ProviderMeta {
    /// Identifier.
    pub id: ProviderId,
    /// Human label.
    pub name: String,
    /// Capabilities.
    pub capabilities: ProviderCapability,
}

/// Async secret / configuration provider.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider metadata.
    fn meta(&self) -> &ProviderMeta;

    /// Fetch a single key. Return `Error::NotFound` when absent.
    async fn get(&self, key: &str) -> Result<ConfigValue>;

    /// Optional bulk fetch. Default walks `keys` via [`get`](Self::get).
    async fn get_many(&self, keys: &[String]) -> Result<ConfigMap> {
        let mut out = ConfigMap::new();
        for key in keys {
            match self.get(key).await {
                Ok(v) => {
                    out.insert(key.clone(), v);
                }
                Err(crate::error::Error::NotFound { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Health / availability probe.
    async fn health(&self) -> Result<()> {
        Ok(())
    }
}
