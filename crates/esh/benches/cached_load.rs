//! Cached configuration load benchmark (NFR: <200ms).

#![allow(missing_docs)]

use criterion::{criterion_group, criterion_main, Criterion};
use esh::{EnvProvider, Hydrator, MemoryCache};
use std::time::Duration;

fn bench_cached_and_cold(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    std::env::set_var("bench_key", "bench-value");

    let hydrator = rt.block_on(async {
        Hydrator::builder()
            .provider(EnvProvider::new())
            .cache(MemoryCache::ttl(Duration::from_secs(300)))
            .cache_ttl(Duration::from_secs(300))
            .keys(vec!["bench_key".into()])
            .build()
            .await
            .unwrap()
    });

    rt.block_on(async {
        let _ = hydrator.load::<serde_json::Value>().await.unwrap();
    });

    c.bench_function("cached_load_p50_target_under_200ms", |b| {
        b.to_async(&rt).iter(|| async {
            let v = hydrator.load::<serde_json::Value>().await.unwrap();
            std::hint::black_box(v);
        });
    });

    let cold = rt.block_on(async {
        Hydrator::builder()
            .provider(EnvProvider::new())
            .keys(vec!["bench_key".into()])
            .build()
            .await
            .unwrap()
    });

    c.bench_function("cold_load_no_cache", |b| {
        b.to_async(&rt).iter(|| async {
            let v = cold.load::<serde_json::Value>().await.unwrap();
            std::hint::black_box(v);
        });
    });

    std::env::remove_var("bench_key");
}

criterion_group!(benches, bench_cached_and_cold);
criterion_main!(benches);
