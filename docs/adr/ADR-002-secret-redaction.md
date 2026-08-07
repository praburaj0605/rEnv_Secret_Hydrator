# ADR-002: Secrets never implement plaintext Display/Debug

- Status: Accepted
- Date: 2026-08-07

## Context

FR-009 requires secrets never appear in logs, panic messages, or debug output.

## Decision

`SecretString` / `SecretBytes` always render as `***REDACTED***`. Plaintext is only available via `expose()`.

## Consequences

- Accidental `println!("{:?}", config)` is safe.
- Call sites that need plaintext must be explicit and reviewable.
