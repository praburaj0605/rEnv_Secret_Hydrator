//! Hydrator builder, typed load, and runtime watch (FR-006 / FR-007).

use crate::audit::{Auditor, NoopAuditor};
use crate::cache::{Cache, NoopCache};
use crate::error::{Error, Result};
use crate::provider::{Provider, ProviderId};
use crate::resolver::{ResolveOptions, Resolver};
use crate::rotate::{FailurePolicy, RefreshConfig};
use crate::select::ProviderSelector;
use crate::validate::Validator;
use crate::value::{map_to_json, ConfigMap};
use arc_swap::ArcSwap;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Local fallback mode (FR-005).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fallback {
    /// Cloud → env → dotenv → defaults.
    #[default]
    Local,
    /// No special reordering; use provider list as given.
    None,
}

/// Live configuration handle updated by background refresh.
#[derive(Clone)]
pub struct WatchedConfig<T> {
    inner: Arc<ArcSwap<T>>,
    rx: watch::Receiver<Arc<T>>,
}

impl<T> WatchedConfig<T> {
    /// Snapshot the current configuration.
    #[must_use]
    pub fn load(&self) -> Arc<T> {
        self.inner.load_full()
    }

    /// Borrow the watch receiver (notifies on refresh).
    #[must_use]
    pub fn receiver(&self) -> watch::Receiver<Arc<T>> {
        self.rx.clone()
    }
}

/// Built hydrator ready to load typed configuration.
pub struct Hydrator {
    providers: Vec<Arc<dyn Provider>>,
    provider_index: HashMap<String, Arc<dyn Provider>>,
    cache: Arc<dyn Cache>,
    auditor: Arc<dyn Auditor>,
    selector: ProviderSelector,
    fallback: Fallback,
    defaults: ConfigMap,
    keys: Vec<String>,
    validator: Validator,
    cache_ttl: Duration,
    refresh: Option<RefreshConfig>,
    ordered_ids: Vec<ProviderId>,
}

impl Hydrator {
    /// Start a builder.
    #[must_use]
    pub fn builder() -> HydratorBuilder {
        HydratorBuilder::default()
    }

    /// Load and deserialize configuration into `T`.
    pub async fn load<T: DeserializeOwned>(&self) -> Result<Arc<T>> {
        let map = self.resolve_map().await?;
        let json = map_to_json(map);
        self.validator.validate_json(&json)?;
        let value: T = serde_json::from_value(json)
            .map_err(|e| Error::Deserialize(e.to_string()))?;
        Ok(Arc::new(value))
    }

    /// Load once and spawn a background refresh loop publishing updates.
    pub async fn watch<T: DeserializeOwned + Send + Sync + 'static>(
        &self,
    ) -> Result<(WatchedConfig<T>, JoinHandle<()>)> {
        let initial = self.load::<T>().await?;
        let inner = Arc::new(ArcSwap::from(initial.clone()));
        let (tx, rx) = watch::channel(initial);

        let watched = WatchedConfig {
            inner: Arc::clone(&inner),
            rx,
        };

        let refresh = self.refresh.clone().unwrap_or_default();
        let hydrator = self.clone_for_task();
        let failure = refresh.on_failure;

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(refresh.interval);
            ticker.tick().await; // skip immediate tick
            loop {
                ticker.tick().await;
                match hydrator.load::<T>().await {
                    Ok(next) => {
                        inner.store(Arc::clone(&next));
                        let _ = tx.send(next);
                    }
                    Err(e) => match failure {
                        FailurePolicy::KeepLastGood => {
                            tracing::warn!(error = %e, "refresh failed; keeping last good config");
                        }
                        FailurePolicy::FailFast => {
                            tracing::error!(error = %e, "refresh failed (fail-fast)");
                        }
                        FailurePolicy::FallbackLocal => {
                            tracing::warn!(error = %e, "refresh failed; attempting local-only");
                            // Local preference already encoded in resolver options when enabled.
                        }
                    },
                }
            }
        });

        Ok((watched, handle))
    }

    async fn resolve_map(&self) -> Result<ConfigMap> {
        let ordered = self.ordered_providers();
        let resolver = Resolver::new(
            ordered,
            Arc::clone(&self.cache),
            Arc::clone(&self.auditor),
        );
        let options = ResolveOptions {
            keys: self.keys.clone(),
            defaults: self.defaults.clone(),
            cache_ttl: self.cache_ttl,
            retry: self
                .refresh
                .as_ref()
                .map(|r| r.retry.clone())
                .unwrap_or_default(),
            prefer_local_on_failure: matches!(self.fallback, Fallback::Local)
                || self
                    .refresh
                    .as_ref()
                    .is_some_and(|r| r.on_failure == FailurePolicy::FallbackLocal),
        };
        resolver.resolve(&options).await
    }

    fn ordered_providers(&self) -> Vec<Arc<dyn Provider>> {
        let (ids, _) = self.selector.select();
        let mut ordered = Vec::new();
        let mut used = std::collections::HashSet::new();

        let id_list = if ids.is_empty() {
            self.ordered_ids.clone()
        } else {
            ids
        };

        for id in &id_list {
            if let Some(p) = self.provider_index.get(id.as_str()) {
                ordered.push(Arc::clone(p));
                used.insert(id.as_str().to_string());
            }
        }

        // Append any registered providers not mentioned (stable registration order).
        for p in &self.providers {
            let id = p.meta().id.as_str();
            if !used.contains(id) {
                // When Fallback::Local, push local providers toward the end if cloud listed first —
                // registration order already typically cloud then local.
                ordered.push(Arc::clone(p));
            }
        }

        if matches!(self.fallback, Fallback::Local) {
            ordered.sort_by_key(|p| u8::from(p.meta().capabilities.local));
            // false (cloud) sorts before true (local) — good: cloud first, local fallback.
        }

        ordered
    }

    fn clone_for_task(&self) -> Self {
        Self {
            providers: self.providers.clone(),
            provider_index: self.provider_index.clone(),
            cache: Arc::clone(&self.cache),
            auditor: Arc::clone(&self.auditor),
            selector: self.selector.clone(),
            fallback: self.fallback,
            defaults: self.defaults.clone(),
            keys: self.keys.clone(),
            validator: self.validator.clone(),
            cache_ttl: self.cache_ttl,
            refresh: self.refresh.clone(),
            ordered_ids: self.ordered_ids.clone(),
        }
    }
}

/// Builder for [`Hydrator`].
pub struct HydratorBuilder {
    providers: Vec<Arc<dyn Provider>>,
    cache: Arc<dyn Cache>,
    auditor: Arc<dyn Auditor>,
    selector: ProviderSelector,
    fallback: Fallback,
    defaults: ConfigMap,
    keys: Vec<String>,
    validator: Validator,
    cache_ttl: Duration,
    refresh: Option<RefreshConfig>,
}

impl Default for HydratorBuilder {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            cache: Arc::new(NoopCache),
            auditor: Arc::new(NoopAuditor),
            selector: ProviderSelector {
                explicit: Vec::new(),
                detect_runtime: false,
                env_list_var: Some("ESH_PROVIDERS".into()),
            },
            fallback: Fallback::Local,
            defaults: ConfigMap::new(),
            keys: Vec::new(),
            validator: Validator::new(),
            cache_ttl: Duration::from_secs(300),
            refresh: None,
        }
    }
}

impl HydratorBuilder {
    /// Register a provider.
    #[must_use]
    pub fn provider(mut self, provider: impl Provider + 'static) -> Self {
        self.providers.push(Arc::new(provider));
        self
    }

    /// Register a shared provider.
    #[must_use]
    pub fn provider_arc(mut self, provider: Arc<dyn Provider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Enable runtime detection for provider ordering.
    #[must_use]
    pub fn detect_runtime(mut self) -> Self {
        self.selector.detect_runtime = true;
        self
    }

    /// Explicit provider id order.
    #[must_use]
    pub fn providers_order(mut self, ids: Vec<ProviderId>) -> Self {
        self.selector.explicit = ids;
        self
    }

    /// Local fallback mode.
    #[must_use]
    pub fn with_fallback(mut self, fallback: Fallback) -> Self {
        self.fallback = fallback;
        self
    }

    /// Set cache backend.
    #[must_use]
    pub fn cache(mut self, cache: impl Cache + 'static) -> Self {
        self.cache = Arc::new(cache);
        self
    }

    /// Set shared cache.
    #[must_use]
    pub fn cache_arc(mut self, cache: Arc<dyn Cache>) -> Self {
        self.cache = cache;
        self
    }

    /// Cache TTL for resolved secrets.
    #[must_use]
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Audit sink.
    #[must_use]
    pub fn on_audit(mut self, auditor: impl Auditor + 'static) -> Self {
        self.auditor = Arc::new(auditor);
        self
    }

    /// Default values (last resort).
    #[must_use]
    pub fn defaults(mut self, defaults: ConfigMap) -> Self {
        self.defaults = defaults;
        self
    }

    /// Keys to resolve. If empty, bulk/provider-specific behavior applies.
    #[must_use]
    pub fn keys(mut self, keys: Vec<String>) -> Self {
        self.keys = keys;
        self
    }

    /// Attach validator.
    #[must_use]
    pub fn validator(mut self, validator: Validator) -> Self {
        self.validator = validator;
        self
    }

    /// Enable background refresh for `watch`.
    #[must_use]
    pub fn refresh(mut self, config: RefreshConfig) -> Self {
        self.refresh = Some(config);
        self
    }

    /// Build the hydrator.
    pub async fn build(self) -> Result<Hydrator> {
        if self.providers.is_empty() {
            return Err(Error::InvalidConfig(
                "at least one provider must be registered".into(),
            ));
        }

        let mut provider_index = HashMap::new();
        let mut ordered_ids = Vec::new();
        for p in &self.providers {
            let id = p.meta().id.as_str().to_string();
            ordered_ids.push(ProviderId::new(&id));
            provider_index.insert(id, Arc::clone(p));
        }

        Ok(Hydrator {
            providers: self.providers,
            provider_index,
            cache: self.cache,
            auditor: self.auditor,
            selector: self.selector,
            fallback: self.fallback,
            defaults: self.defaults,
            keys: self.keys,
            validator: self.validator,
            cache_ttl: self.cache_ttl,
            refresh: self.refresh,
            ordered_ids,
        })
    }
}
