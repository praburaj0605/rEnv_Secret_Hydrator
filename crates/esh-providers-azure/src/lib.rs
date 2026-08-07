//! Azure Key Vault secrets provider with injectable client.

use async_trait::async_trait;
use esh_core::{
    ConfigValue, Error, Provider, ProviderCapability, ProviderId, ProviderMeta, Result,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Client abstraction for Key Vault secret get.
#[async_trait]
pub trait AzureKeyVaultClient: Send + Sync {
    /// Fetch secret by name.
    async fn get_secret(&self, name: &str) -> Result<String>;
}

/// In-memory fake client.
#[derive(Default, Clone)]
pub struct MapAzureClient {
    secrets: HashMap<String, String>,
    fault: Option<AzureFault>,
}

/// Injected failure mode for [`MapAzureClient`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AzureFault {
    /// Access denied.
    Auth,
    /// Request timeout.
    Timeout,
}

impl MapAzureClient {
    /// Empty client.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert secret.
    #[must_use]
    pub fn with_secret(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(name.into(), value.into());
        self
    }

    /// Force failures.
    #[must_use]
    pub fn with_fault(mut self, fault: AzureFault) -> Self {
        self.fault = Some(fault);
        self
    }
}

#[async_trait]
impl AzureKeyVaultClient for MapAzureClient {
    async fn get_secret(&self, name: &str) -> Result<String> {
        match self.fault {
            Some(AzureFault::Auth) => return Err(Error::provider("azure-key-vault", "access denied")),
            Some(AzureFault::Timeout) => return Err(Error::provider("azure-key-vault", "timeout")),
            None => {}
        }
        self.secrets
            .get(name)
            .cloned()
            .ok_or_else(|| Error::not_found(name))
    }
}

/// Azure Key Vault provider.
pub struct AzureKeyVaultProvider {
    meta: ProviderMeta,
    client: Arc<dyn AzureKeyVaultClient>,
    vault_url: String,
}

impl AzureKeyVaultProvider {
    /// Create provider. `vault_url` is retained for diagnostics / future HTTP clients.
    #[must_use]
    pub fn new(vault_url: impl Into<String>, client: Arc<dyn AzureKeyVaultClient>) -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("azure-key-vault"),
                name: "Azure Key Vault".into(),
                capabilities: ProviderCapability {
                    bulk: false,
                    versioned: true,
                    local: false,
                },
            },
            client,
            vault_url: vault_url.into(),
        }
    }

    /// Vault base URL.
    #[must_use]
    pub fn vault_url(&self) -> &str {
        &self.vault_url
    }
}

#[async_trait]
impl Provider for AzureKeyVaultProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        // Azure secret names often use `-` instead of `_`.
        let candidates = [key.to_string(), key.replace('_', "-")];
        for name in &candidates {
            match self.client.get_secret(name).await {
                Ok(v) => return Ok(ConfigValue::secret(v)),
                Err(Error::NotFound { .. }) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::not_found(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_test_support::{
        run_provider_conformance, ProviderConformanceCfg, SAMPLE_SECRET, SAMPLE_TOKEN,
    };

    #[tokio::test]
    async fn resolves_underscore_to_hyphen() {
        let client = Arc::new(MapAzureClient::new().with_secret("db-password", "p"));
        let p = AzureKeyVaultProvider::new("https://example.vault.azure.net", client);
        assert_eq!(p.get("db_password").await.unwrap().as_str(), Some("p"));
    }

    #[tokio::test]
    async fn azure_conformance() {
        let client = Arc::new(MapAzureClient::new().with_secret("database_url", SAMPLE_SECRET));
        let p = AzureKeyVaultProvider::new("https://example.vault.azure.net", client);
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "azure-key-vault".into(),
                expected_capabilities: ProviderCapability {
                    bulk: false,
                    versioned: true,
                    local: false,
                },
                present_key: "database_url".into(),
                present_value: SAMPLE_SECRET.into(),
                missing_key: "missing".into(),
                fault_key: None,
                health_ok: true,
                test_get_many: true,
            },
        )
        .await;
    }

    #[tokio::test]
    async fn azure_auth_fault_no_leak() {
        let client = Arc::new(
            MapAzureClient::new()
                .with_secret("api_token", SAMPLE_TOKEN)
                .with_fault(AzureFault::Auth),
        );
        let p = AzureKeyVaultProvider::new("https://example.vault.azure.net", client);
        let err = p.get("api_token").await.unwrap_err();
        assert!(!err.to_string().contains(SAMPLE_TOKEN));
    }
}
