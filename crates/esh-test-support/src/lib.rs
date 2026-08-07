//! Shared test helpers for Env-Secret-Hydrator.
//!
//! This crate is for tests only (`publish = false`).

#![allow(missing_docs)]

pub mod conformance;
pub mod counting;
pub mod fixtures;
pub mod leak;
pub mod temp;

pub use conformance::cache::{run_cache_conformance, CacheConformanceCfg};
pub use conformance::provider::{run_provider_conformance, ProviderConformanceCfg};
pub use counting::{CountingCache, CountingProvider, FaultMode, FaultyProvider, MapProvider};
pub use fixtures::{API_TOKEN, DATABASE_URL, SAMPLE_SECRET, SAMPLE_TOKEN};
pub use leak::{assert_no_leak, assert_no_leak_in, LeakNeedle};
pub use temp::{write_secret_dir, write_temp_dotenv, TempSecrets};
