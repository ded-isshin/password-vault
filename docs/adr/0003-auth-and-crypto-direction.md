# ADR 0003: Auth And Crypto Direction

Status: accepted for MVP planning and implemented by the current browser preview. Detailed
implementation behavior is tracked in [Crypto V1 Design Note](../security/crypto-design-draft.md),
[Auth Protocol V1](../security/auth-protocol-v1.md), and
[Vault Revision Freshness And Rollback Resistance](../security/revision-freshness.md).

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

### WebAuthn / Passkeys

WebAuthn is a W3C API for public-key credentials. Passkeys can provide phishing-resistant
authentication and are a strong fit for future MFA and passwordless login.

Pros:

- phishing-resistant when used correctly;
- no shared password secret is sent to the server;
- strong user experience on modern browsers and mobile platforms;
- good future fit for browser extension and mobile clients.

Cons:

- authenticates account access but does not automatically unlock encrypted vault payloads;
- device enrollment and recovery flows need careful design;
- policy choices around synced passkeys, device-bound credentials, attestation, and user
  verification need a separate review.

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

Use `derived-auth-v1` as the MVP authentication protocol, while keeping the key hierarchy independent
enough that OPAQUE can replace the authentication layer later.

Use a high-entropy account secret key as the second input to the browser KDF for the MVP. It follows
the same security idea as 1Password-style two-secret key derivation, but it changes UX, device
onboarding, and recovery. The first MVP must define that behavior before implementation.

OPAQUE remains the preferred long-term authentication protocol. Issue #24 found enough evidence to
justify a future PoC, but not enough evidence to make OPAQUE the MVP default.

WebAuthn/passkeys should be designed as a post-MVP phishing-resistant MFA and login path. They do not
replace the vault unlock design by themselves.

## Crypto V1 Direction

Working direction:

- KDF target: Argon2id in the browser through a reviewed, pinned WASM dependency.
- First browser MVP KDF: `pbkdf2-sha256-browser-v1`, PBKDF2-HMAC-SHA-256 with 600,000 iterations,
  through WebCrypto. This is an explicit MVP implementation decision so the browser flow can ship
  without an unreviewed WASM dependency.
- KDF fallback rule: PBKDF2 must not be a silent runtime fallback. Future KDF changes require an
  explicit migration/version decision.
- KDF input target: user password plus high-entropy account secret key.
- Key separation: run one expensive password KDF, then use HKDF domain separation for
  authentication and vault-unlock material.
- Server auth storage: any client-derived auth secret received by the server is treated as a
  password-equivalent secret and stored only as a slow server-side hash, never as a raw replayable
  value.
- Key wrapping: wrap vault keys client-side; do not send unwrapped vault keys to the server.
- AEAD for web MVP: AES-256-GCM through WebCrypto.
- Content key direction: derive a unique item-revision content key from vault/root data key material
  through HKDF, then encrypt exactly one payload under that content key.
- Nonce: 96-bit nonce per encryption, with one encryption per derived item-revision content key in
  the recommended MVP design.
- Associated data: bind ciphertext to version, vault, item, revision, and key context.
- Versioning: store crypto version, KDF algorithm, KDF parameters, key ID, nonce, and payload format
  version with every encrypted artifact.

## WebCrypto And Argon2id

WebCrypto does not provide Argon2id. If Argon2id is selected for browser-side key derivation, the
project must review and pin a WASM dependency, test it against vectors, and document bundle-integrity
controls.

If a WebCrypto-native KDF is selected, the tradeoff against Argon2id must be explicit.

Initial Argon2id parameter target should start from the OWASP minimum recommendation of 19 MiB
memory, 2 iterations, and parallelism 1, then be tuned on representative browsers and devices.

## Pre-Login Metadata

The browser needs KDF salt and parameters before deriving client-side auth material. The final
protocol must prevent this lookup from becoming a user-enumeration endpoint.

Required direction:

- constant-shape metadata responses;
- generic errors;
- stored KDF metadata for real accounts;
- deterministic synthetic metadata for unknown accounts;
- rate limits before expensive server-side auth-secret hashing;
- tests for account non-enumeration.

## Key Hierarchy Draft

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
  -> unwrap vault key copies available to the user

vault key
  -> HKDF(vault_id, item_id, revision_id, key_epoch) -> item-revision content key

item-revision content key
  -> encrypt/decrypt exactly one item revision payload
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

TOTP, recovery-code, session, and CSRF policy is accepted for MVP implementation planning in
[ADR 0005](0005-mfa-session-and-csrf-policy.md). The implementation must include:

- RFC 6238-compatible test vectors;
- accepted time-step window;
- replay protection through last-used-step tracking;
- per-account and per-IP throttling;
- recovery codes;
- encrypted seed custody.

## Decision

Accepted for MVP planning:

- Use `derived-auth-v1`.
- Do not use password-over-TLS.
- Keep OPAQUE as a future preferred migration path.
- Require protocol-neutral auth start/finish endpoints.
- Keep vault unlock and key wrapping separate from auth migration.

- Derived-auth-key flow is the MVP recommended login candidate.
- Account secret key / two-secret key derivation is the recommended MVP baseline, pending final UX,
  recovery, and new-device behavior.
- OPAQUE is the preferred long-term authentication candidate after library review.
- Simple password-over-TLS is not acceptable for the public MVP.
- WebAuthn/passkeys are post-MVP authentication and MFA candidates, not the first MVP blocker.
- Argon2id/WASM remains the KDF hardening target; PBKDF2 is the explicitly approved first browser
  MVP profile.
- AES-256-GCM is the web MVP AEAD target.
- The MVP must be multi-device-capable in data model and protocol even if the first client is only
  the browser web app.
- Use ADR 0005 for TOTP seed custody, recovery-code, session, and CSRF policy.

## Consequences

- Auth/session/MFA product code should follow the accepted API contract, auth protocol direction,
  and ADR 0005.
- Data model details such as `vault_key_wraps` depend on the final key hierarchy.
- TOTP seed custody is a server-owned secret-management decision, not a user-vault decrypt decision.
- Browser-delivered JavaScript remains an accepted residual risk and must be mitigated, not hidden.

## Required Tests

- Argon2id or PBKDF2 known-answer tests.
- HKDF domain separation tests.
- Server stores only a slow hash of received auth secret, not the raw auth secret.
- Pre-login metadata response non-enumeration tests.
- Server-side slow-hash rate-limit and anti-DoS tests.
- Registration generates an account secret key and does not persist it server-side in plaintext.
- AES-GCM round-trip and tamper rejection.
- AES-GCM nonce uniqueness and rekey-budget tests.
- Associated-data tamper rejection.
- Wrong-password unwrap failure.
- Cross-user and cross-vault denial.
- RFC 6238 TOTP vectors.
- TOTP replay rejection for an already accepted time step.
- Crypto version parsing and migration tests once more than one version exists.
- Negative test that backend code cannot decrypt a stored item payload.

## Related Notes

- [Auth login protocol options](../research/auth-login-protocol-options.md)

## Sources

- https://www.rfc-editor.org/info/rfc9807/
- https://pages.nist.gov/800-63-4/sp800-63b.html
- https://www.w3.org/TR/webauthn-3/
- https://www.w3.org/TR/webcrypto/
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.rfc-editor.org/info/rfc6238/
- https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
