# Env-Secret-Hydrator — Test Plan

## Layers

| Layer | Location | Command |
| --- | --- | --- |
| Unit | `crates/*/src/**` + `crates/esh-core/tests/*` | `cargo test -p esh-core --features full` |
| Provider conformance | Each `esh-providers-*` crate | `cargo test -p esh-providers-aws` (etc.) |
| Cache conformance | `esh-cache` + `esh-test-support` | `cargo test -p esh-cache` |
| Integration | `crates/esh/tests/*.rs` | `cargo test -p env-secret-hydrator --features full` |
| Security leak gate | `crates/esh/tests/security_leak.rs` | `cargo test -p env-secret-hydrator --test security_leak --features full` |
| Perf budget | `cached_load_budget` | `cargo test -p env-secret-hydrator --test cached_load_budget --features full` |
| Criterion | `crates/esh/benches` | `cargo bench -p env-secret-hydrator --features full` |
| Property | `esh-core/tests/secret_props.rs` | included in `cargo test -p esh-core` |
| Fuzz | `fuzz/fuzz_targets/fuzz_validate_json.rs` | `cargo fuzz run fuzz_validate_json` (nightly) |
| Live | Redis / Vault `#[ignore = "live"]` | `cargo test ... -- --ignored` |

## Shared harness

[`crates/esh-test-support`](../crates/esh-test-support) provides:

- `run_provider_conformance` / `run_cache_conformance`
- `CountingProvider`, `FaultyProvider`, `MapProvider`
- Leak helpers (`assert_no_leak`)
- Temp dotenv / secret dir fixtures

## FR / NFR traceability

| ID | Suite |
| --- | --- |
| FR-001 | Provider conformance in every provider crate; live Vault/Redis |
| FR-002 | `esh-core/tests/select.rs`, `esh/tests/selection.rs` |
| FR-003 | Memory/File cache conformance; Redis live |
| FR-004 | `esh-core/tests/hydrator_rotate.rs`, `esh/tests/rotation.rs` |
| FR-005 | `esh/tests/fallback.rs` |
| FR-006 | `esh/tests/typed_load.rs` |
| FR-007 | `watch_*` in rotation tests |
| FR-008 | Expanded `validate.rs` unit tests |
| FR-009 | `security_leak`, proptest, fuzz |
| FR-010 | `esh-audit` event-kind tests |
| FR-011 | `esh-crypto` KMS/Age tests |
| `<200ms` | `cached_load_budget` (Linux assert) |

## Definition of Done

- [ ] `cargo test --workspace --features full` green on Linux/macOS/Windows
- [ ] `cargo test -p env-secret-hydrator --test security_leak --features full` green
- [ ] Linux `cached_load_budget` p95 `<200ms`
- [ ] Provider conformance present for env, dotenv, AWS, Azure, GCP, Vault, K8s, Docker
- [ ] Nightly/main live Redis + Vault ignored tests execute
- [ ] Nightly fuzz smoke (`max_total_time=60`)

## Local quick start

```bash
cargo test --workspace --features full
cargo test -p env-secret-hydrator --test security_leak --features full
cargo test -p env-secret-hydrator --test cached_load_budget --features full -- --nocapture
```
