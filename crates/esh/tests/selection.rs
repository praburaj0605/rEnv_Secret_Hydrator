//! Provider selection integration tests (FR-002).

use env_secret_hydrator::{EnvProvider, Hydrator, ProviderId};
use esh_test_support::MapProvider;
use std::sync::OnceLock;
use tokio::sync::Mutex;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test]
async fn explicit_providers_order() {
    let h = Hydrator::builder()
        .provider(MapProvider::new("env", true).with("k", "env-val"))
        .provider(MapProvider::new("dotenv", true).with("k", "dotenv-val"))
        .providers_order(vec![ProviderId::new("dotenv"), ProviderId::new("env")])
        .keys(vec!["k".into()])
        .build()
        .await
        .unwrap();
    let v = h.load::<serde_json::Value>().await.unwrap();
    assert_eq!(v["k"], "dotenv-val");
}

#[tokio::test]
async fn detect_runtime_builds() {
    let _g = env_lock().lock().await;
    let h = Hydrator::builder()
        .provider(EnvProvider::new())
        .detect_runtime()
        .keys(vec!["missing_unlikely_key_zzz".into()])
        .defaults({
            let mut m = env_secret_hydrator::ConfigMap::new();
            m.insert(
                "missing_unlikely_key_zzz".into(),
                env_secret_hydrator::ConfigValue::string("d"),
            );
            m
        })
        .build()
        .await
        .unwrap();
    let v = h.load::<serde_json::Value>().await.unwrap();
    assert_eq!(v["missing_unlikely_key_zzz"], "d");
}
