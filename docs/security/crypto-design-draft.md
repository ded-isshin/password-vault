# Crypto V1 Design Note

Status: accepted for the browser MVP implementation. This note describes the implemented
`derived-auth-v1`, `pbkdf2-sha256-browser-v1`, `account-keyset-v1`, `vault-key-wrap-v1`,
`item-envelope-v1`, and `vault-checkpoint-v1` behavior. It is not a claim that the system is ready
for real user secrets; database backup/PITR/restore, trusted TLS, alert delivery, and operational
secret custody remain separate hard gates.

Related documents:

- [ADR 0003: Auth And Crypto Direction](../adr/0003-auth-and-crypto-direction.md)
- [Auth Protocol V1](auth-protocol-v1.md)
- [Vault Revision Freshness And Rollback Resistance](revision-freshness.md)
- [API Contract](../api-contract.md)

## Goals

- Keep the backend out of plaintext vault item data.
- Keep raw master passwords, account secret keys, local unlock keys, unwrapped vault keys, TOTP
  codes, recovery codes, and item plaintext out of API requests and server storage.
- Use browser-native WebCrypto for the first MVP instead of adding an unreviewed Argon2id WASM
  dependency.
- Version every authentication, key-wrap, item-envelope, and checkpoint format so future migrations
  can reject silent downgrade.
- Detect rollback or forked sync history for browsers that already have an origin-local checkpoint.

## Non-Goals

- No custom cryptographic primitive.
- No server-side decrypt path for user vault item payloads.
- No admin or support recovery path that can decrypt user vault data.
- No organization sharing, passkeys, OPAQUE, browser extension, or mobile-client crypto format in
  this MVP note.
- No claim that browser-delivered JavaScript is equivalent to a native, audited, reproducible
  client.

## Implemented Browser MVP Profiles

| Area | Implemented profile | Notes |
| --- | --- | --- |
| Auth protocol | `derived-auth-v1` | Browser derives auth material; backend verifies challenge-bound proof material. |
| Browser KDF | `pbkdf2-sha256-browser-v1` | PBKDF2-HMAC-SHA-256, 600,000 iterations, WebCrypto `deriveBits`. |
| Auth verifier | `pv-scram-sha-256-v1` | SCRAM-SHA-256-shaped verifier/proof model over the browser-derived auth secret. |
| Account keyset wrap | `account-keyset-v1` | AES-GCM under the browser-derived unlock key. |
| Vault key wrap | `vault-key-wrap-v1` | AES-GCM under the browser-derived unlock key. |
| Item envelope | `item-envelope-v1` | AES-256-GCM, 96-bit nonce, authenticated metadata, one derived content key per revision. |
| Vault integrity | `vault-crypto-v1` | HKDF-derived per-vault integrity key for change MAC and head-hash chain. |
| Local checkpoint | `vault-checkpoint-v1` | Origin-scoped append-only localStorage checkpoint metadata. |

## Browser KDF Decision

The first browser MVP uses PBKDF2-HMAC-SHA-256 through WebCrypto with 600,000 iterations because
WebCrypto exposes PBKDF2, HKDF, HMAC, SHA-256, AES-GCM, random bytes, and UUID generation, but it
does not expose Argon2id.

Argon2id remains the hardening target for password-derived material. Moving to Argon2id requires a
separate reviewed and pinned browser WASM dependency, deterministic vectors, bundle-integrity review,
performance testing on representative devices, and an explicit migration plan. PBKDF2 must not be a
silent fallback for accounts that are expected to use Argon2id.

The current PBKDF2 decision is a deliberate MVP tradeoff, not a statement that PBKDF2 is preferable
to memory-hard KDFs for long-term password-manager design. The account secret key reduces the risk of
a copied database enabling password-only guessing, but it does not remove the need to upgrade KDF
posture before real-secret readiness.

## Key Hierarchy

Current browser MVP hierarchy:

```text
master password
  + account secret key
  + account salt
  -> PBKDF2-HMAC-SHA-256(600000) -> master secret

master secret
  -> HKDF-SHA-256("password-vault/hkdf/auth-secret/v1") -> client auth secret
  -> HKDF-SHA-256("password-vault/hkdf/unlock-key/v1") -> account unlock key

client auth secret
  -> pv-scram-sha-256-v1 verifier/proof material

account unlock key
  -> unwrap account keyset metadata
  -> unwrap vault key metadata

vault key
  -> HKDF-SHA-256(vault_id, item_id, revision_id) -> item-revision content key
  -> HKDF-SHA-256(vault_id) -> vault integrity key

item-revision content key
  -> AES-256-GCM encrypt/decrypt exactly one item revision payload

vault integrity key
  -> HMAC-SHA-256 change MACs and head-hash chain verification
```

The account secret key is generated in the browser and must not be persisted server-side in
plaintext. Losing it can mean losing vault access unless a future zero-knowledge recovery or trusted
device-enrollment design is approved.

## Pre-Login Metadata

The browser needs KDF metadata before deriving auth material. `login/start` therefore uses
constant-shape responses for existing and unknown accounts. Existing accounts return stored metadata.
Unknown accounts return deterministic synthetic metadata derived from a runtime server secret. Errors
remain generic.

The server must not expose per-account KDF iteration differences in pre-login metadata. Current MVP
login metadata is pinned to the PBKDF2 profile and SCRAM verifier iteration count so the response
shape and values do not become an account-existence oracle.

## Item Encryption

The current item envelope format is:

```json
{
  "crypto_version": "item-envelope-v1",
  "key_id": "vault-item-key-vault-crypto-v1",
  "aead": "AES-256-GCM",
  "nonce": "<base64url-12-bytes>",
  "ciphertext": "<base64url-bytes>"
}
```

Plaintext fields currently encrypted in the browser item payload include title, URL, username,
password, and notes. The backend stores sync metadata and ciphertext; it does not decrypt or inspect
item payloads.

AES-GCM associated data is canonical JSON bytes containing:

- `record_type`
- `crypto_version`
- `aead`
- `vault_id`
- `item_id`
- `revision_id`
- `operation`
- `base_revision_seq`
- `base_head_seq`
- `key_id`

The browser derives a new item-revision content key for each item revision from the vault key, vault
ID, item ID, and revision ID. The current design therefore avoids reusing a long-lived item content
key for many AES-GCM encryptions. The nonce is 96 random bits generated by WebCrypto.

## Revision Freshness

AEAD associated data proves that ciphertext and metadata match, but it does not prove that the server
returned the latest known vault state. The current browser MVP implements a client-keyed per-vault
hash chain and origin-local checkpoint as described in
[Vault Revision Freshness And Rollback Resistance](revision-freshness.md).

Every accepted item change binds:

- operation;
- vault ID;
- item ID;
- revision ID;
- item revision sequence;
- vault head sequence;
- base item revision sequence;
- base vault head sequence and hash;
- previous head hash;
- item envelope hash;
- key ID;
- crypto version.

The backend stores the chain metadata and enforces optimistic concurrency. The unlocked client
verifies `change_mac`, `envelope_hash`, and `head_hash` before trusting or decrypting sync results.
This detects rollback for a browser origin that already has a newer checkpoint. A brand-new device
with no checkpoint can still be shown an older internally valid chain; cross-device freshness or a
transparency mechanism is post-MVP.

## TOTP Seed Protection

TOTP seeds are server-side authentication secrets because the server must verify TOTP. They are not
vault encryption keys and must never decrypt user vault data.

The MVP stores pending and confirmed TOTP seeds encrypted with application-level AEAD under a runtime
server key. This is an interim server-owned secret custody path. Vault/OpenBao Transit or another KMS
path may replace it later, but that future system must stay separate from user-vault decrypt keys.

## Recovery

MFA recovery codes recover login-factor access only. They do not unwrap vault key material and do not
recover a lost account secret key.

A future zero-knowledge recovery design may wrap vault material under a user-held recovery key shown
once during enrollment. That design is not approved in the MVP and must be documented separately
before implementation.

## Metadata Boundary

The MVP encrypts user-entered item content in the browser and leaves sync/operational metadata
visible to the backend. The backend can observe account IDs, vault IDs, item IDs, revision IDs,
operation type, timestamps, ciphertext size, and sync cadence.

This means the MVP has no server-side content search. Browser-side search is only possible after
local unlock and sync/decryption.

## Browser JavaScript Residual Risk

The browser client is delivered by the same service that stores ciphertext. A compromised server,
build pipeline, or dependency could serve malicious JavaScript that steals secrets before encryption
or after decryption. This is an accepted MVP residual risk, not a solved problem.

Current mitigations:

- no third-party runtime scripts in the browser MVP;
- security-sensitive code is committed in the public repo and covered by CI/self-tests;
- browser crypto uses WebCrypto instead of unreviewed crypto packages;
- auth, item envelope, and checkpoint formats are versioned;
- public-safety and PR review are required for changes.

Future hardening:

- reviewed native/extension/mobile clients;
- stronger build provenance and release signing;
- pinned/reviewed Argon2id WASM if adopted;
- browser bundle integrity strategy;
- tighter CODEOWNERS for security-sensitive paths.

## Implemented Test Coverage

Current implemented coverage includes:

- SCRAM-SHA-256 RFC 7677 vector coverage in Rust;
- TOTP RFC 6238 vector coverage in Rust;
- random token shape/change test;
- SHA-256 verifier known output test;
- account non-enumeration and synthetic metadata tests;
- registration/login/MFA/session/vault API tests;
- database migration tests for crypto-version constraints;
- Node/browser-API synthetic self-test for AES-GCM round trip and tamper rejection using the same
  protocol constants and WebCrypto-compatible semantics;
- browser checkpoint self-test;
- live synthetic journey coverage in CI/manual workflows for register, TOTP, login, unlock,
  encrypted item create, sync, read/decrypt, recovery-code login, and re-enrollment.

Remaining hardening tests before real-secret readiness:

- browser-side KDF known-answer vector committed near the WebCrypto implementation;
- explicit HKDF label/domain-separation vector coverage;
- typed canonical encoding, `envelope_hash`, `change_mac`, and `head_hash` vectors before a
  second-language client or real-secret readiness claim;
- nonce uniqueness and per-revision-key budget tests;
- stronger backend negative test proving server-only code cannot decrypt stored item envelopes;
- service-worker/cache/bundle-integrity tests if those surfaces are added;
- Argon2id/WASM vectors and dependency review if Argon2id is adopted.

## Sources

- https://www.w3.org/TR/webcrypto/
- https://www.rfc-editor.org/rfc/rfc5869.html
- https://www.rfc-editor.org/rfc/rfc6238.html
- https://www.rfc-editor.org/rfc/rfc7677.html
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.rfc-editor.org/info/rfc9807/
- https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
- https://agilebits.github.io/security-design/
- https://bitwarden.com/help/bitwarden-security-white-paper/
