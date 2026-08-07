//! Security leak suite (FR-009) — CI release gate.

use esh::{
    AuditEvent, EnvProvider, Error, EventKind, Hydrator, MemoryAuditor, SecretBytes, SecretString,
};
use esh_test_support::{assert_no_leak, assert_no_leak_in, SAMPLE_SECRET, SAMPLE_TOKEN};
use serde::Deserialize;
use std::sync::OnceLock;
use tokio::sync::Mutex;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn secret_string_debug_display() {
    let s = SecretString::new(SAMPLE_SECRET);
    assert_no_leak(&s, SAMPLE_SECRET);
}

#[test]
fn secret_bytes_debug_display() {
    let s = SecretBytes::new(SAMPLE_TOKEN.as_bytes().to_vec());
    assert_no_leak(&s, SAMPLE_TOKEN);
}

#[test]
fn error_messages_safe() {
    let err = Error::provider("aws", "access denied");
    assert_no_leak_in(&err.to_string(), SAMPLE_SECRET);
    assert_no_leak_in(&format!("{err:?}"), SAMPLE_SECRET);
}

#[tokio::test]
async fn audit_and_hydrator_path_no_leak() {
    let _g = env_lock().lock().await;
    let auditor = MemoryAuditor::new();
    std::env::set_var("password", SAMPLE_TOKEN);

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct C {
        password: SecretString,
    }

    let _ = Hydrator::builder()
        .provider(EnvProvider::new())
        .on_audit(auditor.clone())
        .keys(vec!["password".into()])
        .build()
        .await
        .unwrap()
        .load::<C>()
        .await
        .unwrap();

    let blob = serde_json::to_string(&auditor.events()).unwrap();
    assert_no_leak_in(&blob, SAMPLE_TOKEN);
    assert!(auditor
        .events()
        .iter()
        .any(|e| e.kind == EventKind::SecretLoaded));
    std::env::remove_var("password");
}

#[test]
fn panic_debug_of_secret_string_redacted() {
    let result = std::panic::catch_unwind(|| {
        let s = SecretString::new(SAMPLE_SECRET);
        panic!("cfg={s:?}");
    });
    let payload = result.unwrap_err();
    let msg = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert_no_leak_in(msg, SAMPLE_SECRET);
}

#[test]
fn audit_event_json_no_values() {
    let ev = AuditEvent::new(EventKind::SecretAccessFailure, "database_url")
        .with_provider("vault")
        .with_message("timeout");
    let json = serde_json::to_string(&ev).unwrap();
    assert_no_leak_in(&json, SAMPLE_SECRET);
}
