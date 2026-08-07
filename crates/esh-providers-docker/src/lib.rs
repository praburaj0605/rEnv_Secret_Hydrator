//! Docker secrets provider (`/run/secrets` by default).

use async_trait::async_trait;
use esh_core::{
    ConfigValue, Error, Provider, ProviderCapability, ProviderId, ProviderMeta, Result,
};
use std::path::{Path, PathBuf};

/// Reads Docker Swarm / Compose secrets from a directory.
pub struct DockerSecretsProvider {
    meta: ProviderMeta,
    root: PathBuf,
}

impl DockerSecretsProvider {
    /// Use `/run/secrets`.
    #[must_use]
    pub fn default_path() -> Self {
        Self::from_path("/run/secrets")
    }

    /// Custom secrets directory (useful in tests).
    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("docker"),
                name: "Docker Secrets".into(),
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
impl Provider for DockerSecretsProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        // Docker secret names often use underscores; also allow as-is filenames.
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
        if self.root.is_dir() {
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
    async fn reads_secret_file() {
        let dir = write_secret_dir(&[("db_password", "s3cret\n")]);
        let p = DockerSecretsProvider::from_path(dir.path());
        let v = p.get("db_password").await.unwrap();
        assert_eq!(v.as_str(), Some("s3cret"));
    }

    #[tokio::test]
    async fn docker_conformance() {
        let dir = write_secret_dir(&[("api_token", SAMPLE_TOKEN)]);
        let p = DockerSecretsProvider::from_path(dir.path());
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "docker".into(),
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
