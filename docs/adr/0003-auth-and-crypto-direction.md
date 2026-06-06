# ADR 0003: Auth And Crypto Direction

Status: proposed.

## Context

The product target is a zero-knowledge password manager. Authentication, vault unlock, key
derivation, item encryption, and recovery must be designed together.

This ADR is not yet a final cryptographic specification. It records the current direction and the
decisions that must be made before implementation.

## Options Considered

### OPAQUE / PAKE-Based Login

OPAQUE is an augmented password-authenticated key exchange protocol standardized as RFC 9807. It is
a strong long-term fit for password-based authentication because server compromise should not expose
a verifier that enables straightforward offline password guessing.

Pros:

- strong fit for "server never sees password";
- avoids sending the password over TLS to the server;
- provides a standardized protocol.

Cons:

- more implementation complexity;
- Rust and browser client library choices need careful review;
- may slow the MVP;
- does not remove the browser-delivered JavaScript residual risk.

### Derived-Auth-Key Flow

Mature password managers often derive authentication and encryption material on the client, then send
only a verifier or derived authentication value to the server.

Pros:

- practical and closer to known product patterns;
- easier than full PAKE;
- can preserve a zero-knowledge vault boundary if designed correctly.

Cons:

- must avoid creating an offline password guessing oracle after database compromise;
- protocol details are subtle and need careful documentation.

### Simple Password Over TLS

The browser sends the password over TLS and the server hashes it.

Pros:

- simplest implementation.

Cons:

- weakens zero-knowledge posture;
- exposes password to backend code and logs if mishandled;
- not appropriate for the public security direction unless explicitly limited to a throwaway internal
  prototype.

## Proposed Direction

Do not implement simple password-over-TLS for the public MVP.

Use a derived-auth-key design as the working MVP candidate, while keeping the key hierarchy
independent enough that OPAQUE can replace the authentication layer later.

OPAQUE remains the preferred long-term authentication protocol, but it should not be implemented
until Rust and browser library maturity, interoperability, and test strategy are reviewed.

## Crypto V1 Direction

Working direction:

- KDF target: Argon2id in the browser through a reviewed, pinned WASM dependency.
- KDF fallback: PBKDF2-HMAC-SHA-256 through WebCrypto only if Argon2id/WASM review is not ready.
- Key separation: run one expensive password KDF, then use HKDF domain separation for
  authentication and vault-unlock material.
- Server auth storage: any client-derived auth secret received by the server is treated as a
  password-equivalent secret and stored only as a slow server-side hash, never as a raw replayable
  value.
- Key wrapping: wrap vault keys client-side; do not send unwrapped vault keys to the server.
- AEAD for web MVP: AES-256-GCM through WebCrypto.
- Nonce: 96-bit nonce per encryption under a key, with an explicit per-key encryption budget and
  rekey trigger before implementation.
- Associated data: bind ciphertext to version, vault, item, revision, and key context.
- Versioning: store crypto version, KDF algorithm, KDF parameters, key ID, nonce, and payload format
  version with every encrypted artifact.

## WebCrypto And Argon2id

WebCrypto does not provide Argon2id. If Argon2id is selected for browser-side key derivation, the
project must review and pin a WASM dependency, test it against vectors, and document bundle-integrity
controls.

If a WebCrypto-native KDF is selected, the tradeoff against Argon2id must be explicit.

## Key Hierarchy Draft

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
  -> unwrap vault key copies available to the user

vault key
  -> encrypt/decrypt item revision payloads
```

Future organization sharing should use organization or vault keys wrapped for authorized members.
Server-side database relationships authorize record access, but cryptographic key wraps determine
who can decrypt.

## Payload Format Draft

Each encrypted item revision should carry:

- crypto version
- AEAD algorithm
- KDF version and parameters when relevant
- key ID
- nonce generation mode
- per-key encryption budget metadata or key epoch
- vault ID
- item ID
- revision ID
- nonce
- ciphertext
- creation timestamp

Associated data should bind at least:

```text
password-vault:item-revision:v1:<vault_id>:<item_id>:<revision_id>:<key_id>
```

## TOTP Direction

TOTP is a login factor only. It is not a vault encryption key.

TOTP design must include:

- RFC 6238-compatible test vectors;
- accepted time-step window;
- replay protection through last-used-step tracking;
- per-account and per-IP throttling;
- recovery codes;
- encrypted seed custody.

## Decision

Proposed, not final:

- Derived-auth-key flow is the MVP working candidate.
- OPAQUE is the preferred long-term authentication candidate after library review.
- Simple password-over-TLS is not acceptable for the public MVP.
- Argon2id/WASM is the KDF target; PBKDF2 is only a documented fallback.
- AES-256-GCM is the web MVP AEAD target.

## Consequences

- Product code should not start until the auth and crypto v1 design is accepted.
- Data model details such as `vault_key_wraps` depend on the final key hierarchy.
- TOTP seed custody is a server-owned secret-management decision, not a user-vault decrypt decision.
- Browser-delivered JavaScript remains an accepted residual risk and must be mitigated, not hidden.

## Required Tests

- Argon2id or PBKDF2 known-answer tests.
- HKDF domain separation tests.
- Server stores only a slow hash of received auth secret, not the raw auth secret.
- AES-GCM round-trip and tamper rejection.
- AES-GCM nonce uniqueness and rekey-budget tests.
- Associated-data tamper rejection.
- Wrong-password unwrap failure.
- Cross-user and cross-vault denial.
- RFC 6238 TOTP vectors.
- TOTP replay rejection for an already accepted time step.
- Crypto version parsing and migration tests once more than one version exists.
- Negative test that backend code cannot decrypt a stored item payload.

## Sources

- https://www.rfc-editor.org/info/rfc9807/
- https://www.w3.org/TR/webcrypto/
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.rfc-editor.org/info/rfc6238/
- https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
