# Research Note: TOTP Seed Custody And MFA Hardening

Status: MVP decision note.

Date: 2026-06-07.

Issues: #4 prerequisite for #16.

Scope: personal web MVP authentication, TOTP enrollment, TOTP verification, MFA recovery codes,
server sessions, and CSRF requirements.

Non-goals:

- WebAuthn/passkeys.
- SMS, phone-call, or email OTP.
- Organization policy controls.
- Admin or support-assisted account recovery.
- Any recovery flow that decrypts a user's vault data.

## Why This Matters

`password-vault` is a zero-knowledge password manager. The backend may authenticate users and store
encrypted vault records, but it must not gain the ability to decrypt vault item contents.

TOTP is different from vault encryption. The server must be able to verify TOTP codes, so the TOTP
seed is a server-owned authentication secret. That makes seed custody a legitimate platform/security
problem, but it must stay outside the user vault decrypt path.

## Design Constraints

- TOTP is a login factor only.
- TOTP never derives, wraps, unwraps, rotates, or recovers vault encryption keys.
- MFA recovery codes recover login-factor access only.
- MFA recovery codes do not decrypt vault data and do not replace lost vault unlock material.
- A valid server session authorizes API calls but does not imply local vault unlock.
- All seed, OTP, recovery-code, session-token, and CSRF-token values are secret material and must be
  excluded from logs, metrics, traces, panic messages, audit payloads, and public docs.

## Sources Checked

- RFC 6238, TOTP: Time-Based One-Time Password Algorithm:
  <https://www.rfc-editor.org/rfc/rfc6238>
- OWASP Multifactor Authentication Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html>
- OWASP Authentication Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html>
- OWASP Session Management Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html>
- OWASP Cross-Site Request Forgery Prevention Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html>
- Google Authenticator Key URI Format, de facto maintainer source:
  <https://github.com/google/google-authenticator/wiki/Key-Uri-Format>

## Relevant Source Guidance

RFC 6238 defines TOTP as HOTP with a time-based moving factor, with Unix time, `T0 = 0`, default
`X = 30` seconds, and test vectors in Appendix B. It permits HMAC-SHA-1, HMAC-SHA-256, and
HMAC-SHA-512. It recommends random keys, protection against unauthorized access, secure channels,
a default 30-second time step, bounded validation windows, and one-time acceptance of an OTP after a
successful validation.

OWASP recommends MFA for login, standards-based TOTP as a common option, short OTP validity,
single-use OTPs, strict attempt limits, no OTP logging, secure MFA reset procedures, and stronger
controls before changing MFA factors.

OWASP authentication guidance requires throttling and account lockout/backoff design that balances
security and denial-of-service risk. OWASP session guidance requires meaningless, high-entropy,
server-side session identifiers and session renewal after authentication or other privilege changes.
OWASP CSRF guidance supports custom request headers for JSON APIs, same-origin checks, SameSite
cookies, and defense-in-depth rather than relying on one control.

The Google Authenticator Key URI format is the practical compatibility target for the QR/manual
enrollment URI. It uses `otpauth://totp/...`, a Base32 `secret`, issuer label/parameter handling,
and defaults compatible with SHA1, six digits, and a 30-second period.

## Decisions

### D1. MVP TOTP Format

Use RFC 6238 TOTP for the MVP.

MVP defaults:

- type: `totp`
- algorithm: `SHA1`
- digits: `6`
- period: `30`
- `T0`: `0`
- time source: Unix time in seconds
- implementation must support time counters larger than 32 bits
- seed length: 20 random bytes before Base32 encoding

Rationale:

- SHA1, six digits, and 30 seconds are the practical compatibility baseline for Google
  Authenticator-style apps.
- RFC 6238 allows SHA256 and SHA512, and the database schema already allows them, but Google
  Authenticator compatibility is better if the MVP starts with SHA1 defaults.
- A 20-byte seed matches the HMAC-SHA-1 output length guidance and gives much more entropy than the
  six-digit displayed OTP space.

### D2. TOTP Provisioning URI

Generate an `otpauth://totp/...` URI for QR display and a manual Base32 secret.

MVP URI shape:

```text
otpauth://totp/Password%20Vault:<account-label>?secret=<base32-secret>&issuer=Password%20Vault&algorithm=SHA1&digits=6&period=30
```

Rules:

- `<account-label>` is the user's normalized login handle or another approved non-secret display
  label.
- The issuer prefix and `issuer` parameter must both be present and must match.
- The secret is uppercase Base32 without padding.
- The QR/manual secret is displayed only during a pending enrollment flow.
- The raw seed is never returned by any endpoint after enrollment confirmation.

Rationale:

- Google Authenticator's de facto format recommends both issuer prefix and issuer parameter for
  compatibility and account disambiguation.
- Explicit algorithm/digits/period parameters make the MVP behavior auditable even though some
  authenticator implementations may ignore them and use compatible defaults.

### D3. Seed Generation

The server generates the TOTP seed with a cryptographically secure random number generator.

Rules:

- Generate 20 random bytes per account enrollment attempt.
- Never accept a user-supplied TOTP seed in the MVP.
- A new enrollment attempt replaces the previous unverified pending seed for that account.
- Enrollment confirmation must verify a valid TOTP code before the factor becomes active.
- A pending factor must not satisfy MFA during login.

Rationale:

- Server generation avoids accepting low-entropy or reused user-supplied seeds.
- Verify-before-activate prevents locking a user into a factor they did not successfully scan.

### D4. MVP Seed Custody

Use application-level AEAD for MVP seed encryption at rest.

Runtime key material:

- `PV_TOTP_SEED_KEY_B64`: base64url-no-padding 32-byte AEAD key supplied by Kubernetes Secret.
- MVP key id: `app-totp-seed-key-v1`, stored with encrypted seed rows.

Stored database fields:

- `seed_ciphertext`
- `seed_nonce`
- `seed_key_id`
- `algorithm`
- `digits`
- `period_seconds`
- `last_accepted_step`
- `verified_at`

AEAD direction:

- Use RustCrypto `chacha20poly1305` `0.10.1`, the latest stable non-RC release checked for this
  slice.
- Use XChaCha20Poly1305 with a 192-bit random nonce for each encryption.
- Associated data must bind at least:
  - purpose: `password-vault:totp-seed:v1`
  - `account_id`
  - `factor_id`
  - `seed_key_id`
  - `algorithm`
  - `digits`
  - `period_seconds`
- Decryption must fail closed if key id, associated data, ciphertext, nonce, or runtime key is wrong.

Operational rules:

- The application must not start TOTP enrollment/verification if the configured seed key is missing
  or malformed.
- The runtime key must not be committed to Git.
- The runtime key must not be printed in deployment, health, panic, tracing, metrics, or test output.
- Database backup alone should not expose TOTP seeds.
- Restoring accounts with active TOTP requires restoring both the database and the matching runtime
  seed-encryption key. If the key is lost, users need MFA recovery/reset; the lost key still must not
  become a vault decrypt problem.

Rationale:

- The current schema already supports encrypted seeds and key identifiers.
- Requiring Vault/OpenBao Transit for the first MVP would add an infrastructure dependency before the
  platform secret-management decision is approved.
- App-level AEAD is acceptable as an explicit interim path for a single-product MVP, provided the key
  is managed as a Kubernetes runtime secret and the future Transit/KMS migration remains possible.

### D5. Vault/OpenBao Transit Direction

Vault/OpenBao Transit or another KMS path is a preferred future hardening step, not an MVP blocker.

Migration direction:

- Keep `seed_key_id` meaningful so encrypted rows can identify app-level or Transit/KMS key versions.
- Add a migration plan before switching custody mode.
- Re-encrypt seeds online or opportunistically after successful TOTP verification.
- Keep Vault/OpenBao out of the user vault decrypt path.

Rationale:

- Transit/KMS can improve custody of server-owned authentication secrets.
- It cannot make TOTP zero-knowledge because the server still verifies TOTP.
- It must not be allowed to decrypt user vault items.

### D6. TOTP Validation Window

Use a bounded three-step validation window for MVP:

- previous step: `current_step - 1`
- current step: `current_step`
- next step: `current_step + 1`

Rules:

- Accept at most one matching candidate step.
- Persist the accepted step in `last_accepted_step`.
- Reject any candidate step less than or equal to `last_accepted_step`.
- Reject steps outside the one-step forward/backward window.
- Do not implement automatic drift tracking in MVP.

Rationale:

- RFC 6238 recommends a policy-bounded validation window and notes that forward/backward limits can
  be used for clock drift.
- One adjacent step in each direction handles normal device clock skew while keeping the window
  small.
- Persisting `last_accepted_step` enforces one-time use after successful validation.

Risk:

- Accepting `current_step + 1` can advance `last_accepted_step` early if the user's device clock is
  ahead. That is acceptable for MVP because subsequent older steps will be rejected until time catches
  up. It is safer than silently accepting repeated or lower steps.

### D7. TOTP Attempt Limits And Lockout

Use DB-backed counters for challenge attempts and account-level backoff.

MVP minimum:

- A pre-MFA challenge expires after 10 minutes.
- A pre-MFA challenge allows at most 5 TOTP or recovery-code attempts.
- A successful verification consumes the pre-MFA challenge.
- A failed verification increments the pre-MFA challenge attempt count.
- Reaching the attempt limit consumes or invalidates the challenge.
- Account-level failed authentication/MFA counters feed `failed_auth_count` and `locked_until`.
- Lockout/backoff must be tied to the account as well as any source-based signal.

Initial backoff policy:

- failures 1 through 4: no account lock, but audit each failure
- failure 5: lock account login attempts for 5 minutes
- each further failure in the same observation window doubles the lock duration up to 1 hour
- successful post-MFA login resets the account failure counter

Rationale:

- A six-digit TOTP has a small online search space, so strict attempt limits are mandatory.
- In a Kubernetes deployment with multiple pods, DB-backed counters are safer than per-pod in-memory
  counters.
- Source-only rate limits are easy to bypass and can also lock out users behind shared NATs; they are
  useful as extra signal, not as the only control.

### D8. MFA Recovery Codes

Generate recovery codes for account MFA recovery only.

MVP rules:

- Generate 10 recovery codes after TOTP enrollment confirmation.
- Each code has at least 128 bits of random entropy before formatting.
- Display recovery codes once.
- Store only one-way verifiers in `recovery_codes.code_hash`.
- Include `account_id` in the verification input.
- Use a server-side pepper if available without weakening deployability; otherwise rely on high
  entropy random codes and a modern hash.
- A recovery code can be used only after the primary account authentication proof succeeds.
- A recovery code substitutes for TOTP only for login-factor recovery and re-enrollment.
- A used recovery code is marked with `used_at` and cannot be reused.
- Recovery-code rotation requires a post-MFA session plus fresh reauthentication or a valid existing
  factor.

Rationale:

- Recovery codes are the only practical self-service MVP fallback for lost TOTP devices.
- They must not be confused with vault recovery. If the user lost the vault unlock material, recovery
  codes do not help decrypt the vault.

### D9. MFA Factor Change Controls

Changing or disabling TOTP is a high-risk action.

Rules:

- Do not allow TOTP disable, replacement, or recovery-code rotation based only on an existing
  session.
- Require fresh primary authentication plus an existing enrolled factor or a valid unused recovery
  code.
- Rotate the server session after successful factor replacement.
- Write audit events for enrollment start, enrollment confirm, failed MFA, recovery-code use,
  recovery-code rotation, TOTP disable, and TOTP replacement.
- Notify the user out of band if an email notification channel exists. If email is not implemented,
  show the event on next login and record the gap as an open risk.

Rationale:

- OWASP treats factor replacement as a takeover-sensitive process.
- A stolen session must not be enough to silently replace a user's MFA.

### D10. Session And CSRF Requirements For MFA Endpoints

Use server-side opaque sessions and explicit pre-MFA challenge state.

Session rules:

- A password/auth proof alone creates a pre-MFA challenge, not a full session, if TOTP is active.
- TOTP or recovery-code success creates a new post-MFA session.
- Session token is at least 256 bits of random entropy.
- Store only `SHA-256(session_token)` or a stronger opaque verifier in the database.
- Cookie name: `__Host-pv_session`.
- Cookie attributes: `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`, no `Domain`.
- Absolute session lifetime: 12 hours for MVP.
- Idle timeout: 30 minutes for MVP.
- No refresh token for MVP.
- Rotate session after login, MFA success, recovery-code use, password/auth material change, and MFA
  factor change.

CSRF rules:

- State-changing browser API requests must use JSON and reject simple content types.
- Require an `Origin` header matching the configured public application origin.
- Require a per-session CSRF token for state-changing routes, sent in `X-PV-CSRF`.
- Store only `SHA-256(csrf_token)` or a stronger verifier in the database.
- Add Fetch Metadata checks where browser support exists.
- Pre-MFA challenge endpoints must not set `__Host-pv_session`.
- Pre-MFA challenge identifiers must not be accepted as bearer sessions.

Rationale:

- Cookie-authenticated JSON APIs need CSRF controls even when SameSite is enabled.
- Server-side sessions allow revocation and reduce token payload leakage.
- `SameSite=Strict` is acceptable for MVP because there is no OAuth or cross-site login return flow.

## Rejected Options

### Plaintext TOTP Seeds In PostgreSQL

Rejected.

Reason: database compromise would directly bypass TOTP for every enrolled account.

### Hashing TOTP Seeds

Rejected.

Reason: the verifier must compute expected TOTP values. One-way hashing the seed prevents normal
verification. Hashing short-lived submitted OTP values is useful for some OTP systems, but it does
not solve TOTP seed custody.

### TOTP As Vault Decryption Or Vault Recovery

Rejected.

Reason: it breaks the zero-knowledge boundary. TOTP is a server-verified login factor and is too
small and operationally exposed to serve as vault encryption material.

### MFA Recovery Codes As Vault Recovery

Rejected.

Reason: recovery codes are server-verified account recovery factors. They must not unwrap vault keys
or replace lost account secret key/master password material.

### SMS, Phone Call, Or Email OTP For MVP

Rejected.

Reason: they add external dependencies, weaker security properties, delivery complexity, and more
personal data handling. They are not needed for the first browser MVP.

### Vault/OpenBao Transit As A Hard MVP Dependency

Deferred.

Reason: it is a strong future direction for server-owned authentication secrets, but it requires
separate platform deployment, unseal/recovery custody, RBAC, audit, backup, and operator runbooks.
The MVP can proceed with app-level AEAD while keeping the data model migration-compatible.

### Stateless JWT Browser Sessions

Rejected for MVP.

Reason: server-side opaque sessions are easier to revoke, easier to bind to MFA state, and avoid
putting auth state into bearer token payloads.

### Support-Assisted MFA Reset

Rejected for MVP.

Reason: there is no staffed identity-proofing process. Self-service recovery codes are the only
approved MVP MFA recovery path.

## Implementation Acceptance Criteria

Issue #16 is not implementation-ready unless the following are covered by code and tests.

### TOTP Algorithm Tests

- RFC 6238 Appendix B vectors pass for SHA1, SHA256, and SHA512.
- The vector test includes timestamps `59`, `1111111109`, `1111111111`, `1234567890`,
  `2000000000`, and `20000000000`.
- The implementation uses 64-bit or wider time-step arithmetic.
- The default MVP config produces SHA1, six-digit, 30-second TOTP values.

### Provisioning Tests

- Enrollment start creates a 20-byte seed and stores only encrypted seed material.
- The returned provisioning URI starts with `otpauth://totp/`.
- The URI includes matching issuer label prefix and `issuer=Password%20Vault`.
- The URI includes uppercase Base32 secret without padding.
- The URI includes `algorithm=SHA1`, `digits=6`, and `period=30`.
- Starting a second pending enrollment replaces the previous unverified pending factor.
- A pending factor with `verified_at IS NULL` does not satisfy login MFA.
- Enrollment confirmation with a wrong TOTP leaves the factor inactive.
- Enrollment confirmation with a correct TOTP sets `verified_at` and creates recovery codes.
- No endpoint returns the raw seed after confirmation.

### Seed Custody Tests

- `seed_ciphertext` does not contain the Base32 seed or raw seed bytes.
- Decryption with the wrong `PV_TOTP_SEED_KEY_B64` fails.
- Decryption with the wrong `PV_TOTP_SEED_KEY_ID` fails.
- AEAD associated-data tampering fails for `account_id`, `algorithm`, `digits`, and
  `period_seconds`.
- Re-encrypting the same seed produces a different nonce and ciphertext.
- TOTP enrollment and verification fail closed if the seed key is missing or malformed.
- Logs/traces/metrics from enrollment and verification contain no raw seed, Base32 secret, OTP code,
  recovery code, session token, or CSRF token.

### TOTP Verification And Replay Tests

- A correct current-step code succeeds.
- A correct `current_step - 1` code succeeds if it is greater than `last_accepted_step`.
- A correct `current_step + 1` code succeeds if it is greater than `last_accepted_step`.
- A `current_step - 2` code fails.
- A `current_step + 2` code fails.
- Reusing the same successful step fails.
- Any candidate step less than or equal to `last_accepted_step` fails.
- Two concurrent submissions of the same valid code result in exactly one success.
- Successful verification updates `last_accepted_step` in the same transaction that consumes the
  pre-MFA challenge and creates the post-MFA session.

### Attempt Limit And Lockout Tests

- A pre-MFA challenge expires after 10 minutes.
- A pre-MFA challenge accepts at most 5 TOTP or recovery-code attempts.
- The sixth attempt for the same pre-MFA challenge fails even if the code is otherwise correct.
- Failed TOTP attempts increment challenge attempt count.
- Reaching the attempt limit invalidates the challenge.
- The fifth failed auth/MFA attempt in the observation window sets `locked_until` at least 5 minutes
  in the future.
- Additional failures extend lock duration up to the configured 1-hour cap.
- Successful post-MFA login resets the account failure counter.
- Lockout checks run before expensive verification where practical.

### Recovery Code Tests

- Enrollment confirmation creates exactly 10 recovery codes.
- Each generated recovery code has at least 128 bits of entropy before display formatting.
- Recovery codes are displayed once and never returned again.
- Database rows contain only one-way code verifiers.
- A recovery code is rejected before primary account authentication succeeds.
- A valid unused recovery code after primary auth succeeds and marks `used_at`.
- Reusing the same recovery code fails.
- A recovery code allows TOTP re-enrollment but does not return vault keys or decrypt vault data.
- Recovery-code rotation invalidates old unused codes only after fresh reauthentication succeeds.

### Session And CSRF Tests

- Successful primary auth for a TOTP-enabled account creates only a pre-MFA challenge.
- Pre-MFA challenge state cannot call session-only or vault-sync endpoints.
- TOTP success creates a new post-MFA server session.
- Session cookie is named `__Host-pv_session`.
- Session cookie has `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`, and no `Domain`.
- Session token stored in PostgreSQL is a hash/verifier, not the raw token.
- Session token rotates after MFA success and MFA factor replacement.
- Expired, revoked, or idle sessions are rejected.
- State-changing requests without `X-PV-CSRF` are rejected.
- State-changing requests with mismatched CSRF token are rejected.
- State-changing requests with missing or mismatched `Origin` are rejected.
- State-changing requests using simple content types are rejected.
- GET/HEAD safe endpoints do not mutate MFA/session state.

### Zero-Knowledge Boundary Tests

- TOTP verification does not read, derive, unwrap, rotate, or return vault encryption keys.
- Recovery-code verification does not read, derive, unwrap, rotate, or return vault encryption keys.
- MFA disable/replacement does not change encrypted vault payloads.
- Audit events for MFA actions contain account/session metadata only, never vault plaintext, vault
  keys, TOTP seeds, OTP codes, recovery codes, session tokens, or CSRF tokens.

## Open Risks

- TOTP is not phishing-resistant. WebAuthn/passkeys should be added later as the stronger MFA path.
- App-level AEAD protects against database-only compromise, but not against a compromised runtime
  process that can read the seed-encryption key.
- Losing the seed-encryption key can lock users out of TOTP verification unless they have recovery
  codes or a future approved reset path.
- The MVP does not include an email notification channel, so out-of-band alerts for MFA changes may
  be unavailable at first.
- Strict SameSite is appropriate for MVP, but future cross-site integrations or OAuth-style flows may
  require a reviewed cookie/CSRF update.
- Accepting one future TOTP step improves clock-skew tolerance but can advance
  `last_accepted_step`; tests must cover that behavior explicitly.

## Recommendation

Proceed with #16 implementation using this MVP baseline:

- server-generated RFC 6238 TOTP seeds;
- Google Authenticator-compatible provisioning URI;
- app-level AEAD seed encryption with Kubernetes-provided runtime key;
- verify-before-activate;
- bounded `[-1, 0, +1]` validation window;
- persisted replay rejection through `last_accepted_step`;
- DB-backed attempt limits and account backoff;
- 10 single-use MFA recovery codes;
- server-side opaque sessions;
- strict cookie and CSRF controls;
- no TOTP or recovery-code path that can decrypt user vault data.
