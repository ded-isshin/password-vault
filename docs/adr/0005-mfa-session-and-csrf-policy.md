# ADR 0005: MFA, Session, And CSRF Policy

Status: accepted for MVP implementation planning. Related issues: #4, #13, #16.

## Context

The MVP needs a browser-first login flow with TOTP MFA, recovery codes, server-side sessions, and
CSRF controls. These controls protect account access only. They do not decrypt user vault data and
must not create a server-side vault recovery path.

The product remains zero-knowledge for vault item payloads:

- the raw master password never reaches the backend;
- the account secret key never reaches the backend;
- unwrapped vault keys never reach the backend;
- TOTP is a login factor only.

## Decisions

### TOTP Profile

Use RFC 6238-compatible TOTP for the first MFA factor.

MVP profile:

- algorithm: HMAC-SHA-1 for Google Authenticator compatibility;
- digits: 6;
- period: 30 seconds;
- epoch: Unix time, `T0 = 0`;
- seed size: 20 random bytes before Base32 encoding;
- provisioning URI: `otpauth://totp/...` with issuer `Password Vault`.

The implementation must include RFC 6238 deterministic tests and Google Authenticator URI tests.

### TOTP Seed Custody

The backend generates TOTP seeds with a cryptographically secure random generator and displays each
seed once during enrollment as QR/manual text.

Store only encrypted TOTP seed material:

- `seed_ciphertext`;
- `seed_nonce`;
- `seed_key_id`;
- `seed_aead`;
- metadata needed for algorithm, digits, period, and activation state.

For MVP, use application-level AEAD with a runtime key supplied as a Kubernetes/runtime secret. The
public repository may document the variable name, but must not contain the key value.

Runtime secret direction:

```text
PV_TOTP_SEED_KEY_B64=<base64url 32-byte key>
PV_TOTP_SEED_KEY_ID=<operator-chosen key id>
```

Vault/OpenBao Transit or another KMS may replace application-level AEAD later, but that is a
platform decision in the infrastructure repository. Vault/OpenBao must not be used to decrypt user
vault item payloads.

TOTP seed key rotation must be supported by metadata before real users:

- active key ID for new enrollments;
- retired key IDs for decrypting old seeds;
- rewrap path after successful verification or through an operator-approved maintenance job.

Database backups containing TOTP seed ciphertext are not sufficient for restore unless the runtime
seed-encryption key custody and restore process are also documented and tested.

### Enrollment

TOTP is not active until verify-before-activate succeeds.

MVP enrollment flow:

1. An authenticated session starts enrollment.
2. The server creates a pending factor and encrypted seed.
3. The server returns an `otpauth://` URI and manual Base32 seed exactly once.
4. The user submits the current TOTP code.
5. The server verifies the code, marks the factor active, and records the confirmed time step as
   used so the same code cannot immediately be reused.
6. The server creates recovery codes and returns them once.

Pending enrollment expires after 10 minutes and may be replaced by starting enrollment again.

### Verification Window And Replay

Accept at most one adjacent time step on either side of the server's current step:

```text
accepted_steps = current_step - 1, current_step, current_step + 1
```

The implementation must reject replay by storing `last_accepted_step` per TOTP factor and rejecting
any successful match whose step is less than or equal to that value.

The MVP does not persist per-device clock drift. If users repeatedly fail because of drift, the UI
should tell them to correct device time.

### Rate Limits And Lockout

Rate limits are required before real user secrets are accepted.

MVP policy:

- login start: account and source throttling hooks;
- auth finish: account and source throttling hooks;
- TOTP verify: account, challenge, and source throttling hooks;
- recovery-code verify: account, challenge, and source throttling hooks;
- generic errors for auth and MFA failures.

The first implementation may use PostgreSQL-backed counters. A separate Redis or edge rate limiter
can be added later if load requires it.

### Recovery Codes

Recovery codes recover login-factor access only. They do not decrypt vault data and must not unwrap
vault keys.

MVP policy:

- generate 10 recovery codes;
- each code has at least 128 bits of random entropy before formatting;
- display codes once;
- store only one-way verifiers with per-code salt;
- using a code consumes it permanently;
- rotating recovery codes invalidates all unused old codes;
- use of a recovery code requires re-enrolling TOTP before returning to normal account state.

### Sessions

Use server-side opaque sessions.

Cookie:

```text
name: __Host-pv_session
Secure: true
HttpOnly: true
SameSite: Strict
Path: /
Domain: not set
Max-Age: not set for MVP browser session cookie
```

Server-side session policy:

- store only a hash of the random session token;
- idle timeout: 30 minutes;
- absolute timeout: 12 hours;
- rotate session state after MFA verification and recovery-code use;
- logout deletes the current session;
- revoke-all deletes all sessions for the account;
- pre-MFA challenges are not sessions and cannot call authenticated endpoints.

### CSRF

Use layered CSRF controls for cookie-authenticated browser APIs:

- no state-changing action uses `GET`;
- require same-origin or trusted same-site `Origin` checks for state-changing requests;
- reject cross-site unsafe requests when Fetch Metadata headers show `Sec-Fetch-Site: cross-site`;
- require an `X-PV-CSRF` header for authenticated state-changing requests;
- issue CSRF tokens through `GET /v1/csrf` after a session exists;
- bind CSRF tokens to the server-side session and rotate them with the session.

`SameSite=Strict` is selected because the MVP has no cross-site OAuth or SSO return flow. If a
future flow needs cross-site top-level navigation, this decision must be revisited explicitly.

## Rejected Options

- Use TOTP as a vault encryption factor: rejected because it would make login MFA part of data
  recovery and would not fit normal authenticator app behavior.
- Store plaintext TOTP seeds: rejected.
- Store TOTP seeds in user vaults only: rejected for MVP because the server must verify login MFA
  before allowing session creation.
- Use Vault/OpenBao Transit for user vault item decryption: rejected because it would break the
  zero-knowledge boundary.
- Store session tokens plaintext: rejected.
- Rely on `SameSite` alone for CSRF: rejected because layered CSRF controls are cheap and testable.
- Make recovery codes decrypt vault contents: rejected.

## Required Tests

- RFC 6238 deterministic TOTP vectors.
- Google Authenticator URI contains issuer, label, Base32 secret, SHA1, 6 digits, and 30 second
  period.
- TOTP cannot become active without confirmation.
- Reusing the same accepted time step fails.
- Previous, current, and next time-step behavior is deterministic.
- TOTP seed ciphertext is stored, plaintext seed is not stored.
- Recovery codes are shown once, stored as one-way verifiers, and cannot be reused.
- Recovery-code login does not expose or alter vault decryption material.
- Pre-MFA challenge cannot call authenticated endpoints.
- Session cookie has `__Host-`, `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`, and no `Domain`.
- CSRF-less authenticated mutation fails.
- Cross-site unsafe request with Fetch Metadata headers fails.
- Auth and MFA failures return generic errors and do not log codes, seeds, passwords, or recovery
  codes.

## Risks

- TOTP is not phishing-resistant. WebAuthn/passkeys should be added after MVP.
- Application-level AEAD makes runtime key custody critical until Vault/OpenBao/KMS is designed.
- A live compromised backend can still observe login flow behavior and abuse runtime secrets.
- Browser-delivered JavaScript remains a structural web-MVP risk.
- `SameSite=Strict` may need revisiting if future login flows depend on cross-site redirects.

## Sources

- https://www.rfc-editor.org/rfc/rfc6238.html
- https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
- https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie
- https://github.com/google/google-authenticator/wiki/Key-Uri-Format
