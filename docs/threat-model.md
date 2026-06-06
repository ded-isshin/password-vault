# Threat Model

Status: bootstrap draft.

## Assets

- user master password or unlock secret
- user vault keys
- encrypted vault item payloads
- vault metadata
- TOTP seeds
- recovery codes
- sessions
- audit events
- PostgreSQL data and backups
- container images
- GitHub repository and CI logs
- Kubernetes runtime secrets

## Security Goals

- server cannot read plaintext vault item contents
- database compromise does not reveal plaintext vault item contents
- one user cannot access another user's vault records
- TOTP protects login but is not treated as vault encryption
- recovery does not create a server-side decrypt path
- logs and metrics never include secret values
- public repository does not expose private infrastructure data

## Initial Threats

- compromised database
- compromised backend service
- malicious or mistaken administrator
- cross-tenant authorization bug
- malicious public PR
- secret leakage in GitHub Actions logs
- compromised browser bundle
- stolen session cookie
- TOTP seed exposure
- backup exposure
- node or pod failure
- replayed TOTP code within an accepted time window
- loss of master password or unlock secret

## Initial Mitigations

- client-side vault encryption
- encrypted item revisions
- strict authorization tests
- public safety checks in CI
- GitHub-hosted runners only
- no secrets in untrusted PR workflows
- HttpOnly SameSite cookies for sessions
- MFA recovery codes
- TOTP replay protection and rate limiting
- off-node backups before real use
- GitOps deployment with human approval

## Accepted Residual Risks

The web MVP depends on JavaScript delivered by the same service the user is logging into. If that
delivery path is compromised, malicious JavaScript can steal unlock secrets before encryption.
Security headers, dependency review, and CI controls reduce risk but do not remove this structural
problem.

Forgotten master password or unlock material should be treated as unrecoverable unless a future
recovery design explicitly preserves zero-knowledge properties. MFA recovery codes recover login
factor access, not vault decryption.

The MVP should assume that item existence, item count, ciphertext size, and update timing are
observable to the server. The recommended MVP boundary encrypts titles, URLs, usernames, notes,
tags, and custom fields, which means the server cannot provide content search.

## Open Questions

- Exact login protocol and key derivation.
- Exact client-side encryption format.
- Metadata encryption boundaries.
- TOTP seed encryption strategy.
- TOTP replay window and last-used-step tracking.
- Backup destination and retention.
- Browser bundle integrity controls.
- WebAuthn/passkey timing.
- Single-device or multi-device MVP.
- Recovery key strategy.
- Lock and unlock timeout policy.
