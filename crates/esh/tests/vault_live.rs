//! Live Vault HTTP test (ignored; nightly/main live-backends job).

#![cfg(feature = "vault")]

use env_secret_hydrator::{MapVaultClient, Provider, VaultKvProvider};
use serde_json::json;
use std::sync::Arc;

/// Scaffold: when `ESH_VAULT_ADDR` is set, operators can extend this with a real HTTP client.
/// For now, validates the ignored harness wiring with the map client under ignore.
#[tokio::test]
#[ignore = "live"]
async fn vault_kv_live_or_map_smoke() {
    if let Ok(addr) = std::env::var("ESH_VAULT_ADDR") {
        // Placeholder for thin HTTP VaultClient — assert env is present in live job.
        assert!(addr.starts_with("http"));
        return;
    }
    let client = Arc::new(MapVaultClient::new().with_secret(
        "secret",
        "app",
        json!({"token": "live-map"}),
    ));
    let p = VaultKvProvider::new(client).secret_path("app");
    assert_eq!(p.get("token").await.unwrap().as_str(), Some("live-map"));
}
