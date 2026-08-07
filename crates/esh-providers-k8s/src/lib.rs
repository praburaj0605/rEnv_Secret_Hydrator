//! Kubernetes secrets via projected / mounted secret volumes.
//!
//! Default root: `/var/run/secrets/esh` (configurable). Also supports flat
//! key files under a directory (common with `secretKeyRef` volume mounts).

use async_trait::async_trait;
use esh_core::{
    ConfigValue, Error, Provider, ProviderCapability, ProviderId, ProviderMeta, Result,
};
use std::path::{Path, PathBuf};

/// File-based Kubernetes secrets provider.
pub struct KubernetesSecretsProvider {
    meta: ProviderMeta,
    root: PathBuf,
}

impl KubernetesSecretsProvider {
    /// Default mount path used by ESH conventions.
    #[must_use]
    pub fn default_path() -> Self {
        Self::from_path("/var/run/secrets/esh")
    }

    /// Custom mount directory.
    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("kubernetes"),
                name: "Kubernetes Secrets".into(),
                capabilities: ProviderCapability {
                    bulk: true,
                    versioned: false,
                    local: true,
                },
            },
            root: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl Provider for KubernetesSecretsProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        let path = self.root.join(key);
        if !path.exists() {
            return Err(Error::not_found(key));
        }
        let raw = tokio::fs::read_to_string(&path).await.map_err(|e| {
            Error::provider(
                self.meta.id.as_str(),
                format!("failed to read {}: {e}", path.display()),
            )
        })?;
        Ok(ConfigValue::secret(raw.trim_end().to_string()))
    }

    async fn health(&self) -> Result<()> {
        if self.root.is_dir()
            || std::path::Path::new("/var/run/secrets/kubernetes.io").exists()
            || std::env::var_os("KUBERNETES_SERVICE_HOST").is_some()
        {
            Ok(())
        } else {
            Err(Error::ProviderUnavailable {
                provider: self.meta.id.as_str().into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_test_support::{
        run_provider_conformance, write_secret_dir, ProviderConformanceCfg, SAMPLE_TOKEN,
    };

    #[tokio::test]
    async fn reads_mounted_secret() {
        let dir = write_secret_dir(&[("api_token", "k8s-token")]);
        let p = KubernetesSecretsProvider::from_path(dir.path());
        assert_eq!(p.get("api_token").await.unwrap().as_str(), Some("k8s-token"));
    }

    #[tokio::test]
    async fn k8s_conformance() {
        let dir = write_secret_dir(&[("api_token", SAMPLE_TOKEN)]);
        let p = KubernetesSecretsProvider::from_path(dir.path());
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "kubernetes".into(),
                expected_capabilities: ProviderCapability {
                    bulk: true,
                    versioned: false,
                    local: true,
                },
                present_key: "api_token".into(),
                present_value: SAMPLE_TOKEN.into(),
                missing_key: "missing".into(),
                fault_key: None,
                health_ok: true,
                test_get_many: true,
            },
        )
        .await;
    }
}
