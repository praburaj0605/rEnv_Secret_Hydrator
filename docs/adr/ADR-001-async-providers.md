# ADR-001: Async-only provider I/O

- Status: Accepted
- Date: 2026-08-07

## Context

Secret backends are network-bound. Mixing sync and async APIs complicates the hydrator and encourages blocking on async runtimes.

## Decision

The `Provider` trait is fully async (`async-trait`). Applications run on Tokio (or compatible).

## Consequences

- Sync wrappers are left to callers if needed.
- File/env providers use async wrappers around fast local I/O.
