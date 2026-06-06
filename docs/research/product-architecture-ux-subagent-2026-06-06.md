# Research Note: Product Architecture And UX Subagent 2026-06-06

Status: draft.

## Purpose

Capture an independent no-edit product architecture and UX review before product code exists.

## Reviewer Scope

The reviewer was asked to analyze:

- core MVP features
- data model boundaries
- item/revision sync
- lock/unlock UX
- TOTP enrollment and recovery UX
- future plugin/integration model
- what to postpone

The reviewer did not modify files.

## Key Findings Accepted

### Crypto Core Is The Product

The operational layer is better specified than the cryptographic product core. The auth,
key-derivation, recovery, and sync model must be settled before product code starts.

### Session Is Not Unlock

The product needs a clear state model:

- logged out
- logged in but locked
- logged in and unlocked

A valid server session authorizes API access, but does not decrypt vault data.

### Recovery Codes Need Careful Naming

TOTP recovery codes recover account MFA access only. They do not recover vault decryption. Users may
assume "recovery" means full vault recovery unless the product explicitly says otherwise.

### Plaintext Metadata Boundary Must Be Explicit

If title, URL, username, or tags are plaintext, the service leaks sensitive behavior. The accepted
draft direction is to encrypt those fields for MVP and accept client-side search only.

### Sync Needs A Revision Protocol

Encrypted sync should use immutable revisions, a per-vault change cursor, tombstones for deletion,
and optimistic concurrency. Server-side merge of encrypted content is out of scope for MVP.

### Plugin Model Is Too Early

Plugins and marketplace features should be postponed. The MVP should still version item payloads and
API routes so future clients, importers, and integrations are possible.

## Accepted Documentation Changes

- Add `docs/feature-map.md`.
- Add `docs/data-model.md`.
- Add `docs/sync-protocol.md`.
- Add `docs/auth-mfa-lifecycle.md`.
- Add `docs/lock-unlock-state.md`.
- Update product brief, architecture, threat model, and crypto draft with the accepted blockers.

## Rejected Or Deferred Suggestions

No suggestion was rejected outright. The following are deferred:

- Full vault recovery key design.
- Multi-device enrollment design.
- Browser extension UX.
- Plugin marketplace.
- Organization sharing.

## Next Artifacts Recommended

- ADR: single-device vs multi-device MVP.
- ADR: auth/login and key-derivation protocol.
- ADR/spec: crypto v1 payload and key hierarchy.
- ADR/spec: TOTP seed custody.
- ADR/spec: PostgreSQL HA, backup, and restore.
- API surface draft for `/v1`.
- Test matrix for auth, crypto, sync, authorization, and public-safety checks.
