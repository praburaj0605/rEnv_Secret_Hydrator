# Security Policy

## Supported versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |
| < 0.1 | No |

## Reporting a vulnerability

**Do not open a public GitHub issue for secret-leakage or authentication bypasses.**

Email security reports to the maintainers (replace with your project security contact) with:

1. Description and impact
2. Minimal reproduction (no production secrets)
3. Affected crate versions / features

We aim to acknowledge within 3 business days and ship patches for confirmed leak-class bugs as `0.x` / `1.x` patch releases.

## Safe usage expectations

- Never log `SecretString::expose()`
- Keep `file-cache` disabled unless the directory is encrypted at rest
- Prefer IAM/RBAC least privilege for cloud providers
- Treat dependency advisories from `cargo deny` as release blockers
