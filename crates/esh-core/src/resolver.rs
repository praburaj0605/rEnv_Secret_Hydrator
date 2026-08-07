//! Multi-provider resolution chain (FR-001 / FR-005).

use crate::audit::{AuditEvent, Auditor, EventKind};
use crate::cache::{Cache, CacheKey};
use crate::error::{Error, Result};
use crate::provider::Provider;
use crate::rotate::RetryPolicy;
use crate::value::{merge_maps, ConfigMap, ConfigValue};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Options controlling a resolve pass.
#[derive(Clone, Debug)]
pub struct ResolveOptions {
    /// Keys to resolve. Empty means providers may supply bulk defaults only via defaults map.
    pub keys: Vec<String>,
    /// Default values used as last resort.
    pub defaults: ConfigMap,
    /// Cache TTL for successful fetches.
    pub cache_ttl: Duration,
    /// Retry policy per provider fetch.
    pub retry: RetryPolicy,
    /// Prefer local providers when cloud fails (`FailurePolicy::FallbackLocal` path).
    pub prefer_local_on_failure: bool,
}

impl Default for ResolveOptions {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            defaults: ConfigMap::new(),
            cache_ttl: Duration::from_secs(300),
            retry: RetryPolicy::default(),
            prefer_local_on_failure: true,
        }
    }
}

/// Resolves configuration by walking an ordered provider list.
pub struct Resolver {
    providers: Vec<Arc<dyn Provider>>,
    cache: Arc<dyn Cache>,
    auditor: Arc<dyn Auditor>,
}

impl Resolver {
    /// Create a resolver.
    #[must_use]
    pub fn new(
        providers: Vec<Arc<dyn Provider>>,
        cache: Arc<dyn Cache>,
        auditor: Arc<dyn Auditor>,
    ) -> Self {
        Self {
            providers,
            cache,
            auditor,
        }
    }

    /// Resolve all requested keys into a merged [`ConfigMap`].
    pub async fn resolve(&self, options: &ResolveOptions) -> Result<ConfigMap> {
        let mut out = ConfigMap::new();

        if options.keys.is_empty() {
            // Attempt bulk from each provider (best-effort), then defaults.
            for provider in &self.providers {
                match provider.get_many(&[]).await {
                    Ok(map) => merge_maps(&mut out, map),
                    Err(Error::ProviderUnavailable { .. }) => continue,
                    Err(Error::NotFound { .. }) => continue,
                    Err(e) => {
                        self.auditor
                            .record(
                                AuditEvent::new(EventKind::SecretAccessFailure, "*")
                                    .with_provider(provider.meta().id.as_str())
                                    .with_message(e.to_string()),
                            )
                            .await;
                        if !options.prefer_local_on_failure {
                            return Err(e);
                        }
                    }
                }
            }
            merge_maps(&mut out, options.defaults.clone());
            return Ok(out);
        }

        for key in &options.keys {
            let value = self.resolve_key(key, options).await?;
            out.insert(key.clone(), value);
        }

        // Overlay defaults for missing optional keys already handled; merge remaining defaults
        // only for keys not present.
        for (k, v) in &options.defaults {
            out.entry(k.clone()).or_insert_with(|| v.clone());
        }

        Ok(out)
    }

    async fn resolve_key(&self, key: &str, options: &ResolveOptions) -> Result<ConfigValue> {
        let cache_key = CacheKey::new(key);
        if let Ok(Some(hit)) = self.cache.get(&cache_key).await {
            debug!(key, "cache hit");
            return Ok(hit);
        }

        let mut last_err: Option<Error> = None;

        for provider in &self.providers {
            let provider_id = provider.meta().id.as_str().to_string();
            match self.fetch_with_retry(provider.as_ref(), key, &options.retry).await {
                Ok(value) => {
                    let _ = self
                        .cache
                        .put(&cache_key, value.clone(), options.cache_ttl)
                        .await;
                    self.auditor
                        .record(
                            AuditEvent::new(EventKind::SecretLoaded, key)
                                .with_provider(&provider_id),
                        )
                        .await;
                    return Ok(value);
                }
                Err(Error::NotFound { .. }) => continue,
                Err(Error::ProviderUnavailable { .. }) => continue,
                Err(e) => {
                    warn!(provider = %provider_id, key, error = %e, "provider fetch failed");
                    self.auditor
                        .record(
                            AuditEvent::new(EventKind::SecretAccessFailure, key)
                                .with_provider(&provider_id)
                                .with_message(e.to_string()),
                        )
                        .await;
                    last_err = Some(e);
                    if !options.prefer_local_on_failure && !provider.meta().capabilities.local {
                        // continue to next; local still allowed later
                    }
                }
            }
        }

        if let Some(default) = options.defaults.get(key) {
            return Ok(default.clone());
        }

        Err(last_err.unwrap_or_else(|| Error::not_found(key)))
    }

    async fn fetch_with_retry(
        &self,
        provider: &dyn Provider,
        key: &str,
        retry: &RetryPolicy,
    ) -> Result<ConfigValue> {
        let mut attempt = 0u32;
        loop {
            match provider.get(key).await {
                Ok(v) => return Ok(v),
                Err(Error::NotFound { .. }) => return Err(Error::not_found(key)),
                Err(Error::ProviderUnavailable { provider }) => {
                    return Err(Error::ProviderUnavailable { provider });
                }
                Err(e) => {
                    if attempt + 1 >= retry.max_attempts {
                        return Err(e);
                    }
                    let delay = retry.delay_for(attempt);
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }
}
