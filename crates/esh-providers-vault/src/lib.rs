//! HashiCorp Vault KV v2 provider with injectable client.

use async_trait::async_trait;
use esh_core::{
    ConfigMap, ConfigValue, Error, Provider, ProviderCapability, ProviderId, ProviderMeta, Result,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// Vault KV read client.
#[async_trait]
pub trait VaultClient: Send + Sync {
    /// Read a KV v2 secret path; returns the `data` object as JSON.
    async fn read_kv2(&self, mount: &str, path: &str) -> Result<JsonValue>;
}

/// In-memory fake Vault.
#[derive(Default, Clone)]
pub struct MapVaultClient {
    /// map key: `{mount}/{path}` → data object
    data: HashMap<String, JsonValue>,
    fault: Option<VaultFault>,
}

/// Injected failure mode for [`MapVaultClient`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultFault {
    /// Permission denied.
    Auth,
    /// Upstream timeout.
    Timeout,
}

impl MapVaultClient {
    /// Empty client.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert KV data.
    #[must_use]
    pub fn with_secret(mut self, mount: &str, path: &str, data: JsonValue) -> Self {
        self.data.insert(format!("{mount}/{path}"), data);
        self
    }

    /// Force failures.
    #[must_use]
    pub fn with_fault(mut self, fault: VaultFault) -> Self {
        self.fault = Some(fault);
        self
    }
}

#[async_trait]
impl VaultClient for MapVaultClient {
    async fn read_kv2(&self, mount: &str, path: &str) -> Result<JsonValue> {
        match self.fault {
            Some(VaultFault::Auth) => return Err(Error::provider("vault", "permission denied")),
            Some(VaultFault::Timeout) => return Err(Error::provider("vault", "timeout")),
            None => {}
        }
        self.data
            .get(&format!("{mount}/{path}"))
            .cloned()
            .ok_or_else(|| Error::not_found(path))
    }
}

/// Vault KV v2 provider.
///
/// Modes:
/// - **path mode** (default): logical key maps to `path_prefix/key` and returns a chosen field
///   or the whole JSON as string.
/// - **field mode**: fixed `path` secret; logical key selects a field inside `data`.
pub struct VaultKvProvider {
    meta: ProviderMeta,
    client: Arc<dyn VaultClient>,
    mount: String,
    path_prefix: String,
    /// When set, treat this path as the secret and keys as fields.
    fixed_path: Option<String>,
    field: Option<String>,
}

impl VaultKvProvider {
    /// Create provider for a KV mount (default `secret`).
    #[must_use]
    pub fn new(client: Arc<dyn VaultClient>) -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("vault"),
                name: "HashiCorp Vault".into(),
                capabilities: ProviderCapability {
                    bulk: true,
                    versioned: true,
                    local: false,
                },
            },
            client,
            mount: "secret".into(),
            path_prefix: String::new(),
            fixed_path: None,
            field: None,
        }
    }

    /// KV mount name.
    #[must_use]
    pub fn mount(mut self, mount: impl Into<String>) -> Self {
        self.mount = mount.into();
        self
    }

    /// Path prefix for per-key secrets.
    #[must_use]
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = prefix.into();
        self
    }

    /// Use a single secret path; logical keys are JSON fields.
    #[must_use]
    pub fn secret_path(mut self, path: impl Into<String>) -> Self {
        self.fixed_path = Some(path.into());
        self
    }

    /// When not in field mode, extract this field from each secret object.
    #[must_use]
    pub fn value_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}

#[async_trait]
impl Provider for VaultKvProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        if let Some(path) = &self.fixed_path {
            let data = self.client.read_kv2(&self.mount, path).await?;
            let val = data
                .get(key)
                .ok_or_else(|| Error::not_found(key))?;
            return Ok(json_to_secret(val));
        }

        let path = if self.path_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{key}", self.path_prefix.trim_end_matches('/'))
        };
        let data = self.client.read_kv2(&self.mount, &path).await.map_err(|e| {
            match e {
                Error::NotFound { .. } => Error::not_found(key),
                other => other,
            }
        })?;

        if let Some(field) = &self.field {
            let val = data.get(field).ok_or_else(|| Error::not_found(key))?;
            Ok(json_to_secret(val))
        } else if let Some(val) = data.get("value") {
            Ok(json_to_secret(val))
        } else if data.is_object() {
            Ok(ConfigValue::secret(data.to_string()))
        } else {
            Ok(json_to_secret(&data))
        }
    }

    async fn get_many(&self, keys: &[String]) -> Result<ConfigMap> {
        if let Some(path) = &self.fixed_path {
            let data = self.client.read_kv2(&self.mount, path).await?;
            let mut out = ConfigMap::new();
            if keys.is_empty() {
                if let Some(obj) = data.as_object() {
                    for (k, v) in obj {
                        out.insert(k.clone(), json_to_secret(v));
                    }
                }
            } else {
                for key in keys {
                    if let Some(v) = data.get(key) {
                        out.insert(key.clone(), json_to_secret(v));
                    }
                }
            }
            return Ok(out);
        }
        // Fall back to default trait impl behavior via sequential get
        let mut out = ConfigMap::new();
        for key in keys {
            match self.get(key).await {
                Ok(v) => {
                    out.insert(key.clone(), v);
                }
                Err(Error::NotFound { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }
}

fn json_to_secret(v: &JsonValue) -> ConfigValue {
    match v {
        JsonValue::String(s) => ConfigValue::secret(s.clone()),
        other => ConfigValue::secret(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_test_support::{
        run_provider_conformance, ProviderConformanceCfg, SAMPLE_SECRET,
    };
    use serde_json::json;

    #[tokio::test]
    async fn field_mode() {
        let client = Arc::new(MapVaultClient::new().with_secret(
            "secret",
            "app/prod",
            json!({"database_url": "postgres://vault"}),
        ));
        let p = VaultKvProvider::new(client).secret_path("app/prod");
        assert_eq!(
            p.get("database_url").await.unwrap().as_str(),
            Some("postgres://vault")
        );
    }

    #[tokio::test]
    async fn vault_conformance() {
        let client = Arc::new(MapVaultClient::new().with_secret(
            "secret",
            "app",
            json!({ "database_url": SAMPLE_SECRET }),
        ));
        let p = VaultKvProvider::new(client).secret_path("app");
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "vault".into(),
                expected_capabilities: ProviderCapability {
                    bulk: true,
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
    async fn vault_auth_fault() {
        let client = Arc::new(
            MapVaultClient::new()
                .with_secret("secret", "app", json!({ "database_url": SAMPLE_SECRET }))
                .with_fault(VaultFault::Auth),
        );
        let p = VaultKvProvider::new(client).secret_path("app");
        let err = p.get("database_url").await.unwrap_err();
        assert!(!err.to_string().contains(SAMPLE_SECRET));
    }
}
