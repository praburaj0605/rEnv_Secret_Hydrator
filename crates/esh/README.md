# esh — Env-Secret-Hydrator

Unified, provider-agnostic configuration and secret hydration for Rust.

Applications declare typed configuration once; `esh` resolves values from environment variables, `.env`, cloud secret stores, Kubernetes/Docker secrets, and local defaults — without leaking origin details into application code.

## Quick start

```toml
[dependencies]
esh = { version = "0.1", features = ["env", "memory-cache", "audit"] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use esh::{DotenvProvider, EnvProvider, Fallback, Hydrator, MemoryCache, SecretString};
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct AppConfig {
    database_url: SecretString,
    #[serde(default)]
    region: String,
}

#[tokio::main]
async fn main() -> esh::Result<()> {
    let config = Hydrator::builder()
        .provider(EnvProvider::new())
        .provider(DotenvProvider::from_default()?)
        .with_fallback(Fallback::Local)
        .cache(MemoryCache::ttl(Duration::from_secs(300)))
        .keys(vec!["database_url".into(), "region".into()])
        .build()
        .await?
        .load::<AppConfig>()
        .await?;

    println!("region={}", config.region);
    println!("db={}", config.database_url); // prints ***REDACTED***
    Ok(())
}
```

## Feature matrix

| Feature | Enables |
| --- | --- |
| `env` (default) | Env + dotenv providers |
| `aws` | AWS Secrets Manager + SSM |
| `azure` | Azure Key Vault |
| `gcp` | Google Secret Manager |
| `vault` | HashiCorp Vault KV v2 |
| `k8s` | Kubernetes mounted secrets |
| `docker` | Docker `/run/secrets` |
| `memory-cache` (default) | In-memory TTL cache |
| `file-cache` | Opt-in file cache (warns: may persist secrets) |
| `redis-cache` | Redis TTL cache |
| `audit` | Tracing / stdout / memory auditors |
| `crypto` / `age` | KMS traits + Age decrypt |
| `yaml` / `toml` | Extra serde formats in core |
| `full` | All of the above except Redis |

## Security invariants

- `SecretString` / `SecretBytes` never print plaintext in `Display` / `Debug`
- Errors carry key names and provider IDs — never values
- File cache is **off** unless you enable `file-cache`
- Audit events are metadata-only

## License

MIT OR Apache-2.0
