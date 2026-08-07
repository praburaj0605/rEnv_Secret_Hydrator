//! Environment and dotenv configuration providers.

use async_trait::async_trait;
use esh_core::{
    ConfigValue, Error, Provider, ProviderCapability, ProviderId, ProviderMeta, Result,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

/// Reads configuration from process environment variables.
pub struct EnvProvider {
    meta: ProviderMeta,
    /// Optional prefix stripped from env keys (e.g. `APP_`).
    prefix: Option<String>,
}

impl EnvProvider {
    /// Create a provider with no prefix.
    #[must_use]
    pub fn new() -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("env"),
                name: "Environment Variables".into(),
                capabilities: ProviderCapability {
                    bulk: true,
                    versioned: false,
                    local: true,
                },
            },
            prefix: None,
        }
    }

    /// Only consider env vars starting with `prefix` (prefix removed from key).
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    fn env_key(&self, key: &str) -> String {
        match &self.prefix {
            Some(p) => format!("{p}{key}"),
            None => key.to_string(),
        }
    }
}

impl Default for EnvProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for EnvProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        let env_key = self.env_key(key);
        match env::var(&env_key) {
            Ok(v) => Ok(ConfigValue::secret(v)),
            Err(env::VarError::NotPresent) => Err(Error::not_found(key)),
            Err(env::VarError::NotUnicode(_)) => Err(Error::provider(
                self.meta.id.as_str(),
                format!("environment variable `{env_key}` is not valid UTF-8"),
            )),
        }
    }
}

/// Reads from a `.env` file (and optional in-memory overlay).
pub struct DotenvProvider {
    meta: ProviderMeta,
    path: PathBuf,
    values: RwLock<HashMap<String, String>>,
}

impl DotenvProvider {
    /// Load from `path` immediately.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut values = HashMap::new();
        if path.exists() {
            let iter = dotenvy::from_path_iter(&path).map_err(|e| {
                Error::provider("dotenv", format!("failed to read {}: {e}", path.display()))
            })?;
            for item in iter {
                let (k, v) = item.map_err(|e| Error::provider("dotenv", e.to_string()))?;
                values.insert(k, v);
            }
        }
        Ok(Self {
            meta: ProviderMeta {
                id: ProviderId::new("dotenv"),
                name: "Dotenv File".into(),
                capabilities: ProviderCapability {
                    bulk: true,
                    versioned: false,
                    local: true,
                },
            },
            path,
            values: RwLock::new(values),
        })
    }

    /// Default `.env` in current directory.
    pub fn from_default() -> Result<Self> {
        Self::from_path(".env")
    }

    /// Reload file from disk.
    pub fn reload(&self) -> Result<()> {
        let mut values = HashMap::new();
        if self.path.exists() {
            let iter = dotenvy::from_path_iter(&self.path).map_err(|e| {
                Error::provider(
                    "dotenv",
                    format!("failed to read {}: {e}", self.path.display()),
                )
            })?;
            for item in iter {
                let (k, v) = item.map_err(|e| Error::provider("dotenv", e.to_string()))?;
                values.insert(k, v);
            }
        }
        *self.values.write() = values;
        Ok(())
    }
}

#[async_trait]
impl Provider for DotenvProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        self.values
            .read()
            .get(key)
            .cloned()
            .map(ConfigValue::secret)
            .ok_or_else(|| Error::not_found(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_test_support::{
        run_provider_conformance, write_temp_dotenv, ProviderConformanceCfg, SAMPLE_SECRET,
    };
    use std::sync::OnceLock;
    use tokio::sync::Mutex;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[tokio::test]
    async fn env_provider_reads_var() {
        let _g = env_lock().lock().await;
        env::set_var("ESH_TEST_KEY_XYZ", "value-1");
        let p = EnvProvider::new();
        let v = p.get("ESH_TEST_KEY_XYZ").await.unwrap();
        assert_eq!(v.as_str(), Some("value-1"));
        env::remove_var("ESH_TEST_KEY_XYZ");
    }

    #[tokio::test]
    async fn dotenv_provider_reads_file() {
        let (_tmp, path) = write_temp_dotenv("DATABASE_URL=postgres://local\n");
        let p = DotenvProvider::from_path(&path).unwrap();
        let v = p.get("DATABASE_URL").await.unwrap();
        assert_eq!(v.as_str(), Some("postgres://local"));
    }

    #[tokio::test]
    async fn env_conformance() {
        let _g = env_lock().lock().await;
        env::set_var("database_url", SAMPLE_SECRET);
        let p = EnvProvider::new();
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "env".into(),
                expected_capabilities: ProviderCapability {
                    bulk: true,
                    versioned: false,
                    local: true,
                },
                present_key: "database_url".into(),
                present_value: SAMPLE_SECRET.into(),
                missing_key: "esh_missing_key_zzz".into(),
                fault_key: None,
                health_ok: true,
                test_get_many: true,
            },
        )
        .await;
        env::remove_var("database_url");
    }

    #[tokio::test]
    async fn dotenv_conformance() {
        let (_tmp, path) = write_temp_dotenv(&format!("database_url={SAMPLE_SECRET}\n"));
        let p = DotenvProvider::from_path(&path).unwrap();
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "dotenv".into(),
                expected_capabilities: ProviderCapability {
                    bulk: true,
                    versioned: false,
                    local: true,
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
}
