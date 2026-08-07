//! Audit sinks (FR-010). Events never include secret values.

use async_trait::async_trait;
use esh_core::{AuditEvent, Auditor};
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, warn};

/// Logs audit events as JSON via `tracing`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TracingAuditor;

#[async_trait]
impl Auditor for TracingAuditor {
    async fn record(&self, event: AuditEvent) {
        match serde_json::to_string(&event) {
            Ok(json) => info!(target: "esh_audit", %json, "audit"),
            Err(e) => warn!(error = %e, "failed to serialize audit event"),
        }
    }
}

/// Writes each event as a JSON line to stdout.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdoutAuditor;

#[async_trait]
impl Auditor for StdoutAuditor {
    async fn record(&self, event: AuditEvent) {
        if let Ok(json) = serde_json::to_string(&event) {
            println!("{json}");
        }
    }
}

/// Collects events in memory (tests).
#[derive(Clone, Default)]
pub struct MemoryAuditor {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl MemoryAuditor {
    /// Create empty auditor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot recorded events.
    #[must_use]
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().clone()
    }
}

#[async_trait]
impl Auditor for MemoryAuditor {
    async fn record(&self, event: AuditEvent) {
        self.events.lock().push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_core::EventKind;
    use esh_test_support::{assert_no_leak_in, SAMPLE_SECRET};

    #[tokio::test]
    async fn memory_auditor_records_without_values() {
        let a = MemoryAuditor::new();
        a.record(AuditEvent::new(EventKind::SecretLoaded, "db_url").with_provider("env"))
            .await;
        let ev = &a.events()[0];
        let json = serde_json::to_string(ev).unwrap();
        assert!(json.contains("db_url"));
        assert!(json.contains("secret_loaded"));
        assert!(!json.contains("postgres://"));
    }

    #[tokio::test]
    async fn all_event_kinds_serialize_without_secret_values() {
        let a = MemoryAuditor::new();
        for kind in [
            EventKind::SecretLoaded,
            EventKind::SecretRefreshed,
            EventKind::SecretExpired,
            EventKind::SecretAccessFailure,
        ] {
            a.record(
                AuditEvent::new(kind, "database_url")
                    .with_provider("aws")
                    .with_message("safe"),
            )
            .await;
        }
        let blob = serde_json::to_string(&a.events()).unwrap();
        assert_no_leak_in(&blob, SAMPLE_SECRET);
        assert!(blob.contains("secret_loaded"));
        assert!(blob.contains("secret_refreshed"));
        assert!(blob.contains("secret_expired"));
        assert!(blob.contains("secret_access_failure"));
    }

    #[tokio::test]
    async fn tracing_and_stdout_accept_events() {
        TracingAuditor
            .record(AuditEvent::new(EventKind::SecretLoaded, "k"))
            .await;
        StdoutAuditor
            .record(AuditEvent::new(EventKind::SecretRefreshed, "k"))
            .await;
    }
}
