# ADR-003: File cache opt-in only

- Status: Accepted
- Date: 2026-08-07

## Context

NFR requires zero plaintext persistence by default.

## Decision

Default cache is memory (or none). `FileCache` is feature-gated and emits a warning on construction.

## Consequences

- Operators must consciously accept disk risk.
- Encrypted-at-rest volumes remain an ops concern.
