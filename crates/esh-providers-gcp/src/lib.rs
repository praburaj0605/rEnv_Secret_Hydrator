//! Google Secret Manager provider with injectable client.

use async_trait::async_trait;
use esh_core::{
    ConfigValue, Error, Provider, ProviderCapability, ProviderId, ProviderMeta, Result,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Client abstraction for GCP Secret Manager access.
#[async_trait]
pub trait GcpSecretClient: Send + Sync {
    /// Access secret payload for `projects/{project}/secrets/{id}/versions/latest`.
    async fn access_secret(&self, secret_id: &str) -> Result<String>;
}

/// In-memory fake.
#[derive(Default, Clone)]
pub struct MapGcpClient {
    secrets: HashMap<String, String>,
    fault: Option<GcpFault>,
}

/// Injected failure mode for [`MapGcpClient`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GcpFault {
    /// Access denied.
    Auth,
    /// Request timeout.
    Timeout,
}

impl MapGcpClient {
    /// Empty client.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert secret.
    #[must_use]
    pub fn with_secret(mut self, id: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(id.into(), value.into());
        self
    }

    /// Force failures.
    #[must_use]
    pub fn with_fault(mut self, fault: GcpFault) -> Self {
        self.fault = Some(fault);
        self
    }
}

#[async_trait]
impl GcpSecretClient for MapGcpClient {
    async fn access_secret(&self, secret_id: &str) -> Result<String> {
        match self.fault {
            Some(GcpFault::Auth) => {
                return Err(Error::provider("gcp-secret-manager", "access denied"));
            }
            Some(GcpFault::Timeout) => {
                return Err(Error::provider("gcp-secret-manager", "timeout"));
            }
            None => {}
        }
        self.secrets
            .get(secret_id)
            .cloned()
            .ok_or_else(|| Error::not_found(secret_id))
    }
}

/// GCP Secret Manager provider.
pub struct GcpSecretManagerProvider {
    meta: ProviderMeta,
    client: Arc<dyn GcpSecretClient>,
    project: String,
    prefix: String,
}

impl GcpSecretManagerProvider {
    /// Create provider for a GCP project.
    #[must_use]
    pub fn new(project: impl Into<String>, client: Arc<dyn GcpSecretClient>) -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("gcp-secret-manager"),
                name: "Google Secret Manager".into(),
                capabilities: ProviderCapability {
                    bulk: false,
                    versioned: true,
                    local: false,
                },
            },
            client,
            project: project.into(),
            prefix: String::new(),
        }
    }

    /// Optional name prefix.
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// GCP project id.
    #[must_use]
    pub fn project(&self) -> &str {
        &self.project
    }

    fn secret_id(&self, key: &str) -> String {
        // Secret Manager IDs: letters, numbers, hyphens.
        let normalized = key.replace('_', "-");
        format!("{}{normalized}", self.prefix)
    }
}

#[async_trait]
impl Provider for GcpSecretManagerProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        let id = self.secret_id(key);
        let body = self.client.access_secret(&id).await.map_err(|e| match e {
            Error::NotFound { .. } => Error::not_found(key),
            other => other,
        })?;
        Ok(ConfigValue::secret(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_test_support::{
        run_provider_conformance, ProviderConformanceCfg, SAMPLE_SECRET,
    };

    #[tokio::test]
    async fn reads_secret() {
        let client = Arc::new(MapGcpClient::new().with_secret("db-url", "postgres://gcp"));
        let p = GcpSecretManagerProvider::new("demo", client);
        assert_eq!(p.get("db_url").await.unwrap().as_str(), Some("postgres://gcp"));
    }

    #[tokio::test]
    async fn gcp_conformance() {
        let client = Arc::new(MapGcpClient::new().with_secret("database-url", SAMPLE_SECRET));
        let p = GcpSecretManagerProvider::new("demo", client);
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "gcp-secret-manager".into(),
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
    async fn gcp_timeout_fault() {
        let client = Arc::new(
            MapGcpClient::new()
                .with_secret("database-url", SAMPLE_SECRET)
                .with_fault(GcpFault::Timeout),
        );
        let p = GcpSecretManagerProvider::new("demo", client);
        let err = p.get("database_url").await.unwrap_err();
        assert!(!err.to_string().contains(SAMPLE_SECRET));
    }
}
