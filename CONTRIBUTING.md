# Contributing

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --features full -- -D warnings
cargo test --workspace --features full
```

## Guidelines

1. **No secret leakage** — add/adjust leak tests when touching `Display`/`Debug`/`Error`/audit paths.
2. **Feature gates** — new cloud SDKs or heavy deps must be optional Cargo features.
3. **Conformance** — new providers implement `Provider` and get a unit/conformance test.
4. **ADRs** — record material design decisions under `docs/adr/`.
5. **SemVer** — breaking changes only in `0.x` until `1.0`; document migrations.

## PR checklist

- [ ] Tests added/updated
- [ ] `CHANGELOG.md` note (when present)
- [ ] Feature matrix / README updated if public API changed
- [ ] No plaintext secrets in fixtures
