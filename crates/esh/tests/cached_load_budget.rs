//! Cached load p95 budget gate (NFR: <200ms). Linux CI asserts; other OS informational.

use env_secret_hydrator::{EnvProvider, Hydrator, MemoryCache};
use std::time::{Duration, Instant};

#[tokio::test]
async fn cached_load_p95_under_200ms() {
    std::env::set_var("budget_key", "budget-value");
    let h = Hydrator::builder()
        .provider(EnvProvider::new())
        .cache(MemoryCache::ttl(Duration::from_secs(300)))
        .cache_ttl(Duration::from_secs(300))
        .keys(vec!["budget_key".into()])
        .build()
        .await
        .unwrap();

    // Warm cache
    let _ = h.load::<serde_json::Value>().await.unwrap();

    let mut samples = Vec::with_capacity(100);
    for _ in 0..100 {
        let start = Instant::now();
        let v = h.load::<serde_json::Value>().await.unwrap();
        std::hint::black_box(v);
        samples.push(start.elapsed());
    }
    samples.sort();
    let p95 = samples[(samples.len() as f64 * 0.95) as usize];
    eprintln!("cached_load p95 = {p95:?}");

    #[cfg(target_os = "linux")]
    assert!(
        p95 < Duration::from_millis(200),
        "cached load p95 {p95:?} exceeded 200ms budget"
    );

    std::env::remove_var("budget_key");
}
