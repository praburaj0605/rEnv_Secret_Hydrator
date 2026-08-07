//! Opt-in file cache. Disabled by default in the façade (zero plaintext persistence NFR).
//!
//! Values are stored as JSON. Prefer encrypting the cache directory at rest (OS / KMS)
//! when enabling this backend.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use esh_core::{Cache, CacheKey, ConfigValue, Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::warn;

#[derive(Serialize, Deserialize)]
struct DiskEntry {
    value: ConfigValue,
    expires_at: DateTime<Utc>,
}

/// Filesystem cache backend.
pub struct FileCache {
    root: PathBuf,
}

impl FileCache {
    /// Create cache root directory if needed.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        warn!(
            path = %root.display(),
            "file cache enabled — ensure directory is encrypted at rest; secrets may persist on disk"
        );
        Ok(Self { root })
    }

    fn path_for(&self, key: &CacheKey) -> PathBuf {
        let name = match &key.namespace {
            Some(ns) => format!("{ns}__{}", key.key),
            None => key.key.clone(),
        };
        // Sanitize path segments.
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.root.join(format!("{safe}.json"))
    }
}

#[async_trait]
impl Cache for FileCache {
    async fn get(&self, key: &CacheKey) -> Result<Option<ConfigValue>> {
        let path = self.path_for(key);
        if !path.exists() {
            return Ok(None);
        }
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| Error::Cache(e.to_string()))?;
        let entry: DiskEntry =
            serde_json::from_str(&raw).map_err(|e| Error::Cache(e.to_string()))?;
        if entry.expires_at > Utc::now() {
            Ok(Some(entry.value))
        } else {
            let _ = tokio::fs::remove_file(&path).await;
            Ok(None)
        }
    }

    async fn put(&self, key: &CacheKey, value: ConfigValue, ttl: Duration) -> Result<()> {
        let expires_at = Utc::now()
            + chrono::Duration::from_std(ttl).unwrap_or_else(|_| chrono::Duration::seconds(0));
        let entry = DiskEntry { value, expires_at };
        let raw = serde_json::to_string(&entry).map_err(|e| Error::Cache(e.to_string()))?;
        let path = self.path_for(key);
        tokio::fs::write(&path, raw)
            .await
            .map_err(|e| Error::Cache(e.to_string()))?;
        Ok(())
    }

    async fn invalidate(&self, key: &CacheKey) -> Result<()> {
        let path = self.path_for(key);
        if path.exists() {
            tokio::fs::remove_file(path)
                .await
                .map_err(|e| Error::Cache(e.to_string()))?;
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut rd = tokio::fs::read_dir(&self.root)
            .await
            .map_err(|e| Error::Cache(e.to_string()))?;
        while let Some(ent) = rd
            .next_entry()
            .await
            .map_err(|e| Error::Cache(e.to_string()))?
        {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let _ = tokio::fs::remove_file(path).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_test_support::{run_cache_conformance, CacheConformanceCfg, TempSecrets};
    use std::time::Duration;

    #[tokio::test]
    async fn file_cache_conformance() {
        let tmp = TempSecrets::create();
        let cache = FileCache::new(tmp.path().join("cache")).unwrap();
        run_cache_conformance(
            &cache,
            &CacheConformanceCfg {
                supports_clear: true,
                short_ttl: Duration::from_millis(40),
                test_expiry: true,
            },
        )
        .await;
    }

    #[test]
    fn file_cache_is_opt_in_construction() {
        // Construction succeeds only when caller explicitly creates FileCache.
        let tmp = TempSecrets::create();
        assert!(FileCache::new(tmp.path().join("c")).is_ok());
    }
}
