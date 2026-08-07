//! Cache trait.

use crate::error::Result;
use crate::value::ConfigValue;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

/// Cache lookup key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Logical config key.
    pub key: String,
    /// Optional provider namespace.
    pub namespace: Option<String>,
}

impl CacheKey {
    /// Create a simple key.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            namespace: None,
        }
    }
}

/// Cached entry with expiry.
#[derive(Clone, Debug)]
pub struct CacheEntry {
    /// Value.
    pub value: ConfigValue,
    /// Absolute expiry time.
    pub expires_at: DateTime<Utc>,
}

impl CacheEntry {
    /// Create an entry that expires after `ttl`.
    #[must_use]
    pub fn with_ttl(value: ConfigValue, ttl: Duration) -> Self {
        let expires_at = Utc::now()
            + chrono::Duration::from_std(ttl).unwrap_or_else(|_| chrono::Duration::seconds(0));
        Self { value, expires_at }
    }

    /// Whether the entry is still fresh.
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.expires_at > Utc::now()
    }
}

/// Async cache backend.
#[async_trait]
pub trait Cache: Send + Sync {
    /// Get a fresh entry.
    async fn get(&self, key: &CacheKey) -> Result<Option<ConfigValue>>;

    /// Put an entry with TTL.
    async fn put(&self, key: &CacheKey, value: ConfigValue, ttl: Duration) -> Result<()>;

    /// Invalidate a key.
    async fn invalidate(&self, key: &CacheKey) -> Result<()>;

    /// Clear all entries.
    async fn clear(&self) -> Result<()>;
}

/// No-op cache (always miss).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCache;

#[async_trait]
impl Cache for NoopCache {
    async fn get(&self, _key: &CacheKey) -> Result<Option<ConfigValue>> {
        Ok(None)
    }

    async fn put(&self, _key: &CacheKey, _value: ConfigValue, _ttl: Duration) -> Result<()> {
        Ok(())
    }

    async fn invalidate(&self, _key: &CacheKey) -> Result<()> {
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_entry_freshness() {
        let fresh = CacheEntry::with_ttl(ConfigValue::string("x"), Duration::from_secs(60));
        assert!(fresh.is_fresh());
        let expired = CacheEntry {
            value: ConfigValue::string("x"),
            expires_at: Utc::now() - chrono::Duration::seconds(1),
        };
        assert!(!expired.is_fresh());
    }
}
