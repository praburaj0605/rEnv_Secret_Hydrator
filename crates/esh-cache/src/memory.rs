//! In-memory TTL cache.

use async_trait::async_trait;
use dashmap::DashMap;
use esh_core::{Cache, CacheEntry, CacheKey, ConfigValue, Result};
use std::time::Duration;

/// Process-local memory cache.
#[derive(Debug, Default)]
pub struct MemoryCache {
    inner: DashMap<String, CacheEntry>,
    default_ttl: Duration,
}

impl MemoryCache {
    /// Create with a default TTL used by helpers.
    #[must_use]
    pub fn ttl(ttl: Duration) -> Self {
        Self {
            inner: DashMap::new(),
            default_ttl: ttl,
        }
    }

    /// Default 300s TTL.
    #[must_use]
    pub fn new() -> Self {
        Self::ttl(Duration::from_secs(300))
    }

    fn map_key(key: &CacheKey) -> String {
        match &key.namespace {
            Some(ns) => format!("{ns}::{}", key.key),
            None => key.key.clone(),
        }
    }

    /// Default TTL.
    #[must_use]
    pub fn default_ttl(&self) -> Duration {
        self.default_ttl
    }
}

#[async_trait]
impl Cache for MemoryCache {
    async fn get(&self, key: &CacheKey) -> Result<Option<ConfigValue>> {
        let k = Self::map_key(key);
        if let Some(entry) = self.inner.get(&k) {
            if entry.is_fresh() {
                return Ok(Some(entry.value.clone()));
            }
            drop(entry);
            self.inner.remove(&k);
        }
        Ok(None)
    }

    async fn put(&self, key: &CacheKey, value: ConfigValue, ttl: Duration) -> Result<()> {
        self.inner
            .insert(Self::map_key(key), CacheEntry::with_ttl(value, ttl));
        Ok(())
    }

    async fn invalidate(&self, key: &CacheKey) -> Result<()> {
        self.inner.remove(&Self::map_key(key));
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        self.inner.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_core::ConfigValue;
    use esh_test_support::{run_cache_conformance, CacheConformanceCfg};
    use std::time::Duration;

    #[tokio::test]
    async fn memory_roundtrip() {
        let cache = MemoryCache::ttl(Duration::from_secs(60));
        let key = CacheKey::new("a");
        cache
            .put(&key, ConfigValue::secret("v"), Duration::from_secs(60))
            .await
            .unwrap();
        let got = cache.get(&key).await.unwrap().unwrap();
        assert_eq!(got.as_str(), Some("v"));
    }

    #[tokio::test]
    async fn memory_conformance() {
        let cache = MemoryCache::new();
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
}
