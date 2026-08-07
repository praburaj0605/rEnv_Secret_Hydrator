//! Shared Provider conformance runner (FR-001).

use crate::leak::assert_no_leak_in;
use esh_core::{Error, Provider, ProviderCapability};

/// Configuration for a provider under test.
pub struct ProviderConformanceCfg {
    /// Expected provider id (`meta().id`).
    pub expected_id: String,
    /// Expected capability flags.
    pub expected_capabilities: ProviderCapability,
    /// Key that must resolve successfully.
    pub present_key: String,
    /// Expected plaintext for `present_key` (used for equality; also leak needle).
    pub present_value: String,
    /// Key that must return NotFound.
    pub missing_key: String,
    /// When set, provider is expected to fail `get` with Provider/Unavailable for this key.
    pub fault_key: Option<String>,
    /// Expect health() to succeed.
    pub health_ok: bool,
    /// When true, run get_many checks.
    pub test_get_many: bool,
}

/// Run the standard provider contract suite against `provider`.
pub async fn run_provider_conformance(provider: &dyn Provider, cfg: &ProviderConformanceCfg) {
    let meta = provider.meta();
    assert_eq!(meta.id.as_str(), cfg.expected_id);
    assert_eq!(meta.capabilities.bulk, cfg.expected_capabilities.bulk);
    assert_eq!(meta.capabilities.local, cfg.expected_capabilities.local);
    assert_eq!(
        meta.capabilities.versioned,
        cfg.expected_capabilities.versioned
    );

    // 1. Happy path
    let got = provider
        .get(&cfg.present_key)
        .await
        .unwrap_or_else(|e| panic!("happy-path get failed: {e}"));
    assert_eq!(got.as_str(), Some(cfg.present_value.as_str()));

    // 2. Missing key
    let missing = provider.get(&cfg.missing_key).await;
    match missing {
        Err(Error::NotFound { key }) => {
            assert_eq!(key, cfg.missing_key);
            assert_no_leak_in(&key, &cfg.present_value);
        }
        other => panic!("expected NotFound, got {other:?}"),
    }

    // 3/4. Fault path (optional — separate FaultyProvider tests cover modes)
    if let Some(fault_key) = &cfg.fault_key {
        let err = provider.get(fault_key).await.expect_err("expected fault");
        let s = err.to_string();
        assert_no_leak_in(&s, &cfg.present_value);
        match &err {
            Error::Provider { .. } | Error::ProviderUnavailable { .. } => {}
            other => panic!("expected Provider/Unavailable fault, got {other:?}"),
        }
    }

    // 5. Health
    match provider.health().await {
        Ok(()) => assert!(cfg.health_ok, "health succeeded but health_ok=false"),
        Err(e) => {
            assert!(!cfg.health_ok, "health failed unexpectedly: {e}");
            assert_no_leak_in(&e.to_string(), &cfg.present_value);
        }
    }

    // 6. get_many
    if cfg.test_get_many {
        let keys = vec![cfg.present_key.clone(), cfg.missing_key.clone()];
        let many = provider.get_many(&keys).await.expect("get_many");
        assert!(many.contains_key(&cfg.present_key));
        assert!(!many.contains_key(&cfg.missing_key));
    }

    // 8. Leak check on happy-path error formatting for not-found
    let nf = Error::not_found(&cfg.present_key);
    assert_no_leak_in(&nf.to_string(), &cfg.present_value);
}
