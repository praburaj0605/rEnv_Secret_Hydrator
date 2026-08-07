//! Rotation and hydrator builder tests.

use esh_core::{
    ConfigValue, FailurePolicy, Fallback, Hydrator, ProviderId, RefreshConfig, RetryPolicy,
    SecretString,
};
use esh_test_support::{FaultMode, FaultyProvider, MapProvider, SAMPLE_SECRET};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn retry_delay_respects_max() {
    let policy = RetryPolicy {
        max_attempts: 5,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(250),
        jitter: false,
    };
    let d0 = policy.delay_for(0);
    let d3 = policy.delay_for(3);
    assert!(d0 <= policy.max_delay);
    assert!(d3 <= policy.max_delay);
    assert!(d3 >= d0 || d3 == policy.max_delay);
}

#[test]
fn failure_policy_default_is_keep_last_good() {
    assert_eq!(FailurePolicy::default(), FailurePolicy::KeepLastGood);
}

#[tokio::test]
async fn builder_rejects_empty_providers() {
    match Hydrator::builder().build().await {
        Err(e) => assert!(e.to_string().contains("at least one provider")),
        Ok(_) => panic!("expected error"),
    }
}

#[derive(Debug, Deserialize)]
struct Cfg {
    database_url: SecretString,
}

#[tokio::test]
async fn typed_load_and_fallback_order() {
    let cloud = MapProvider::new("aws", false); // empty
    let local = MapProvider::new("env", true).with("database_url", SAMPLE_SECRET);
    let h = Hydrator::builder()
        .provider(cloud)
        .provider(local)
        .with_fallback(Fallback::Local)
        .keys(vec!["database_url".into()])
        .build()
        .await
        .unwrap();
    let cfg = h.load::<Cfg>().await.unwrap();
    assert_eq!(cfg.database_url.expose(), SAMPLE_SECRET);
    assert!(!format!("{:?}", cfg.database_url).contains(SAMPLE_SECRET));
}

#[tokio::test]
async fn typed_deserialize_error() {
    let p = MapProvider::new("env", true).with("database_url", SAMPLE_SECRET);
    let h = Hydrator::builder()
        .provider(p)
        .keys(vec!["database_url".into()])
        .build()
        .await
        .unwrap();

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Bad {
        database_url: i64,
    }

    match h.load::<Bad>().await {
        Err(esh_core::Error::Deserialize(_)) => {}
        Err(e) => {
            assert!(!e.to_string().contains(SAMPLE_SECRET));
            panic!("unexpected error variant: {e}");
        }
        Ok(_) => panic!("expected deserialize error"),
    }
}

#[tokio::test]
async fn watch_keep_last_good_on_refresh_failure() {
    use async_trait::async_trait;
    use esh_core::{Provider, ProviderCapability, ProviderMeta, Result};
    use parking_lot::Mutex;
    use std::collections::HashMap;

    struct DynMap {
        values: Arc<Mutex<HashMap<String, String>>>,
        meta: ProviderMeta,
    }

    #[async_trait]
    impl Provider for DynMap {
        fn meta(&self) -> &ProviderMeta {
            &self.meta
        }
        async fn get(&self, key: &str) -> Result<ConfigValue> {
            let guard = self.values.lock();
            guard
                .get(key)
                .cloned()
                .map(ConfigValue::secret)
                .ok_or_else(|| esh_core::Error::not_found(key))
        }
    }

    let values = Arc::new(Mutex::new(HashMap::from([(
        "database_url".into(),
        SAMPLE_SECRET.to_string(),
    )])));

    let dyn_p = DynMap {
        meta: ProviderMeta {
            id: ProviderId::new("env"),
            name: "env".into(),
            capabilities: ProviderCapability {
                bulk: true,
                versioned: false,
                local: true,
            },
        },
        values: values.clone(),
    };

    let h = Hydrator::builder()
        .provider(dyn_p)
        .keys(vec!["database_url".into()])
        .refresh(RefreshConfig {
            interval: Duration::from_millis(40),
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
    assert_eq!(watched.load().database_url.expose(), SAMPLE_SECRET);

    values.lock().remove("database_url");
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert_eq!(watched.load().database_url.expose(), SAMPLE_SECRET);
    handle.abort();
}

#[tokio::test]
async fn explicit_provider_order() {
    let h = Hydrator::builder()
        .provider(MapProvider::new("env", true).with("k", "from-env"))
        .provider(MapProvider::new("dotenv", true).with("k", "from-dotenv"))
        .providers_order(vec![ProviderId::new("dotenv"), ProviderId::new("env")])
        .keys(vec!["k".into()])
        .build()
        .await
        .unwrap();
    let v = h.load::<serde_json::Value>().await.unwrap();
    assert_eq!(v["k"], "from-dotenv");
}

#[tokio::test]
async fn faulty_provider_alone_errors() {
    let h = Hydrator::builder()
        .provider(FaultyProvider::new("aws", FaultMode::Unavailable))
        .keys(vec!["k".into()])
        .build()
        .await
        .unwrap();
    let err = h.load::<serde_json::Value>().await;
    match err {
        Err(esh_core::Error::ProviderUnavailable { .. })
        | Err(esh_core::Error::NotFound { .. })
        | Err(esh_core::Error::Provider { .. }) => {}
        other => panic!("unexpected: {other:?}"),
    }
}
