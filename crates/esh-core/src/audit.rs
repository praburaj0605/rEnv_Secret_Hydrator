//! Audit logging traits and events (FR-010).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Kind of audit event. Never includes secret values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Secret successfully loaded.
    SecretLoaded,
    /// Secret refreshed from provider.
    SecretRefreshed,
    /// Cached / known secret expired.
    SecretExpired,
    /// Access / fetch failure.
    SecretAccessFailure,
}

/// Structured audit event. Fields are metadata only.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event kind.
    pub kind: EventKind,
    /// Timestamp (UTC).
    pub at: DateTime<Utc>,
    /// Logical key (never the value).
    pub key: String,
    /// Provider id when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Safe message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl AuditEvent {
    /// Build an event at "now".
    #[must_use]
    pub fn new(kind: EventKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            at: Utc::now(),
            key: key.into(),
            provider: None,
            message: None,
        }
    }

    /// Attach provider id.
    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Attach safe message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

/// Audit sink.
#[async_trait]
pub trait Auditor: Send + Sync {
    /// Record an event.
    async fn record(&self, event: AuditEvent);
}

/// Discards all events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAuditor;

#[async_trait]
impl Auditor for NoopAuditor {
    async fn record(&self, _event: AuditEvent) {}
}
