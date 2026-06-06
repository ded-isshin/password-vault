# Feature Map

Status: draft.

This document maps product capabilities to MVP, post-MVP, and explicit non-goals.

## Core MVP

| Area | Feature | Notes |
| --- | --- | --- |
| Identity | Register personal account | Organization accounts are deferred. |
| Identity | Login | Final auth/key-derivation protocol is not selected. |
| Sessions | Server-side browser session | Opaque session ID in secure cookies is preferred for MVP. |
| MFA | TOTP enrollment | QR and manual secret display, verify before activation. |
| MFA | TOTP verification | Rate limiting and replay prevention are required. |
| MFA | Account recovery codes | Recover login factor only, not vault decryption. |
| Vault | One personal vault | Future organization vaults must fit the same key model. |
| Items | Login item | Type stored inside encrypted payload. |
| Items | Secure note | Type stored inside encrypted payload. |
| Items | Item revisions | Immutable encrypted revisions. |
| Sync | Delta pull by cursor | Server-visible cursor, encrypted payloads. |
| Sync | Optimistic concurrency | Stale base revision returns conflict. |
| Audit | Security/product events | No secret values in audit events. |
| CI | Docs, public-safety, tests | GitHub-hosted runners only. |
| Deploy | GitOps-compatible artifacts | No direct cluster mutation from product repo. |

## Post-MVP

| Area | Feature | Why delayed |
| --- | --- | --- |
| Devices | Explicit device enrollment | Requires final key hierarchy. |
| Recovery | Zero-knowledge recovery key | Must be designed before making recovery promises. |
| Identity | Email verification | Useful but not core to vault crypto. |
| Auth | WebAuthn/passkeys | Stronger MFA, but TOTP is first. |
| Organizations | Org accounts and memberships | Requires sharing, groups, and key wrapping. |
| Sharing | Shared vaults or collections | Requires per-member key distribution. |
| Clients | Browser extension | Should reuse the same sync and crypto model. |
| Clients | Desktop/mobile clients | Stronger client distribution, larger scope. |
| Import | KeePass/KDBX import | Migration feature, not MVP core. |
| Import | 1Password/Bitwarden import | Migration feature, not MVP core. |
| Integrations | CLI/API tokens | Requires careful secret and scope model. |
| Platform | Vault/OpenBao for app secrets | Platform ADR after MVP core design. |

## Explicit Non-Goals For MVP

- No server-side plaintext vault data.
- No server-side search over item contents.
- No admin recovery path for user vault contents.
- No plugin marketplace.
- No billing.
- No organization sharing.
- No public self-hosted GitHub Actions runner.
- No direct deployment from product CI to the home cluster.

## Product Risks

- Recovery codes can be misunderstood as vault recovery.
- Client-side search requires downloading encrypted vault data and decrypting locally.
- Multi-device unlock and key delivery are not yet specified.
- Browser JavaScript delivery is a residual risk for a web-only zero-knowledge MVP.
- Plaintext metadata decisions can leak account behavior even when item payloads are encrypted.
