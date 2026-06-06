# Research Note: Auth And Crypto V1

Status: bootstrap research note. Do not implement product code from this note alone.

## Why This Matters

Authentication, vault unlock, key derivation, item encryption, MFA, and recovery are coupled in a
zero-knowledge password manager. A simple implementation mistake can either expose user secrets,
create an offline guessing oracle, or make legitimate recovery impossible.

## Official Documentation Checked

- RFC 9807 OPAQUE.
- RFC 9106 Argon2.
- RFC 6238 TOTP.
- W3C WebCrypto.
- OWASP Authentication Cheat Sheet.
- OWASP Cryptographic Storage Cheat Sheet.
- OWASP Key Management Cheat Sheet.

## Current Direction

### Login Protocol

OPAQUE is a serious long-term candidate because it hides the password from the server and is now
published as RFC 9807. It is not automatically the MVP choice because implementation maturity across
Rust and browser clients must be reviewed.

The practical MVP fallback is a derived-auth-key flow:

- derive authentication material client-side;
- derive vault unlock/wrapping material separately;
- never send the raw unlock password to the backend;
- avoid storing a database value that enables cheap offline guessing after DB compromise;
- document and test every protocol step.

Simple password-over-TLS is rejected for the public MVP direction. It may be easy, but it weakens
the product's zero-knowledge posture and increases logging/mishandling risk.

### Browser KDF

Argon2id is the target KDF direction for password-derived material, but WebCrypto does not provide
Argon2id. Browser Argon2id requires a reviewed and pinned WASM dependency, deterministic test
vectors, supply-chain controls, and bundle-integrity review.

If the project falls back to WebCrypto-native PBKDF2 for an early prototype, that must be explicitly
documented as a tradeoff and not represented as the final security target.

### Item Encryption

For the browser MVP, AES-GCM is the likely first AEAD candidate because it is available through
WebCrypto. The final crypto ADR must define:

- key hierarchy;
- vault key wrapping;
- item payload format;
- AEAD algorithm;
- nonce generation and uniqueness guarantees;
- associated data;
- version fields;
- migration strategy.

### TOTP

TOTP is a login factor only. It is not a vault encryption key.

TOTP design must include:

- RFC 6238 vectors;
- accepted time-step window;
- replay protection using last accepted step tracking;
- per-account and per-IP throttling;
- recovery codes;
- encrypted-at-rest TOTP seed custody.

## Recommended Next Artifact

Create a dedicated ADR for the MVP auth and crypto protocol before implementation. It should include
message shapes, database columns, test vectors, threat analysis, and explicit rejected alternatives.

## Risks

- OPAQUE libraries may not be mature enough for MVP.
- Browser Argon2id/WASM dependency may introduce supply-chain risk.
- AES-GCM nonce misuse would be catastrophic.
- Browser-delivered JavaScript remains a structural web-MVP risk.
- TOTP is not phishing-resistant and should be complemented by WebAuthn/passkeys later.
- Recovery code handling can accidentally become an admin decrypt path if not designed carefully.

## Open Questions

- OPAQUE now or derived-auth-key first?
- Which browser Argon2id implementation, if any?
- AES-GCM versus another AEAD for non-browser clients later?
- Exact key hierarchy and wrapping strategy.
- Whether the MVP supports multiple devices at launch.
- How much metadata may remain plaintext for sync and UI.

## Sources

- https://www.rfc-editor.org/info/rfc9807/
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.rfc-editor.org/info/rfc6238/
- https://www.w3.org/TR/webcrypto/
- https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
