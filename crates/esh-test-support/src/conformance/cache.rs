//! Shared Cache conformance runner (FR-003).

use esh_core::{Cache, CacheKey, ConfigValue};
use std::time::Duration;

/// Cache conformance options.
pub struct CacheConformanceCfg {
    /// Whether `clear()` is supported (Redis returns error).
    pub supports_clear: bool,
    /// Short TTL used for expiry tests.
    pub short_ttl: Duration,
    /// Whether to wait for TTL expiry (skip if time cannot be paused reliably).
    pub test_expiry: bool,
}

impl Default for CacheConformanceCfg {
    fn default() -> Self {
        Self {
            supports_clear: true,
            short_ttl: Duration::from_millis(50),
            test_expiry: true,
        }
    }
}

/// Run put/get/invalidate/(clear)/expiry against any [`Cache`].
pub async fn run_cache_conformance(cache: &dyn Cache, cfg: &CacheConformanceCfg) {
    let key = CacheKey::new("conformance_key");
    let value = ConfigValue::secret("cache-secret-needle");

    cache
        .put(&key, value.clone(), Duration::from_secs(60))
        .await
        .expect("put");
    let got = cache.get(&key).await.expect("get").expect("cache hit");
    assert_eq!(got.as_str(), Some("cache-secret-needle"));

    cache.invalidate(&key).await.expect("invalidate");
    assert!(cache.get(&key).await.expect("get after invalidate").is_none());

    cache
        .put(&key, value.clone(), Duration::from_secs(60))
        .await
        .expect("put2");

    if cfg.supports_clear {
        cache.clear().await.expect("clear");
        assert!(cache.get(&key).await.expect("get after clear").is_none());
    }

    if cfg.test_expiry {
        cache
            .put(&key, value, cfg.short_ttl)
            .await
            .expect("put short ttl");
        tokio::time::sleep(cfg.short_ttl + Duration::from_millis(30)).await;
        assert!(
            cache.get(&key).await.expect("get expired").is_none(),
            "expected TTL expiry"
        );
    }
}
