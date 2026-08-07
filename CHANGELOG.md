# Changelog

## 0.1.0 — 2026-08-07

### Added

- Cargo workspace with `esh` façade and modular provider crates
- Core hydrator: typed load, validation, local fallback, retry, watch/refresh
- Providers: env, dotenv, AWS (injectable), Azure, GCP, Vault, Kubernetes, Docker
- Caches: memory, opt-in file, Redis (feature)
- Audit sinks and Age/KMS crypto traits
- CI, deny.toml, threat model, ADRs, local example
- Production test suite: `esh-test-support`, provider/cache conformance, security leak gate, proptest, fuzz target, cached-load p95 budget, OS/MSRV CI matrix, live Redis/Vault jobs (`docs/TEST_PLAN.md`)
