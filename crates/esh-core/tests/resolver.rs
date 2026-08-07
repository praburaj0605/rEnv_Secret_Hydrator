//! Resolver unit/integration tests.

use esh_core::{
    AuditEvent, Auditor, ConfigValue, EventKind, NoopAuditor, NoopCache, ResolveOptions, Resolver,
};
use esh_test_support::{
    CountingProvider, FaultMode, FaultyProvider, MapProvider, SAMPLE_SECRET,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn resolves_from_first_provider() {
    let p1 = Arc::new(MapProvider::new("cloud", false).with("database_url", SAMPLE_SECRET));
    let p2 = Arc::new(MapProvider::new("env", true).with("database_url", "local"));
    let counting = Arc::new(CountingProvider::new(p1));
    let resolver = Resolver::new(
        vec![counting.clone(), p2],
        Arc::new(NoopCache),
        Arc::new(NoopAuditor),
    );
    let opts = ResolveOptions {
        keys: vec!["database_url".into()],
        ..Default::default()
    };
    let map = resolver.resolve(&opts).await.unwrap();
    assert_eq!(map["database_url"].as_str(), Some(SAMPLE_SECRET));
    assert_eq!(counting.get_count(), 1);
}

#[tokio::test]
async fn cache_hit_skips_second_provider_fetch() {
    let inner = Arc::new(MapProvider::new("cloud", false).with("k", "v1"));
    let counting = Arc::new(CountingProvider::new(inner));

    use async_trait::async_trait;
    use esh_core::{Cache, CacheKey, Result};
    use parking_lot::Mutex;
    use std::collections::HashMap;

    struct Mem {
        m: Mutex<HashMap<String, ConfigValue>>,
    }

    #[async_trait]
    impl Cache for Mem {
        async fn get(&self, key: &CacheKey) -> Result<Option<ConfigValue>> {
            Ok(self.m.lock().get(&key.key).cloned())
        }
        async fn put(&self, key: &CacheKey, value: ConfigValue, _ttl: Duration) -> Result<()> {
            self.m.lock().insert(key.key.clone(), value);
            Ok(())
        }
        async fn invalidate(&self, key: &CacheKey) -> Result<()> {
            self.m.lock().remove(&key.key);
            Ok(())
        }
        async fn clear(&self) -> Result<()> {
            self.m.lock().clear();
            Ok(())
        }
    }

    let cache = Arc::new(Mem {
        m: Mutex::new(HashMap::new()),
    });
    let resolver = Resolver::new(
        vec![counting.clone()],
        cache,
        Arc::new(NoopAuditor),
    );
    let opts = ResolveOptions {
        keys: vec!["k".into()],
        cache_ttl: Duration::from_secs(60),
        ..Default::default()
    };
    let _ = resolver.resolve(&opts).await.unwrap();
    let _ = resolver.resolve(&opts).await.unwrap();
    assert_eq!(counting.get_count(), 1, "second resolve must be cache hit");
}

#[tokio::test]
async fn defaults_used_when_missing() {
    let p = Arc::new(MapProvider::new("env", true));
    let resolver = Resolver::new(vec![p], Arc::new(NoopCache), Arc::new(NoopAuditor));
    let mut defaults = esh_core::ConfigMap::new();
    defaults.insert(
        "database_url".into(),
        ConfigValue::secret("postgres://default"),
    );
    let opts = ResolveOptions {
        keys: vec!["database_url".into()],
        defaults,
        ..Default::default()
    };
    let map = resolver.resolve(&opts).await.unwrap();
    assert_eq!(map["database_url"].as_str(), Some("postgres://default"));
}

#[tokio::test]
async fn soft_failure_continues_to_local() {
    let bad = Arc::new(FaultyProvider::new("aws", FaultMode::Auth));
    let good = Arc::new(MapProvider::new("env", true).with("k", "ok"));
    let resolver = Resolver::new(vec![bad, good], Arc::new(NoopCache), Arc::new(NoopAuditor));
    let opts = ResolveOptions {
        keys: vec!["k".into()],
        prefer_local_on_failure: true,
        ..Default::default()
    };
    let map = resolver.resolve(&opts).await.unwrap();
    assert_eq!(map["k"].as_str(), Some("ok"));
}

struct CollectingAuditor {
    events: parking_lot::Mutex<Vec<AuditEvent>>,
}

#[async_trait::async_trait]
impl Auditor for CollectingAuditor {
    async fn record(&self, event: AuditEvent) {
        self.events.lock().push(event);
    }
}

#[tokio::test]
async fn audit_records_access_failure() {
    let bad = Arc::new(FaultyProvider::new("aws", FaultMode::Timeout));
    let auditor = Arc::new(CollectingAuditor {
        events: parking_lot::Mutex::new(Vec::new()),
    });
    let resolver = Resolver::new(vec![bad], Arc::new(NoopCache), auditor.clone());
    let opts = ResolveOptions {
        keys: vec!["k".into()],
        prefer_local_on_failure: true,
        ..Default::default()
    };
    let _ = resolver.resolve(&opts).await;
    let events = auditor.events.lock().clone();
    assert!(events
        .iter()
        .any(|e| e.kind == EventKind::SecretAccessFailure));
}
