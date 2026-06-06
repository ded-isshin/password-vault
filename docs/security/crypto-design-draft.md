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

Working candidate: derived-auth-key MVP with OPAQUE as a future authentication-layer migration.

Rejected for public MVP: sending the master password to the server and hashing it there.

This decision affects authentication, server password hashing, device enrollment, vault key wrapping,
and recovery behavior. See [ADR 0003](../adr/0003-auth-and-crypto-direction.md).

### Browser KDF

Working candidate: one Argon2id pass through reviewed, pinned WASM, followed by HKDF domain
separation.

WebCrypto does not provide Argon2id. If the client uses Argon2id, the product needs a reviewed WASM
implementation, deterministic test vectors, supply-chain controls, and bundle-integrity review.

Fallback candidate: PBKDF2-HMAC-SHA-256 through WebCrypto, explicitly documented as weaker and
migration-ready. If used, the fallback must set a concrete minimum iteration count and require
explicit downgrade approval.

### Key Hierarchy

Working hierarchy:

```text
user password
  -> Argon2id(password, salt, params) -> master secret

master secret
  -> HKDF("password-vault/auth/v1") -> client auth secret
  -> HKDF("password-vault/unlock/v1") -> account unlock key

client auth secret
  -> server-side slow hash before storage

account unlock key
  -> unwrap user key material

user key material
  -> unwrap vault key

vault key
  -> encrypt/decrypt item revision payloads
```

Open: whether the MVP uses per-item keys or a single vault key for item payloads.

### Item Encryption

Working candidate:

- AEAD: AES-256-GCM through WebCrypto.
- Nonce: 96-bit nonce per encryption under a key.
- Budget: the crypto v1 spec must define a per-key encryption budget and rekey trigger before using
  long-lived vault keys with AES-GCM.
- Associated data: bind record type, crypto version, vault ID, item ID, revision ID, and key ID.
- Payload: versioned encrypted item revision.
- Migration: every encrypted artifact carries version and algorithm metadata.

### TOTP Seed Protection

Open.

TOTP seeds are server-side authentication secrets because the server must verify TOTP. They are not
vault encryption keys.

Future options:

- application-level encryption with a server-owned key
- Vault/OpenBao Transit for server-owned seed encryption
- another platform KMS path

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
- AES-GCM nonce uniqueness and rekey-budget tests
- server-side test proving raw client auth secret is not stored
- negative test that backend code cannot decrypt a stored item payload

## Sources

- https://www.rfc-editor.org/info/rfc9807/
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.rfc-editor.org/info/rfc6238/
- https://w3c.github.io/webcrypto/
- https://agilebits.github.io/security-design/
- https://bitwarden.com/help/bitwarden-security-white-paper/
