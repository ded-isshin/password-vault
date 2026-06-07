# Crypto Design Draft

Status: direction draft. Do not implement product code from this document until the auth and crypto
ADR is accepted and converted into a precise implementation specification.

## Goal

Define a zero-knowledge vault cryptography design before implementation.

## Non-Goals

- No custom cryptographic primitives.
- No server-side plaintext vault item storage.
- No server-side decrypt path for user vault item payloads.
- No admin recovery path that can decrypt user vault data.

## Required Decisions

### Login And Key Derivation

Working candidate: derived-auth-key MVP with account secret key as the recommended second KDF input,
and OPAQUE as a future authentication-layer migration.

Rejected for public MVP: sending the master password to the server and hashing it there.

This decision affects authentication, server password hashing, device enrollment, vault key wrapping,
and recovery behavior. See [ADR 0003](../adr/0003-auth-and-crypto-direction.md).

### Browser KDF

Working candidate: one Argon2id pass through reviewed, pinned WASM, followed by HKDF domain
separation.

WebCrypto does not provide Argon2id. If the client uses Argon2id, the product needs a reviewed WASM
implementation, deterministic test vectors, supply-chain controls, and bundle-integrity review.

Initial Argon2id parameter target should start from the OWASP minimum recommendation:

```text
memory: 19 MiB
iterations: 2
parallelism: 1
```

The final values must be tuned on representative browsers and devices before implementation.

First browser MVP profile: PBKDF2-HMAC-SHA-256 through WebCrypto with 600,000 iterations,
explicitly documented as weaker than the Argon2id target and migration-ready. PBKDF2 must not be a
silent runtime fallback; it is the current explicitly approved browser-MVP profile because Argon2id
requires a reviewed WASM dependency.

### Pre-Login KDF Metadata

The browser needs KDF salt and parameters before it can derive client-side auth material. This
creates a pre-login endpoint design problem.

The final auth protocol must define an enumeration-resistant metadata flow:

- constant-shape responses for existing and non-existing accounts;
- stored KDF metadata for existing accounts;
- deterministic synthetic metadata for non-existing accounts;
- generic errors;
- rate limits before expensive server-side verification;
- backup and rotation handling for any server secret used to generate synthetic metadata.

### Key Hierarchy

Working hierarchy:

```text
user password
  + account secret key
  -> Argon2id(combined input, salt, params) -> master secret

master secret
  -> HKDF("password-vault/auth/v1") -> client auth secret
  -> HKDF("password-vault/unlock/v1") -> account unlock key

client auth secret
  -> server-side slow hash before storage

account unlock key
  -> unwrap user key material

user key material
  -> unwrap vault key / root data key

vault key / root data key
  -> HKDF(vault_id, item_id, revision_id, key_epoch) -> item-revision content key
  -> HKDF("password-vault/vault-integrity/v1", vault_id) -> vault integrity key

item-revision content key
  -> encrypt/decrypt exactly one item revision payload

vault integrity key
  -> HMAC-SHA-256 per-vault state hash chain
```

Recommended direction: derive a unique content key per item revision. This keeps the vault key as
wrapping/root material and reduces AES-GCM nonce-budget risk for the MVP.

The account secret key must not be persisted server-side in plaintext. Losing it can become
equivalent to losing vault access unless a separate zero-knowledge recovery or device-enrollment path
is approved.

### Item Encryption

Working candidate:

- AEAD: AES-256-GCM through WebCrypto.
- Nonce: 96-bit nonce per encryption under a key.
- Budget: one encryption per item-revision content key in the recommended MVP design. If long-lived
  item or vault content keys are used instead, the crypto v1 spec must define a per-key encryption
  budget and rekey trigger before implementation.
- Associated data: bind record type, crypto version, vault ID, item ID, revision ID, and key ID.
- Payload: versioned encrypted item revision.
- Migration: every encrypted artifact carries version and algorithm metadata.

### Revision Freshness

Associated data does not prove the server returned the latest state. The MVP must use a client-keyed
per-vault state hash chain and local client checkpoints, as specified in
[Vault Revision Freshness And Rollback Resistance](revision-freshness.md).

The hash-chain key is derived from unlocked vault material and is never sent to the backend.

Every encrypted item write must bind:

- previous vault state hash;
- new vault sequence;
- operation type;
- vault ID;
- item ID;
- item revision ID;
- item revision sequence;
- base item revision sequence;
- base vault head sequence and hash;
- key ID and crypto version;
- hash of the encrypted envelope.

MVP operation values are `create`, `update`, and `delete`. A deletion is an authenticated deletion
revision, not a server-only metadata flag.

The client computes a `change_mac` over the client-controlled change fields and encrypted envelope
hash. The chain head then binds the server-ordered vault head sequence, previous head hash, and
`change_mac`.

The backend stores the chain head and enforces optimistic concurrency, but clients verify the
`change_mac` and chain. This detects rollback for clients that have a newer local checkpoint. It does
not fully protect a new device with no checkpoint; that stronger transparency problem is post-MVP.

Implementation prerequisite: define typed canonical encoding and test vectors for `envelope_hash`,
`change_mac`, and `head_hash` before writing product code.

### TOTP Seed Protection

Open.

TOTP seeds are server-side authentication secrets because the server must verify TOTP. They are not
vault encryption keys.

Future options:

- application-level encryption with a server-owned key
- Vault/OpenBao Transit for server-owned seed encryption
- another platform KMS path

Recommended staged direction: use app-level AEAD only as an explicitly documented MVP interim path
if Vault/OpenBao is not deployed. Prefer Vault/OpenBao Transit or another KMS path once the platform
decision is approved. In every design, the TOTP seed-protection key must not decrypt user vault
items.

### Recovery

Open.

MFA recovery codes may recover login-factor access. They must not silently recover vault decryption
unless a future zero-knowledge-compatible recovery design is approved.

Potential future recovery-key design:

- generate a high-entropy recovery key during registration
- use it to wrap a copy of vault key material
- show it once to the user
- never store the plaintext recovery key on the server

This is not approved for implementation yet, but the key hierarchy should not accidentally make this
impossible.

### Metadata Boundary

Open, but the recommended MVP default is conservative:

- encrypt title, URL, username, password, notes, tags, and custom fields
- keep only sync metadata visible to the server
- accept that server-side content search is not available in MVP

## Test Requirements

- published vectors for KDF where applicable
- published or independently generated TOTP vectors
- deterministic encryption/decryption tests
- wrong-password denial
- wrong-user/cross-vault denial
- replayed TOTP denial
- crypto-version migration tests after more than one version exists
- AEAD associated-data tamper rejection
- vault state hash-chain tamper and rollback rejection
- client change MAC tamper rejection
- AES-GCM nonce uniqueness and rekey-budget tests
- server-side test proving raw client auth secret is not stored
- server-side rate-limit and anti-DoS tests around slow auth-secret hashing
- pre-login metadata tests for constant-shape responses and non-enumeration behavior
- account secret key, emergency-kit, and new-device requirements
- negative test that backend code cannot decrypt a stored item payload

## Sources

- https://www.rfc-editor.org/info/rfc9807/
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.rfc-editor.org/info/rfc6238/
- https://w3c.github.io/webcrypto/
- https://agilebits.github.io/security-design/
- https://bitwarden.com/help/bitwarden-security-white-paper/
