//! Counting and fault-injecting test doubles.

use async_trait::async_trait;
use esh_core::{
    Cache, CacheKey, ConfigMap, ConfigValue, Error, Provider, ProviderCapability, ProviderId,
    ProviderMeta, Result,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// How a faulty provider should fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultMode {
    Auth,
    Timeout,
    Unavailable,
}

/// In-memory map provider for tests.
#[derive(Clone)]
pub struct MapProvider {
    meta: ProviderMeta,
    values: HashMap<String, String>,
}

impl MapProvider {
    pub fn new(id: impl Into<String>, local: bool) -> Self {
        let id = id.into();
        Self {
            meta: ProviderMeta {
                id: ProviderId::new(id.clone()),
                name: id,
                capabilities: ProviderCapability {
                    bulk: true,
                    versioned: false,
                    local,
                },
            },
            values: HashMap::new(),
        }
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(key.into(), value.into());
        self
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }
}

#[async_trait]
impl Provider for MapProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        self.values
            .get(key)
            .cloned()
            .map(ConfigValue::secret)
            .ok_or_else(|| Error::not_found(key))
    }

    async fn get_many(&self, keys: &[String]) -> Result<ConfigMap> {
        let mut out = ConfigMap::new();
        if keys.is_empty() {
            for (k, v) in &self.values {
                out.insert(k.clone(), ConfigValue::secret(v.clone()));
            }
            return Ok(out);
        }
        for key in keys {
            if let Some(v) = self.values.get(key) {
                out.insert(key.clone(), ConfigValue::secret(v.clone()));
            }
        }
        Ok(out)
    }
}

/// Wraps an inner provider and counts `get` / `get_many` / `health` calls.
pub struct CountingProvider {
    inner: Arc<dyn Provider>,
    pub gets: AtomicU64,
    pub get_manys: AtomicU64,
    pub healths: AtomicU64,
}

impl CountingProvider {
    pub fn new(inner: Arc<dyn Provider>) -> Self {
        Self {
            inner,
            gets: AtomicU64::new(0),
            get_manys: AtomicU64::new(0),
            healths: AtomicU64::new(0),
        }
    }

    pub fn get_count(&self) -> u64 {
        self.gets.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Provider for CountingProvider {
    fn meta(&self) -> &ProviderMeta {
        self.inner.meta()
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.inner.get(key).await
    }

    async fn get_many(&self, keys: &[String]) -> Result<ConfigMap> {
        self.get_manys.fetch_add(1, Ordering::SeqCst);
        self.inner.get_many(keys).await
    }

    async fn health(&self) -> Result<()> {
        self.healths.fetch_add(1, Ordering::SeqCst);
        self.inner.health().await
    }
}

/// Provider that always fails according to [`FaultMode`].
pub struct FaultyProvider {
    meta: ProviderMeta,
    mode: FaultMode,
}

impl FaultyProvider {
    pub fn new(id: impl Into<String>, mode: FaultMode) -> Self {
        let id = id.into();
        Self {
            meta: ProviderMeta {
                id: ProviderId::new(id.clone()),
                name: format!("faulty-{id}"),
                capabilities: ProviderCapability {
                    bulk: false,
                    versioned: false,
                    local: false,
                },
            },
            mode,
        }
    }
}

#[async_trait]
impl Provider for FaultyProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, _key: &str) -> Result<ConfigValue> {
        match self.mode {
            FaultMode::Auth => Err(Error::provider(self.meta.id.as_str(), "access denied")),
            FaultMode::Timeout => Err(Error::provider(self.meta.id.as_str(), "timeout")),
            FaultMode::Unavailable => Err(Error::ProviderUnavailable {
                provider: self.meta.id.as_str().into(),
            }),
        }
    }

    async fn health(&self) -> Result<()> {
        match self.mode {
            FaultMode::Unavailable => Err(Error::ProviderUnavailable {
                provider: self.meta.id.as_str().into(),
            }),
            FaultMode::Auth => Err(Error::provider(self.meta.id.as_str(), "access denied")),
            FaultMode::Timeout => Err(Error::provider(self.meta.id.as_str(), "timeout")),
        }
    }
}

/// Counting cache wrapper.
pub struct CountingCache {
    inner: Arc<dyn Cache>,
    pub gets: AtomicU64,
    pub puts: AtomicU64,
}

impl CountingCache {
    pub fn new(inner: Arc<dyn Cache>) -> Self {
        Self {
            inner,
            gets: AtomicU64::new(0),
            puts: AtomicU64::new(0),
        }
    }

    pub fn get_count(&self) -> u64 {
        self.gets.load(Ordering::SeqCst)
    }

    pub fn put_count(&self) -> u64 {
        self.puts.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Cache for CountingCache {
    async fn get(&self, key: &CacheKey) -> Result<Option<ConfigValue>> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.inner.get(key).await
    }

    async fn put(&self, key: &CacheKey, value: ConfigValue, ttl: Duration) -> Result<()> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.put(key, value, ttl).await
    }

    async fn invalidate(&self, key: &CacheKey) -> Result<()> {
        self.inner.invalidate(key).await
    }

    async fn clear(&self) -> Result<()> {
        self.inner.clear().await
    }
}
