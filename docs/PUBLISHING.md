# Publishing to crates.io

Checklist and ordered publish steps for **Env-Secret-Hydrator** `0.1.0`.

## Why `env-secret-hydrator`?

The short name `esh` is already taken on crates.io. The façade package is published as **`env-secret-hydrator`** (Rust import: `env_secret_hydrator`). Leaf crates keep the `esh-*` prefix.

## Pre-flight

1. Confirm you are logged in: `cargo login` (API token from https://crates.io/settings/tokens).
2. Ensure `git status` is clean on the release commit.
3. Run locally:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --features full -- -D warnings
   cargo test --workspace --features full
   cargo deny check
   ```
4. Dry-run packaging for crates whose crates.io dependencies already exist (start with `esh-core`). Dependents can only fully dry-run after their workspace deps are published:
   ```bash
   cargo publish -p esh-core --dry-run
   # after esh-core is live on crates.io:
   cargo publish -p esh-providers-env --dry-run
   ```
   To inspect the tarball without resolving unpublished workspace deps:
   ```bash
   cargo package -p esh-providers-env --no-verify --list
   ```

## Publish order

Publish dependencies before dependents. Wait for crates.io to index each crate (~1 minute) before the next, or use `--no-verify` only if you already verified locally (prefer full verify).

1. `esh-core`
2. `esh-providers-env`
3. `esh-providers-aws`
4. `esh-providers-azure`
5. `esh-providers-gcp`
6. `esh-providers-vault`
7. `esh-providers-k8s`
8. `esh-providers-docker`
9. `esh-cache`
10. `esh-audit`
11. `esh-crypto`
12. `env-secret-hydrator` (directory: `crates/esh`)

Not published (`publish = false`):

- `esh-test-support`
- `local_app`

## Commands

```bash
cargo publish -p esh-core
cargo publish -p esh-providers-env
cargo publish -p esh-providers-aws
cargo publish -p esh-providers-azure
cargo publish -p esh-providers-gcp
cargo publish -p esh-providers-vault
cargo publish -p esh-providers-k8s
cargo publish -p esh-providers-docker
cargo publish -p esh-cache
cargo publish -p esh-audit
cargo publish -p esh-crypto
cargo publish -p env-secret-hydrator
```

## After publish

1. Tag the release: `git tag v0.1.0 && git push origin v0.1.0`
2. Create a GitHub Release from the tag with notes from `CHANGELOG.md`
3. Verify docs.rs builds for `env-secret-hydrator` (all-features enabled via package metadata)

## Version bumps

Bump `[workspace.package] version` and matching `version = "..."` entries under `[workspace.dependencies]` together. Keep path+version for publishable workspace members.
