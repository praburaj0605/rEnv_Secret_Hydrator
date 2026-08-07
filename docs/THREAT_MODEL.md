# Threat Model — Env-Secret-Hydrator

## Assets

- Application secrets in memory (`SecretString` / `SecretBytes`)
- Cached secrets (memory / optional file / Redis)
- Cloud credentials used by provider clients (owned by the host app)

## Trust boundaries

| Boundary | Trust |
| --- | --- |
| Application process | Trusted to call `expose()` |
| Logs / metrics / audit sinks | Untrusted for secret values |
| File cache directory | Semi-trusted; must be encrypted at rest if enabled |
| Cloud provider APIs | Trusted after TLS + IAM |

## Threats and controls

| ID | Threat | Control |
| --- | --- | --- |
| T1 | Secrets printed in logs/panics | Redacted `Display`/`Debug`; CI leak tests |
| T2 | Secrets in error strings | `Error` variants carry keys/providers only |
| T3 | Plaintext disk persistence | File cache off by default; warning on enable |
| T4 | Stale secrets after rotation | Refresh + TTL + audit `SecretExpired`/`Refreshed` |
| T5 | Provider outage | Ordered fallback; `FailurePolicy` |
| T6 | Dependency compromise | `cargo deny`, feature-minimal deps |
| T7 | Redis eavesdropping | Prefer TLS Redis URLs; network policy |
| T8 | Memory scraping | `zeroize` on drop; residual OS risk documented |

## Out of scope (Phase F)

- HSM / FIPS modules
- Multi-cloud active-active failover orchestration
- Kubernetes operator control plane
