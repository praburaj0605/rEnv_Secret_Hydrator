# ADR-004: Feature flags per cloud vendor

- Status: Accepted
- Date: 2026-08-07

## Context

Cloud SDKs and HTTP stacks inflate compile times and binary size.

## Decision

Each provider family is an optional Cargo feature on the `env-secret-hydrator` façade. Injectable client traits keep unit tests free of live cloud credentials.

## Consequences

- Consumers enable only what they need.
- Production AWS/Azure/GCP SDK wrappers can be added behind the same traits without breaking the hydrator.
