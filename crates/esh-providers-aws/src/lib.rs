//! AWS Secrets Manager and Systems Manager Parameter Store providers.
//!
//! These providers use an injectable [`AwsSecretsClient`] so unit tests do not
//! require live AWS credentials. Production integrations can wrap the AWS SDK
//! or a signed HTTP client behind the same trait.

use async_trait::async_trait;
use esh_core::{
    ConfigValue, Error, Provider, ProviderCapability, ProviderId, ProviderMeta, Result,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// Minimal client surface for Secrets Manager + SSM.
#[async_trait]
pub trait AwsSecretsClient: Send + Sync {
    /// `GetSecretValue` equivalent. Returns secret string.
    async fn get_secret_value(&self, secret_id: &str) -> Result<String>;

    /// `GetParameter` equivalent (with decryption).
    async fn get_parameter(&self, name: &str) -> Result<String>;
}

/// In-memory fake for tests and local demos.
#[derive(Default, Clone)]
pub struct MapAwsClient {
    secrets: HashMap<String, String>,
    parameters: HashMap<String, String>,
    /// When set, all calls fail with this mode.
    fault: Option<AwsFault>,
}

/// Injected failure mode for [`MapAwsClient`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwsFault {
    /// Simulate auth / access denied.
    Auth,
    /// Simulate request timeout.
    Timeout,
}

impl MapAwsClient {
    /// Create empty client.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a Secrets Manager value.
    #[must_use]
    pub fn with_secret(mut self, id: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(id.into(), value.into());
        self
    }

    /// Insert an SSM parameter.
    #[must_use]
    pub fn with_parameter(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.insert(name.into(), value.into());
        self
    }

    /// Force all subsequent calls to fail.
    #[must_use]
    pub fn with_fault(mut self, fault: AwsFault) -> Self {
        self.fault = Some(fault);
        self
    }

    fn check_fault(&self) -> Result<()> {
        match self.fault {
            Some(AwsFault::Auth) => Err(Error::provider("aws", "access denied")),
            Some(AwsFault::Timeout) => Err(Error::provider("aws", "timeout")),
            None => Ok(()),
        }
    }
}

#[async_trait]
impl AwsSecretsClient for MapAwsClient {
    async fn get_secret_value(&self, secret_id: &str) -> Result<String> {
        self.check_fault()?;
        self.secrets
            .get(secret_id)
            .cloned()
            .ok_or_else(|| Error::not_found(secret_id))
    }

    async fn get_parameter(&self, name: &str) -> Result<String> {
        self.check_fault()?;
        self.parameters
            .get(name)
            .cloned()
            .ok_or_else(|| Error::not_found(name))
    }
}

/// AWS Secrets Manager provider.
///
/// Key lookup uses `prefix` + key as the secret id/name. If the secret string
/// is JSON, nested keys are available via `key` matching a top-level field.
pub struct AwsSecretsManagerProvider {
    meta: ProviderMeta,
    client: Arc<dyn AwsSecretsClient>,
    /// Prefix prepended to logical keys (e.g. `prod/app/`).
    prefix: String,
    /// When true, treat secret body as JSON object and extract `key`.
    json_field_mode: bool,
}

impl AwsSecretsManagerProvider {
    /// Create with an injectable client.
    #[must_use]
    pub fn new(client: Arc<dyn AwsSecretsClient>) -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("aws-secrets-manager"),
                name: "AWS Secrets Manager".into(),
                capabilities: ProviderCapability {
                    bulk: false,
                    versioned: true,
                    local: false,
                },
            },
            client,
            prefix: String::new(),
            json_field_mode: false,
        }
    }

    /// Set name prefix.
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Extract JSON object fields by logical key.
    #[must_use]
    pub fn json_fields(mut self) -> Self {
        self.json_field_mode = true;
        self
    }

    fn secret_id(&self, key: &str) -> String {
        format!("{}{key}", self.prefix)
    }
}

#[async_trait]
impl Provider for AwsSecretsManagerProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        if self.json_field_mode {
            // In JSON field mode, `prefix` is the secret id; `key` is a field.
            let secret_id = if self.prefix.is_empty() {
                return Err(Error::InvalidConfig(
                    "aws-secrets-manager json_fields mode requires a secret prefix/id".into(),
                ));
            } else {
                self.prefix.trim_end_matches('/').to_string()
            };
            let body = self.client.get_secret_value(&secret_id).await?;
            let json: JsonValue = serde_json::from_str(&body).map_err(|e| {
                Error::provider(self.meta.id.as_str(), format!("secret is not JSON: {e}"))
            })?;
            let field = json
                .get(key)
                .and_then(|v| v.as_str().map(str::to_string).or_else(|| Some(v.to_string())))
                .ok_or_else(|| Error::not_found(key))?;
            return Ok(ConfigValue::secret(field));
        }

        let id = self.secret_id(key);
        let body = self.client.get_secret_value(&id).await.map_err(|e| match e {
            Error::NotFound { .. } => Error::not_found(key),
            other => other,
        })?;
        Ok(ConfigValue::secret(body))
    }
}

/// AWS SSM Parameter Store provider.
pub struct AwsSsmProvider {
    meta: ProviderMeta,
    client: Arc<dyn AwsSecretsClient>,
    prefix: String,
}

impl AwsSsmProvider {
    /// Create with injectable client.
    #[must_use]
    pub fn new(client: Arc<dyn AwsSecretsClient>) -> Self {
        Self {
            meta: ProviderMeta {
                id: ProviderId::new("aws-ssm"),
                name: "AWS SSM Parameter Store".into(),
                capabilities: ProviderCapability {
                    bulk: false,
                    versioned: true,
                    local: false,
                },
            },
            client,
            prefix: String::new(),
        }
    }

    /// Path prefix (e.g. `/prod/app/`).
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    fn param_name(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else if self.prefix.ends_with('/') {
            format!("{}{key}", self.prefix)
        } else {
            format!("{}/{key}", self.prefix)
        }
    }
}

#[async_trait]
impl Provider for AwsSsmProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    async fn get(&self, key: &str) -> Result<ConfigValue> {
        let name = self.param_name(key);
        let body = self.client.get_parameter(&name).await.map_err(|e| match e {
            Error::NotFound { .. } => Error::not_found(key),
            other => other,
        })?;
        Ok(ConfigValue::secret(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esh_core::ProviderCapability;
    use esh_test_support::{
        run_provider_conformance, ProviderConformanceCfg, SAMPLE_SECRET, SAMPLE_TOKEN,
    };

    #[tokio::test]
    async fn secrets_manager_json_field() {
        let client = Arc::new(
            MapAwsClient::new()
                .with_secret("app/prod", r#"{"database_url":"postgres://aws"}"#),
        );
        let p = AwsSecretsManagerProvider::new(client)
            .with_prefix("app/prod")
            .json_fields();
        let v = p.get("database_url").await.unwrap();
        assert_eq!(v.as_str(), Some("postgres://aws"));
    }

    #[tokio::test]
    async fn ssm_with_prefix() {
        let client = Arc::new(MapAwsClient::new().with_parameter("/prod/app/token", "t1"));
        let p = AwsSsmProvider::new(client).with_prefix("/prod/app");
        assert_eq!(p.get("token").await.unwrap().as_str(), Some("t1"));
    }

    #[tokio::test]
    async fn secrets_manager_conformance() {
        let client = Arc::new(MapAwsClient::new().with_secret("database_url", SAMPLE_SECRET));
        let p = AwsSecretsManagerProvider::new(client);
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "aws-secrets-manager".into(),
                expected_capabilities: ProviderCapability {
                    bulk: false,
                    versioned: true,
                    local: false,
                },
                present_key: "database_url".into(),
                present_value: SAMPLE_SECRET.into(),
                missing_key: "missing_key_xyz".into(),
                fault_key: None,
                health_ok: true,
                test_get_many: true,
            },
        )
        .await;
    }

    #[tokio::test]
    async fn secrets_manager_auth_fault() {
        let client = Arc::new(
            MapAwsClient::new()
                .with_secret("database_url", SAMPLE_SECRET)
                .with_fault(AwsFault::Auth),
        );
        let p = AwsSecretsManagerProvider::new(client);
        let err = p.get("database_url").await.unwrap_err();
        assert!(matches!(err, Error::Provider { .. }));
        assert!(!err.to_string().contains(SAMPLE_SECRET));
    }

    #[tokio::test]
    async fn ssm_conformance() {
        let client = Arc::new(MapAwsClient::new().with_parameter("api_token", SAMPLE_TOKEN));
        let p = AwsSsmProvider::new(client);
        run_provider_conformance(
            &p,
            &ProviderConformanceCfg {
                expected_id: "aws-ssm".into(),
                expected_capabilities: ProviderCapability {
                    bulk: false,
                    versioned: true,
                    local: false,
                },
                present_key: "api_token".into(),
                present_value: SAMPLE_TOKEN.into(),
                missing_key: "nope".into(),
                fault_key: None,
                health_ok: true,
                test_get_many: true,
            },
        )
        .await;
    }
}
