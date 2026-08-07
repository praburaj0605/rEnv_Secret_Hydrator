//! Redis cache live tests (ignored by default; run with --ignored in live CI).

#![cfg(feature = "redis-cache")]

use esh_cache::RedisCache;
use esh_test_support::{run_cache_conformance, CacheConformanceCfg};
use std::time::Duration;

#[tokio::test]
#[ignore = "live"]
async fn redis_cache_conformance_live() {
    let url = std::env::var("ESH_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let cache = RedisCache::connect(&url)
        .await
        .expect("connect redis")
        .with_prefix("esh-test:");
    run_cache_conformance(
        &cache,
        &CacheConformanceCfg {
            supports_clear: false,
            short_ttl: Duration::from_millis(200),
            test_expiry: true,
        },
    )
    .await;
}
