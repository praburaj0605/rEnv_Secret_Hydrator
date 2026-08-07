# Env-Secret-Hydrator — Production Development Plan

| Field | Value |
| --- | --- |
| Document | Development Plan |
| Product | Env-Secret-Hydrator (Project 2) |
| Source | Rust Cloud-Native Libraries SRS-II v1.0 |
| Status | Implemented (v0.1.0 workspace) |
| Target | Production-grade Rust library (crates.io + enterprise adoption) |
| Horizon | ~16 weeks to GA (1.0.0) |

---

## 1. Executive summary

Env-Secret-Hydrator is a **unified, provider-agnostic configuration and secret hydration library** for Rust. Applications declare typed configuration once; the library resolves values from environment variables, `.env`, cloud secret stores, Kubernetes/Docker secrets, and local defaults — without leaking origin details into application code.

**Success looks like:** a feature-gated crate that (1) loads typed config safely, (2) caches and rotates secrets, (3) never prints secrets, (4) works offline for local development, and (5) meets `<200ms` cached load latency.

---

## 2. Goals, non-goals, and constraints

### 2.1 Goals (from SRS objectives)

| Goal | Plan implication |
| --- | --- |
| Remove hardcoded env assumptions | Provider abstraction + resolution chain |
| Cloud-native secret management | Pluggable backends behind Cargo features |
| Local development without cloud | Ordered fallback: cloud → `.env` → defaults |
| Automatic secret rotation | Background refresh + atomic publish |
| Strongly typed configuration | Serde-first `load::<T>()` API |

### 2.2 Non-goals (GA / 1.0)

Defer to post-GA (SRS §6):

- Secret version rollback UI/API beyond basic version pin
- HSM / FIPS compliance
- Kubernetes Operator
- GitOps controllers
- Full multi-cloud active-active failover
- Policy engine (OPA-style)

### 2.3 Hard constraints (NFRs)

| Area | Requirement | Design rule |
| --- | --- | --- |
| Performance | Cached load `<200ms` | Hot path stays in-memory; no blocking cloud I/O on cache hit |
| Concurrency | Async operations | `async` provider trait; Tokio-compatible |
| Memory | Low usage | Feature-gated SDKs; zeroize secret buffers |
| Security | Zero plaintext persistence | No default disk write of secrets; file cache encrypted or opt-in |
| Logging | Secure | Custom `SecretString`; redact in `Display`/`Debug`/`Error` |
| Availability | Offline fallback + redundancy | Local chain always available; optional secondary provider |

---

## 3. Delivery strategy

### 3.1 Principles

1. **Traits first, providers second** — stabilize `Provider`, `Cache`, `Resolver`, `Auditor` before cloud SDKs.
2. **Cargo features = blast radius** — consumers pay only for providers they enable.
3. **Security is a release gate** — secret-leak tests block merge from day one.
4. **Contract tests** — every provider implements the same conformance suite.
5. **Semantic versioning** — `0.x` while APIs settle; `1.0` only after AC + security review.

### 3.2 Recommended crate layout

```text
env-secret-hydrator/                 # workspace root
├── Cargo.toml                       # workspace + shared lints/MSRV
├── crates/
│   ├── esh-core/                    # traits, resolver, typed load, mask, validate
│   ├── esh-providers-env/           # env + dotenv (always-on or default feature)
│   ├── esh-providers-aws/           # Secrets Manager + SSM
│   ├── esh-providers-azure/         # Key Vault
│   ├── esh-providers-gcp/           # Secret Manager
│   ├── esh-providers-vault/         # HashiCorp Vault
│   ├── esh-providers-k8s/           # Kubernetes secrets
│   ├── esh-providers-docker/        # Docker secrets
│   ├── esh-cache/                   # memory + file (+ redis feature)
│   ├── esh-crypto/                  # KMS / SOPS / Age adapters
│   ├── esh-audit/                   # structured audit events
│   └── esh/                         # façade crate re-exporting features
├── examples/
├── benches/
├── tests/                           # workspace integration + conformance
└── docs/                            # book / ADRs
```

**Façade usage (illustrative):**

```toml
env-secret-hydrator = { version = "1", features = ["aws", "yaml", "redis-cache"] }
```

### 3.3 Suggested public API shape (stabilize in P0–P1)

```rust
#[derive(Deserialize)]
struct AppConfig {
    database_url: SecretString,
    #[serde(default)]
    region: String,
}

let config: Arc<AppConfig> = Hydrator::builder()
    .detect_runtime()                 // FR-002
    .with_fallback(Fallback::Local)   // FR-005
    .cache(MemoryCache::ttl(secs(300)))
    .on_audit(StdoutAuditor::json())
    .build()
    .await?
    .load()
    .await?;

let live = hydrator.watch::<AppConfig>().await?; // FR-007 → watch::Receiver
```

Exact names are negotiable; **behavior and invariants are not**.

---

## 4. Phased roadmap

Estimates assume **one senior Rust engineer** (scale linearly with team size). Calendar can compress with parallel provider work after P0 traits land.

| Phase | Weeks | Theme | Exit gate |
| --- | --- | --- | --- |
| **P0** | 1–3 | Foundation | Local load + typed + mask + validate |
| **P1** | 4–6 | Cloud core | AWS + cache + audit + selection |
| **P2** | 7–10 | Full providers + live secrets | All FR-001 providers + rotation + reload |
| **P3** | 11–13 | Security & NFR hardening | Encryption + perf + redundancy |
| **P4** | 14–16 | GA | Docs, release, security sign-off |

### Phase P0 — Foundation (Weeks 1–3)

**Objectives:** Make the library useful for local Rust apps with zero cloud deps.

| Work item | SRS | Notes |
| --- | --- | --- |
| Workspace, MSRV, CI (fmt, clippy `-D`, test, `cargo deny`) | — | Treat as Definition of Ready for all later work |
| Error type that never embeds secret values | FR-009 | `thiserror` + redacted sources |
| `SecretString` / `SecretBytes` with `Zeroize` | FR-009 | No `Debug` plaintext; `serde` deserialize OK |
| `Provider` + `ConfigSource` traits | FR-001 | Sync metadata + async `fetch` |
| Env provider | FR-001 | Process environment |
| Dotenv provider | FR-001, FR-005 | Optional path; never commit secrets |
| Resolution chain: cloud→env→dotenv→default | FR-005 | Cloud stub returns `Unavailable` in P0 |
| Typed `load::<T: DeserializeOwned>()` | FR-006 | JSON/YAML/TOML behind features |
| Validator: required / format / missing | FR-008 | Schema hooks + field-level errors |
| Unit + property tests for masking | FR-009 | Assert `format!("{:?}", s)` has no raw value |
| Example: `examples/local_app` | AC | README quickstart |

**P0 Definition of Done**

- [ ] `cargo test --workspace` green on Linux/macOS/Windows CI
- [ ] Local fallback path documented and demoed
- [ ] Leak tests fail the build if a secret appears in `Debug`/`Display`

### Phase P1 — Cloud core (Weeks 4–6)

**Objectives:** Production path for the most common cloud (AWS) + operational visibility.

| Work item | SRS | Notes |
| --- | --- | --- |
| Runtime / config-based provider selection | FR-002 | Explicit config wins; then env hints; then detect |
| AWS Secrets Manager provider | FR-001 | JSON secret → flatten into config map |
| AWS SSM Parameter Store provider | FR-001 | Path prefix support |
| Memory cache + TTL | FR-003 | Default cache |
| File cache (opt-in, encrypted-at-rest preferred) | FR-003 | Disabled by default (NFR: zero plaintext persistence) |
| Audit events: loaded / refreshed / expired / failure | FR-010 | Structured JSON; no secret values |
| Integration tests via LocalStack or AWS stubs | AC | Wiremock / testcontainers |
| IAM least-privilege docs | Security | Example policies |

**P1 Definition of Done**

- [ ] AWS path works with real credentials in staging
- [ ] Cached hit path measured (baseline for `<200ms`)
- [ ] Audit log fixtures reviewed for leakage

### Phase P2 — Remaining providers + live behavior (Weeks 7–10)

**Objectives:** Complete FR-001 surface; enable rotation and hot reload.

| Work item | SRS | Notes |
| --- | --- | --- |
| Azure Key Vault | FR-001 | Feature `azure` |
| Google Secret Manager | FR-001 | Feature `gcp` |
| HashiCorp Vault (KV v2) | FR-001 | Token / AppRole / K8s auth |
| Kubernetes secrets | FR-001 | In-cluster + kubeconfig |
| Docker secrets | FR-001 | `/run/secrets` |
| Redis cache | FR-003 | Feature `redis-cache`; TLS |
| Refresh interval + retry + failure policy | FR-004 | Exponential backoff; jitter; circuit break |
| Runtime watcher / reload | FR-007 | `tokio::sync::watch` or `arc-swap` |
| Conformance test suite for all providers | AC | Same assertions per backend |
| Chaos tests: provider timeout / 5xx | FR-004 | Fallback behavior documented |

**Rotation failure policy (recommended defaults)**

| Mode | Behavior |
| --- | --- |
| `KeepLastGood` (default) | Serve last valid config; emit audit failure |
| `FailFast` | Surface error to health probe / `Result` |
| `FallbackLocal` | Drop to `.env` / defaults if configured |

**P2 Definition of Done**

- [ ] All SRS providers implemented behind features
- [ ] Rotation refreshes without process restart
- [ ] Concurrent readers never observe partially updated config

### Phase P3 — Encryption, hardening, NFRs (Weeks 11–13)

**Objectives:** Meet security/performance NFRs; encryption layer.

| Work item | SRS | Notes |
| --- | --- | --- |
| AWS KMS decrypt path | FR-011 | Ciphertext envelope support |
| Azure KV / GCP KMS decrypt | FR-011 | Feature-gated |
| SOPS + Age adapters | FR-011 | Local/GitOps-friendly |
| Multi-provider ordered redundancy | NFR | Primary → secondary on hard failure |
| Memory cleanup / drop guarantees | NFR | `zeroize` on drop; document limits |
| Criterion benches: cold vs cached load | NFR | Gate: cached p95 `<200ms` on CI runner profile |
| Security review + dependency audit | AC | `cargo deny`, advisory DB, manual review |
| Fuzz redaction / config parsers | FR-009 | cargo-fuzz on parsers only (not live cloud) |

**P3 Definition of Done**

- [ ] Encryption features documented with threat model note
- [ ] Perf gate green; regression bench in CI (nightly OK)
- [ ] Written threat model (`docs/THREAT_MODEL.md`)

### Phase P4 — GA release (Weeks 14–16)

**Objectives:** Ship 1.0.0 as a trustworthy industry artifact.

| Work item | Deliverable |
| --- | --- |
| API freeze + semver promise | CHANGELOG + migration notes |
| mdBook / docs.rs complete | Provider matrix, security, examples |
| Reference architectures | ECS/EKS/AKS/GKE + local |
| Supply chain | Signed tags, provenance (optional), LICENSE, CODE_OF_CONDUCT |
| Support policy | MSRV, LTS branch rules |
| Acceptance criteria dry-run | Checklist in §8 signed off |
| crates.io publish + GitHub Release | `env-secret-hydrator` 1.0.0 |

---

## 5. Architecture & design standards

### 5.1 Module map (SRS §5)

| Module | Responsibility | Crate |
| --- | --- | --- |
| Config Loader | Compose sources → typed `T` | `esh-core` |
| Secret Providers | Fetch raw secrets | `esh-providers-*` |
| Cache Manager | TTL, invalidation, backends | `esh-cache` |
| Validator | Required/format/expiry | `esh-core` |
| Runtime Watcher | Poll / push refresh | `esh-core` |
| Encryption Layer | Decrypt envelopes | `esh-crypto` |
| Audit Logger | Lifecycle events | `esh-audit` |

### 5.2 Resolution algorithm (normative)

```text
1. Build ordered provider list (config > env > runtime detect)
2. For each config key / secret reference:
   a. Cache lookup (if enabled and fresh) → use
   b. Else try providers in order until success
   c. On success → populate cache + audit Loaded/Refreshed
   d. On soft failure → next provider
   e. On exhaustion → local .env → defaults → validation error
3. Deserialize aggregated map into T
4. Validate T
5. Publish Arc<T> atomically
```

### 5.3 Key design decisions (ADRs to write in P0)

1. **ADR-001:** Async-only provider I/O (with thin sync wrapper if needed).
2. **ADR-002:** Secrets never implement plaintext `Display`/`Debug`.
3. **ADR-003:** File cache off by default; when on, encrypt or warn loudly.
4. **ADR-004:** Feature flags per cloud vendor to control binary size.
5. **ADR-005:** Last-good config retained on refresh failure by default.

### 5.4 Observability

- **Metrics (optional feature):** cache hit ratio, refresh latency, provider errors.
- **Tracing spans:** `esh.resolve`, `esh.provider.fetch`, `esh.cache` — field values redacted.
- **Audit:** durable event stream for security teams (FR-010).

---

## 6. Engineering standards

### 6.1 Language & toolchain

| Item | Standard |
| --- | --- |
| Language | Rust 2021 edition |
| MSRV | Pin and CI-test (e.g. N-2 stable) |
| Async | Tokio (optional runtime feature if feasible) |
| Serialization | `serde` + `serde_json`; `yaml`/`toml` features |
| Lints | `clippy::pedantic` selectively; deny warnings in CI |
| Supply chain | `cargo deny` (licenses, advisories, bans) |

### 6.2 Testing strategy

| Layer | Scope |
| --- | --- |
| Unit | Masking, validation, resolution order, TTL expiry |
| Conformance | Every provider: happy path, not-found, timeout, auth failure |
| Integration | LocalStack / Azurite / Fake GCP / Vault dev / kind cluster |
| Security | Leak assertions on logs, errors, panics |
| Performance | Criterion cold/cached; CI budget for cached path |
| Compatibility | Matrix: OS × features × MSRV |

### 6.3 CI/CD pipeline (minimum)

```text
PR → fmt → clippy → test (default features)
    → test (--all-features where credentials mocked)
    → cargo deny → doc build → bench smoke (optional)
Tag v* → create GitHub Release → crates.io publish (manual approval)
```

### 6.4 Code review checklist (security-critical)

- [ ] No `println!` / `dbg!` of config structs containing secrets
- [ ] Errors use type names / key names, never values
- [ ] New provider has conformance tests
- [ ] Feature flag documented in README feature matrix
- [ ] Changelog entry under SemVer category

---

## 7. Security & compliance plan

### 7.1 Threat model (summary)

| Threat | Control |
| --- | --- |
| Secrets in logs/panics | `SecretString`, custom hooks, CI leak tests |
| Secrets on disk | No plaintext persistence by default |
| Dependency compromise | `cargo deny`, pin versions, minimal features |
| Confused deputy / wrong account | Explicit provider config; document IAM |
| Stale secrets after rotation | Refresh + TTL + audit Expired |
| Memory scraping | `zeroize`; document residual OS risk |

### 7.2 Secure development lifecycle

1. Threat model in repo (P3).
2. Dependency review on every PR.
3. Pre-1.0 external or peer security review.
4. Coordinated vulnerability disclosure policy (`SECURITY.md`).
5. Rapid 1.0.x patch process for leak-class bugs (treat as P0).

---

## 8. Acceptance criteria traceability

| Acceptance criterion (SRS §7) | Verification |
| --- | --- |
| Works with all supported providers | Conformance suite green for each feature |
| Automatic rotation operational | Integration test: rotate → reload within interval |
| Strong typing verified | Compile-time `load::<T>()` + negative tests |
| Local fallback functional | Offline CI job without network |
| Zero secret leakage in logs | Dedicated leak test job + audit fixture review |

NFR checks:

| NFR | Verification |
| --- | --- |
| Cached load `<200ms` | Bench gate |
| Async operations | API is `async`; no blocking on hot path |
| Offline fallback | Network-disabled test |
| Multi-provider redundancy | Failover integration test |

---

## 9. Work breakdown (epics → first sprint backlog)

### Epic E1 — Platform

- Scaffold workspace, CI, `SECURITY.md`, `CONTRIBUTING.md`, license
- Shared lint/deny config
- ADR template

### Epic E2 — Core hydration

- Traits + resolver + typed load
- Env + dotenv
- Validation + masking
- Local example

### Epic E3 — AWS path

- SM + SSM providers
- Memory/file cache
- Audit logger
- LocalStack tests

### Epic E4 — Provider expansion

- Azure, GCP, Vault, K8s, Docker
- Redis cache
- Conformance harness

### Epic E5 — Live secrets

- Rotation scheduler
- Retry/backoff policies
- Runtime watch API

### Epic E6 — Crypto & GA

- KMS/SOPS/Age
- Perf hardening
- Docs + 1.0 release

### Sprint 1 backlog (Week 1)

1. Initialize Cargo workspace + CI.
2. Implement `SecretString` + leak tests.
3. Define `Provider` / `Resolver` traits + error model.
4. Env provider + dotenv provider.
5. Skeleton `Hydrator::builder().load::<T>()`.
6. Draft ADR-001..005.

---

## 10. Risk register

| ID | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| R1 | Accidental secret leakage | Med | Critical | Types + CI from day one |
| R2 | Cloud SDK binary bloat | High | Med | Feature flags; prefer lean HTTP where viable |
| R3 | Refresh races / torn reads | Med | High | Atomic `Arc` publish |
| R4 | Flaky cloud integration tests | High | Med | Testcontainers + recorded fixtures |
| R5 | Scope creep (SRS future features) | Med | Med | Strict GA cutoff; backlog Phase F |
| R6 | Auth model differences across clouds | High | Med | Provider-specific auth modules + docs |
| R7 | File cache contradicts “zero plaintext” | Med | High | Opt-in + encryption + warnings |

---

## 11. Release & versioning policy

| Version | Meaning |
| --- | --- |
| `0.1.x` | P0 complete — local-only usable |
| `0.2.x` | P1 — AWS production preview |
| `0.3.x` | P2 — multi-provider preview |
| `0.9.x` | P3 — API freeze candidate |
| `1.0.0` | GA — AC met, security review done |

Breaking changes allowed only in `0.x`. After `1.0`, follow SemVer strictly; deprecations require at least one minor release.

---

## 12. Documentation deliverables

| Doc | Phase |
| --- | --- |
| README quickstart | P0 |
| Feature matrix | P1+ |
| Provider setup guides (IAM/RBAC) | Per provider |
| Security & threat model | P3 |
| ADR set | Ongoing |
| Migration guide to 1.0 | P4 |
| Runbooks: rotation failure, cache poisoning suspicion | P4 |

---

## 13. Team roles (recommended)

| Role | Focus |
| --- | --- |
| Tech lead / library designer | Traits, API stability, ADRs |
| Cloud engineers (parallel after P0) | Provider implementations |
| Security reviewer | Leakage, threat model, release gate |
| DevEx / docs | Examples, mdBook, DX polish |

Minimum viable staffing: **1 senior Rust engineer** through P4; add 1 engineer at P2 for provider parallelism.

---

## 14. Immediate next actions

1. Approve this plan and GA scope cutoff (defer SRS §6).
2. Confirm MSRV, license (MIT/Apache-2.0 dual recommended), and crate name (`env-secret-hydrator`; `esh` is taken on crates.io).
3. Start **Sprint 1 / P0**: workspace scaffold + core traits + env/dotenv + masking.
4. Create GitHub Project board mirroring epics E1–E6.
5. Schedule mid-P3 security review slot.

---

## 15. Appendix — SRS requirement index

| ID | Title | Target phase |
| --- | --- | --- |
| FR-001 | Configuration Loader (all sources) | P0–P2 |
| FR-002 | Provider Selection | P1 |
| FR-003 | Secret Caching | P1–P2 |
| FR-004 | Secret Rotation | P2 |
| FR-005 | Local Development Mode | P0 |
| FR-006 | Typed Configuration | P0 |
| FR-007 | Runtime Reload | P2 |
| FR-008 | Validation | P0–P1 |
| FR-009 | Secret Masking | P0 |
| FR-010 | Audit Logging | P1 |
| FR-011 | Encryption | P3 |

---

*This plan is the execution baseline for Env-Secret-Hydrator. Update it when scope or staffing changes; keep SRS acceptance criteria as the release authority for 1.0.*
