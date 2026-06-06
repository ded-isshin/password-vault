# ADR 0001: Initial Product Direction

Status: proposed.

## Context

`password-vault` is a planned public, Kubernetes-native password manager. The first product goal is
a personal vault MVP with TOTP MFA and encrypted vault item storage.

## Decision

Start with:

- public product repository
- Rust backend
- TypeScript frontend
- PostgreSQL product database
- zero-knowledge user vault model
- TOTP MFA in MVP
- WebAuthn/passkeys later
- GitHub Actions with GitHub-hosted runners
- GitOps deployment path through infrastructure repository

Do not use:

- KeePass/KDBX files as primary storage
- ClickHouse as primary storage
- Vault/OpenBao as the user-vault database or decrypt path
- direct cluster mutation from product repository

## Rationale

The product needs strong security boundaries and transactional product state. PostgreSQL plus
client-side encryption fits that better than file-per-user storage or analytics databases.

Rust is a good fit for a security-sensitive backend if the MVP remains narrow and avoids custom
cryptographic primitives.

Vault/OpenBao may be valuable for platform runtime secrets, but using it to decrypt user vault data
would weaken the zero-knowledge goal.

## Consequences

- A dedicated cryptographic design note is required before implementation.
- Threat modeling must be kept current.
- Browser-delivered cryptography is accepted as a residual web-MVP risk, not solved by headers alone.
- GitHub workflow must prevent secrets from entering public repo artifacts.
- Kubernetes deployment must wait for a GitOps and backup design.

## Open Questions

- Exact login protocol.
- Exact encryption format.
- Backup target.
- Secret-management approach for runtime infrastructure.
- Whether the first deployment is public immediately or staged behind stricter access controls.
