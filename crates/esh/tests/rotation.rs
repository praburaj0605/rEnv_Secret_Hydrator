//! Rotation / watch integration tests (FR-004 / FR-007).

use esh::{
    ConfigValue, EnvProvider, FailurePolicy, Hydrator, Provider, ProviderCapability, ProviderId,
    ProviderMeta, RefreshConfig, RetryPolicy, SecretString,
};
use esh_test_support::SAMPLE_SECRET;
use parking_lot::Mutex;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

fn env_lock() -> &'static AsyncMutex<()> {
    static LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| AsyncMutex::new(()))
}

#[derive(Debug, Deserialize)]
struct Cfg {
    database_url: SecretString,
}

struct DynMap {
    values: Arc<Mutex<HashMap<String, String>>>,
    meta: ProviderMeta,
}

#[async_trait::async_trait]
impl Provider for DynMap {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> esh::Result<ConfigValue> {
        self.values
            .lock()
            .get(key)
            .cloned()
            .map(ConfigValue::secret)
            .ok_or_else(|| esh::Error::not_found(key))
    }
}

#[tokio::test]
async fn watch_publishes_updates() {
    let values = Arc::new(Mutex::new(HashMap::from([(
        "database_url".into(),
        SAMPLE_SECRET.to_string(),
    )])));
    let provider = DynMap {
        meta: ProviderMeta {
            id: ProviderId::new("dyn"),
            name: "dyn".into(),
            capabilities: ProviderCapability {
                bulk: true,
                versioned: false,
                local: true,
            },
        },
        values: values.clone(),
    };

    let h = Hydrator::builder()
        .provider(provider)
        .keys(vec!["database_url".into()])
        .refresh(RefreshConfig {
            interval: Duration::from_millis(30),
            retry: RetryPolicy {
                max_attempts: 1,
                jitter: false,
                ..RetryPolicy::default()
            },
            on_failure: FailurePolicy::KeepLastGood,
        })
        .build()
        .await
        .unwrap();

    let (watched, handle) = h.watch::<Cfg>().await.unwrap();
    assert_eq!(watched.load().database_url.expose(), SAMPLE_SECRET);

    values
        .lock()
        .insert("database_url".into(), "postgres://rotated".into());
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(watched.load().database_url.expose(), "postgres://rotated");
    handle.abort();
}

#[tokio::test]
async fn watch_keep_last_good() {
    let values = Arc::new(Mutex::new(HashMap::from([(
        "database_url".into(),
        SAMPLE_SECRET.to_string(),
    )])));
    let provider = DynMap {
        meta: ProviderMeta {
            id: ProviderId::new("dyn"),
            name: "dyn".into(),
            capabilities: ProviderCapability {
                bulk: true,
                versioned: false,
                local: true,
            },
        },
        values: values.clone(),
    };

    let h = Hydrator::builder()
        .provider(provider)
        .keys(vec!["database_url".into()])
        .refresh(RefreshConfig {
            interval: Duration::from_millis(30),
            retry: RetryPolicy {
                max_attempts: 1,
                ..RetryPolicy::default()
            },
            on_failure: FailurePolicy::KeepLastGood,
        })
        .build()
        .await
        .unwrap();

    let (watched, handle) = h.watch::<Cfg>().await.unwrap();
    values.lock().clear();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(watched.load().database_url.expose(), SAMPLE_SECRET);
    handle.abort();
}

#[tokio::test]
async fn watch_initial_from_env() {
    let _g = env_lock().lock().await;
    std::env::set_var("database_url", "postgres://watch");
    let h = Hydrator::builder()
        .provider(EnvProvider::new())
        .keys(vec!["database_url".into()])
        .refresh(RefreshConfig {
            interval: Duration::from_millis(50),
            retry: RetryPolicy {
                max_attempts: 1,
                ..RetryPolicy::default()
            },
            ..RefreshConfig::default()
        })
        .build()
        .await
        .unwrap();
    let (watched, handle) = h.watch::<Cfg>().await.unwrap();
    assert_eq!(watched.load().database_url.expose(), "postgres://watch");
    handle.abort();
    std::env::remove_var("database_url");
}
