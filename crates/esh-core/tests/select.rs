//! Provider selection tests (FR-002).

use esh_core::{ProviderId, ProviderSelector, RuntimeHints, SelectionSource};
use std::sync::OnceLock;
use tokio::sync::Mutex;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn explicit_order_wins() {
    let sel = ProviderSelector {
        explicit: vec![ProviderId::new("vault"), ProviderId::new("env")],
        detect_runtime: true,
        env_list_var: Some("ESH_PROVIDERS".into()),
    };
    let (ids, source) = sel.select();
    assert_eq!(source, SelectionSource::Explicit);
    assert_eq!(ids[0].as_str(), "vault");
    assert_eq!(ids[1].as_str(), "env");
}

#[tokio::test]
async fn env_list_var_selection() {
    let _g = env_lock().lock().await;
    std::env::set_var("ESH_PROVIDERS_TEST", "aws-ssm,dotenv");
    let sel = ProviderSelector {
        explicit: vec![],
        detect_runtime: false,
        env_list_var: Some("ESH_PROVIDERS_TEST".into()),
    };
    let (ids, source) = sel.select();
    assert_eq!(source, SelectionSource::Environment);
    assert_eq!(ids[0].as_str(), "aws-ssm");
    assert_eq!(ids[1].as_str(), "dotenv");
    std::env::remove_var("ESH_PROVIDERS_TEST");
}

#[tokio::test]
async fn runtime_detect_includes_env_dotenv() {
    let _g = env_lock().lock().await;
    // Clear cloud hints that might be present in CI.
    for k in [
        "AWS_REGION",
        "AWS_DEFAULT_REGION",
        "AWS_ACCESS_KEY_ID",
        "AZURE_CLIENT_ID",
        "GOOGLE_CLOUD_PROJECT",
        "VAULT_ADDR",
        "KUBERNETES_SERVICE_HOST",
    ] {
        std::env::remove_var(k);
    }
    let hints = RuntimeHints::detect();
    let suggested = hints.suggested_providers();
    assert!(suggested.iter().any(|p| p.as_str() == "env"));
    assert!(suggested.iter().any(|p| p.as_str() == "dotenv"));

    let sel = ProviderSelector {
        explicit: vec![],
        detect_runtime: true,
        env_list_var: None,
    };
    let (_, source) = sel.select();
    assert_eq!(source, SelectionSource::RuntimeDetect);
}
