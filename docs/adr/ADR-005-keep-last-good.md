# ADR-005: Keep last-good config on refresh failure

- Status: Accepted
- Date: 2026-08-07

## Context

Rotation refresh can fail transiently. Dropping config would take applications offline.

## Decision

Default `FailurePolicy::KeepLastGood` retains the last successful `Arc<T>` while emitting audit/trace failures. `FailFast` and `FallbackLocal` are available.

## Consequences

- Availability preferred over immediate consistency by default.
- Operators can opt into stricter policies.
