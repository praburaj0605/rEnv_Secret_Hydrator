//! Automatic provider selection (FR-002).

use crate::provider::ProviderId;
use std::env;

/// How a provider was selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionSource {
    /// Explicit builder / config order.
    Explicit,
    /// Environment variable hint.
    Environment,
    /// Runtime detection heuristics.
    RuntimeDetect,
}

/// Environment / runtime hints used for detection.
#[derive(Clone, Debug, Default)]
pub struct RuntimeHints {
    /// `AWS_REGION` / `AWS_DEFAULT_REGION` present.
    pub aws: bool,
    /// Azure identity env present.
    pub azure: bool,
    /// GCP project env present.
    pub gcp: bool,
    /// Vault address present.
    pub vault: bool,
    /// Running in Kubernetes.
    pub kubernetes: bool,
    /// Docker secrets directory present.
    pub docker: bool,
}

impl RuntimeHints {
    /// Detect from process environment and common paths.
    #[must_use]
    pub fn detect() -> Self {
        let aws = env::var_os("AWS_REGION").is_some()
            || env::var_os("AWS_DEFAULT_REGION").is_some()
            || env::var_os("AWS_ACCESS_KEY_ID").is_some()
            || env::var_os("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI").is_some()
            || env::var_os("AWS_WEB_IDENTITY_TOKEN_FILE").is_some();

        let azure = env::var_os("AZURE_CLIENT_ID").is_some()
            || env::var_os("IDENTITY_ENDPOINT").is_some()
            || env::var_os("MSI_ENDPOINT").is_some();

        let gcp = env::var_os("GOOGLE_CLOUD_PROJECT").is_some()
            || env::var_os("GCP_PROJECT").is_some()
            || env::var_os("GOOGLE_APPLICATION_CREDENTIALS").is_some();

        let vault = env::var_os("VAULT_ADDR").is_some();

        let kubernetes = std::path::Path::new("/var/run/secrets/kubernetes.io").exists()
            || env::var_os("KUBERNETES_SERVICE_HOST").is_some();

        let docker = std::path::Path::new("/run/secrets").is_dir();

        Self {
            aws,
            azure,
            gcp,
            vault,
            kubernetes,
            docker,
        }
    }

    /// Ordered provider ids suggested by detection (cloud first, then local).
    #[must_use]
    pub fn suggested_providers(&self) -> Vec<ProviderId> {
        let mut out = Vec::new();
        if self.aws {
            out.push(ProviderId::new("aws-secrets-manager"));
            out.push(ProviderId::new("aws-ssm"));
        }
        if self.azure {
            out.push(ProviderId::new("azure-key-vault"));
        }
        if self.gcp {
            out.push(ProviderId::new("gcp-secret-manager"));
        }
        if self.vault {
            out.push(ProviderId::new("vault"));
        }
        if self.kubernetes {
            out.push(ProviderId::new("kubernetes"));
        }
        if self.docker {
            out.push(ProviderId::new("docker"));
        }
        out.push(ProviderId::new("env"));
        out.push(ProviderId::new("dotenv"));
        out
    }
}

/// Selects and orders providers.
#[derive(Clone, Debug, Default)]
pub struct ProviderSelector {
    /// Explicit preference list (provider ids).
    pub explicit: Vec<ProviderId>,
    /// Whether to append runtime detection after explicit.
    pub detect_runtime: bool,
    /// Optional env var that lists providers (comma-separated), e.g. `ESH_PROVIDERS`.
    pub env_list_var: Option<String>,
}

impl ProviderSelector {
    /// Build the ordered provider id list and the selection source used for the first entry.
    #[must_use]
    pub fn select(&self) -> (Vec<ProviderId>, SelectionSource) {
        if !self.explicit.is_empty() {
            return (self.explicit.clone(), SelectionSource::Explicit);
        }

        if let Some(var) = &self.env_list_var {
            if let Ok(raw) = env::var(var) {
                let list: Vec<ProviderId> = raw
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ProviderId::new)
                    .collect();
                if !list.is_empty() {
                    return (list, SelectionSource::Environment);
                }
            }
        }

        if self.detect_runtime {
            return (
                RuntimeHints::detect().suggested_providers(),
                SelectionSource::RuntimeDetect,
            );
        }

        (
            vec![ProviderId::new("env"), ProviderId::new("dotenv")],
            SelectionSource::Explicit,
        )
    }
}
