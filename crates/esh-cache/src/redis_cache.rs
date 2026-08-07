//! Redis cache backend (feature `redis-cache`).

use async_trait::async_trait;
use esh_core::{Cache, CacheKey, ConfigValue, Error, Result};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::time::Duration;

/// Redis-backed cache.
pub struct RedisCache {
    conn: ConnectionManager,
    key_prefix: String,
}

impl RedisCache {
    /// Connect using a redis URL (`redis://...`).
    pub async fn connect(url: &str) -> Result<Self> {
        let client = redis::Client::open(url).map_err(|e| Error::Cache(e.to_string()))?;
        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| Error::Cache(e.to_string()))?;
        Ok(Self {
            conn,
            key_prefix: "esh:".into(),
        })
    }

    /// Set key prefix.
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    fn redis_key(&self, key: &CacheKey) -> String {
        match &key.namespace {
            Some(ns) => format!("{}{ns}:{}", self.key_prefix, key.key),
            None => format!("{}{}", self.key_prefix, key.key),
        }
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get(&self, key: &CacheKey) -> Result<Option<ConfigValue>> {
        let mut conn = self.conn.clone();
        let rk = self.redis_key(key);
        let raw: Option<String> = conn
            .get(rk)
            .await
            .map_err(|e| Error::Cache(e.to_string()))?;
        match raw {
            Some(s) => {
                let v: ConfigValue =
                    serde_json::from_str(&s).map_err(|e| Error::Cache(e.to_string()))?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, key: &CacheKey, value: ConfigValue, ttl: Duration) -> Result<()> {
        let mut conn = self.conn.clone();
        let rk = self.redis_key(key);
        let raw = serde_json::to_string(&value).map_err(|e| Error::Cache(e.to_string()))?;
        let secs = ttl.as_secs().max(1);
        conn.set_ex::<_, _, ()>(rk, raw, secs)
            .await
            .map_err(|e| Error::Cache(e.to_string()))?;
        Ok(())
    }

    async fn invalidate(&self, key: &CacheKey) -> Result<()> {
        let mut conn = self.conn.clone();
        let rk = self.redis_key(key);
        let _: () = conn.del(rk).await.map_err(|e| Error::Cache(e.to_string()))?;
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        Err(Error::Cache(
            "RedisCache::clear is intentionally unsupported; delete by prefix externally".into(),
        ))
    }
}
