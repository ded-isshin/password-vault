# Crypto Design Draft

Status: incomplete draft. Do not implement product code from this document yet.

## Goal

Define a zero-knowledge vault cryptography design before implementation.

## Non-Goals

- No custom cryptographic primitives.
- No server-side plaintext vault item storage.
- No server-side decrypt path for user vault item payloads.
- No admin recovery path that can decrypt user vault data.

## Required Decisions

### Login And Key Derivation

Open.

Options to analyze:

- PAKE-based login such as OPAQUE.
- Derived-auth-key flow similar in spirit to mature password managers.
- Simpler internal MVP flow with explicit limitations.

This decision affects authentication, server password hashing, device enrollment, vault key wrapping,
and recovery behavior.

### Browser KDF

Open.

WebCrypto does not provide Argon2id. If the client uses Argon2id, the product needs a reviewed WASM
implementation, deterministic test vectors, supply-chain controls, and bundle-integrity review.

Alternative: use browser-native primitives and document the tradeoffs.

### Key Hierarchy

Open.

The future design must define:

- user unlock input
- KDF salt and parameters
- account authentication material
- user key material
- vault key
- item key strategy
- key wrapping strategy
- crypto version fields

### Item Encryption

Open.

The future design must define:

- AEAD algorithm
- nonce generation
- associated data
- payload format
- revision format
- migration path

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

## Test Requirements

- published vectors for KDF where applicable
- published or independently generated TOTP vectors
- deterministic encryption/decryption tests
- wrong-password denial
- wrong-user/cross-vault denial
- replayed TOTP denial
- crypto-version migration tests after more than one version exists
