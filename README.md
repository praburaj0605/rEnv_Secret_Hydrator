# Env-Secret-Hydrator

Production-grade Rust library for unified configuration and secret hydration across environment variables, `.env`, AWS, Azure, GCP, Vault, Kubernetes, and Docker.

## Status

Implementation tracks [`DEVELOPMENT_PLAN.md`](./DEVELOPMENT_PLAN.md) (SRS-II). Current crate version: **0.1.0**.

Testing: see [`docs/TEST_PLAN.md`](./docs/TEST_PLAN.md) for the production-grade suite (conformance, security leak gate, perf budget, live backends).

## Workspace layout

```text
crates/
  esh-core/              traits, hydrator, masking, validation, watch
  esh-providers-*        env, aws, azure, gcp, vault, k8s, docker
  esh-cache/             memory, file, redis
  esh-audit/             tracing / stdout / memory auditors
  esh-crypto/            Age + KMS decrypt traits
  esh/                   façade crate
examples/local_app/
```

## Quick start

```toml
[dependencies]
esh = { path = "crates/esh", features = ["env", "memory-cache", "audit"] }
```

See [`crates/esh/README.md`](./crates/esh/README.md) for the feature matrix and API examples.

```bash
cargo test --workspace --features full
cargo run -p local_app
```

## Security

- Secrets never appear in `Debug` / `Display` / audit payloads
- Report vulnerabilities per [`SECURITY.md`](./SECURITY.md)
- Threat model: [`docs/THREAT_MODEL.md`](./docs/THREAT_MODEL.md)

## License

MIT OR Apache-2.0
