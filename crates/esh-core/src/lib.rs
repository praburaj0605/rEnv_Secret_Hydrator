//! Core types and traits for Env-Secret-Hydrator.
//!
//! Applications should normally depend on the [`esh`](https://docs.rs/esh) façade crate.

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

mod audit;
mod cache;
mod error;
mod hydrator;
mod provider;
mod resolver;
mod rotate;
mod secret;
mod select;
mod validate;
mod value;

pub use audit::{AuditEvent, Auditor, EventKind, NoopAuditor};
pub use cache::{Cache, CacheKey, CacheEntry, NoopCache};
pub use error::{Error, Result};
pub use hydrator::{Fallback, Hydrator, HydratorBuilder, WatchedConfig};
pub use provider::{Provider, ProviderCapability, ProviderId, ProviderMeta};
pub use resolver::{ResolveOptions, Resolver};
pub use rotate::{FailurePolicy, RefreshConfig, RetryPolicy};
pub use secret::{SecretBytes, SecretString};
pub use select::{ProviderSelector, RuntimeHints, SelectionSource};
pub use validate::{FieldRule, ValidationError, Validator};
pub use value::{ConfigMap, ConfigValue, SecretRef};
