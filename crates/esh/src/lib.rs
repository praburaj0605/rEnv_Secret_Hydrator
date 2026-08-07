//! # Env-Secret-Hydrator (`env-secret-hydrator`)
//!
//! Unified, provider-agnostic configuration and secret hydration for Rust.
//!
//! ```rust,no_run
//! use env_secret_hydrator::{DotenvProvider, EnvProvider, Fallback, Hydrator, SecretString};
//! use serde::Deserialize;
//! use std::sync::Arc;
//!
//! #[derive(Deserialize)]
//! struct AppConfig {
//!     database_url: SecretString,
//!     #[serde(default)]
//!     region: String,
//! }
//!
//! # async fn demo() -> env_secret_hydrator::Result<Arc<AppConfig>> {
//! let config = Hydrator::builder()
//!     .provider(EnvProvider::new())
//!     .provider(DotenvProvider::from_default()?)
//!     .with_fallback(Fallback::Local)
//!     .keys(vec!["database_url".into(), "region".into()])
//!     .build()
//!     .await?
//!     .load::<AppConfig>()
//!     .await?;
//! Ok(config)
//! # }
//! ```

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub use esh_core::*;

#[cfg(feature = "env")]
pub use esh_providers_env::{DotenvProvider, EnvProvider};

#[cfg(feature = "aws")]
pub use esh_providers_aws::{
    AwsSecretsClient, AwsSecretsManagerProvider, AwsSsmProvider, MapAwsClient,
};

#[cfg(feature = "azure")]
pub use esh_providers_azure::{AzureKeyVaultClient, AzureKeyVaultProvider, MapAzureClient};

#[cfg(feature = "gcp")]
pub use esh_providers_gcp::{GcpSecretClient, GcpSecretManagerProvider, MapGcpClient};

#[cfg(feature = "vault")]
pub use esh_providers_vault::{MapVaultClient, VaultClient, VaultKvProvider};

#[cfg(feature = "k8s")]
pub use esh_providers_k8s::KubernetesSecretsProvider;

#[cfg(feature = "docker")]
pub use esh_providers_docker::DockerSecretsProvider;

#[cfg(feature = "memory-cache")]
pub use esh_cache::MemoryCache;

#[cfg(feature = "file-cache")]
pub use esh_cache::FileCache;

#[cfg(feature = "redis-cache")]
pub use esh_cache::RedisCache;

#[cfg(feature = "audit")]
pub use esh_audit::{MemoryAuditor, StdoutAuditor, TracingAuditor};

#[cfg(feature = "crypto")]
pub use esh_crypto::{
    Decryptor, Encryptor, KmsClient, KmsDecryptor, PassthroughKms, SopsIntegration,
};

#[cfg(feature = "age")]
pub use esh_crypto::{age_encrypt, decrypt_age_string, AgeIdentityDecryptor};
