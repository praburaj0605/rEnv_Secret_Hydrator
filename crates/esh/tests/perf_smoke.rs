//! Loose cached-load smoke (hard gate is `cached_load_budget`).

use env_secret_hydrator::{EnvProvider, Hydrator, MemoryCache};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test]
async fn cached_load_smoke_under_one_second() {
    let _g = env_lock().lock().await;
    std::env::set_var("bench_key", "bench-value");
    let h = Hydrator::builder()
        .provider(EnvProvider::new())
        .cache(MemoryCache::ttl(Duration::from_secs(60)))
        .cache_ttl(Duration::from_secs(60))
        .keys(vec!["bench_key".into()])
        .build()
        .await
        .unwrap();
    let _ = h.load::<serde_json::Value>().await.unwrap();
    let start = Instant::now();
    for _ in 0..50 {
        let _ = h.load::<serde_json::Value>().await.unwrap();
    }
    assert!(start.elapsed() < Duration::from_secs(1));
    std::env::remove_var("bench_key");
}
