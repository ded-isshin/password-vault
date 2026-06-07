# Auth And MFA Lifecycle

Status: MVP implementation direction. Detailed API shapes live in
[api-contract.md](api-contract.md). MFA/session policy is accepted in
[ADR 0005](adr/0005-mfa-session-and-csrf-policy.md).

This document describes intended account, session, TOTP, and account-recovery flows.

## Security Principles

- Login establishes a server session.
- Unlock establishes local browser access to vault decryption.
- A valid server session does not imply vault unlock.
- TOTP protects login. TOTP is not a vault encryption key.
- The MVP uses an account secret key as a second KDF input. It strengthens password-derived
  authentication and unlock material, but it makes new-device and lost-secret behavior a hard UX
  requirement.
- Recovery codes recover account MFA access only. They do not decrypt vault data.

## Registration Flow

```text
user enters login handle, master password, and account secret key
client performs derived-auth-v1 key-derivation flow
client creates or wraps vault key according to crypto design
server stores account auth metadata and encrypted vault metadata
server starts session or requires first login
user is prompted to enroll TOTP
```

The MVP auth protocol is `derived-auth-v1`. OPAQUE remains a preferred future migration path after a
separate browser/Rust interoperability proof-of-concept.

The final protocol must define when and how KDF salt and parameters are created, stored, and returned
to the client. The login metadata endpoint must not reveal whether an account exists.

## TOTP Enrollment Flow

```text
authenticated user starts enrollment
server generates TOTP seed
server stores seed encrypted with server-owned seed protection
client shows QR code and manual secret
user enters current TOTP code
server verifies code before activation and records the confirmed step as used
server marks TOTP verified
server displays one-time account recovery codes
user acknowledges recovery-code custody
```

TOTP seed custody is a server-side secret-management problem. The seed must not become a vault
decrypt key.

MVP policy:

- server generates a 20-byte random seed;
- seed is displayed once during enrollment as QR/manual code;
- default TOTP profile is SHA1, 6 digits, 30 second period, `T0 = 0`;
- seed is stored encrypted with application-level AEAD under a runtime key;
- `PV_TOTP_SEED_KEY_B64` is a runtime secret, never repository content;
- stored seed metadata includes key ID `app-totp-seed-key-v1` and AEAD profile
  `xchacha20poly1305-v1` for future rotation;
- Vault/OpenBao Transit or another KMS path is a future infrastructure/platform decision.

The provisioning URI follows the Google Authenticator `otpauth://totp/...` format for the MVP
browser flow.

Pending enrollment is bound to the setup/recovery session and its idle/absolute expiry. A pending
factor is not usable for login until confirmation succeeds.

## Login Flow

```text
user submits login handle
client obtains constant-shape login metadata
client derives and submits a challenge-bound derived-auth-v1 proof
server validates account authentication
server creates a pre-MFA challenge if TOTP is enabled
user submits TOTP code
server checks replay/rate-limit state
server creates server-side session
client performs local vault unlock if needed
```

The pre-login metadata flow must return KDF salt and parameters without making account existence easy
to enumerate.

The pre-login metadata flow must not reveal whether TOTP is enrolled. MFA requirement is revealed
only after password/auth proof succeeds.

## Recovery Code Flow

```text
user enters login handle and approved auth proof
server asks for TOTP
user selects account recovery code
server verifies one-time recovery code hash
server invalidates the used code
server lets user re-enroll TOTP
```

Recovery codes must be labeled as account MFA recovery, not vault recovery.

MVP recovery-code policy:

- generate 10 recovery codes;
- each code has at least 128 bits of random entropy before formatting;
- display codes once;
- store only one-way verifiers with per-code salt;
- consume a code permanently after use;
- require TOTP re-enrollment after recovery-code use.

Recovery-code use must not expose, unwrap, rotate, or otherwise alter vault decryption material.

## TOTP Verification Policy

The MVP accepts at most one adjacent 30-second step on either side of the server's current step:

```text
current_step - 1
current_step
current_step + 1
```

The server stores `last_accepted_step` per active factor and rejects any accepted match whose step is
less than or equal to that value. Enrollment confirmation records the confirmed step so the same code
cannot be used immediately for login.

The MVP does not persist per-device clock drift. Repeated drift failures should guide the user to
fix device time.

## Session And CSRF Flow

```text
auth proof succeeds
server creates pre-MFA challenge if MFA is active
MFA succeeds
server creates post-MFA server-side session
server sets __Host-pv_session cookie
browser obtains CSRF token through GET /v1/csrf
state-changing API calls send X-PV-CSRF and pass Origin/Fetch Metadata checks
```

Cookie policy:

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

Authenticated state-changing requests require:

- non-GET method;
- valid session;
- `X-PV-CSRF` header bound to the session;
- acceptable `Origin`;
- Fetch Metadata rejection for cross-site unsafe requests where headers are present.

## Required Tests

- TOTP cannot be enabled without verify-before-activate.
- Replayed TOTP step is rejected.
- Rate limits apply to password/auth and TOTP attempts.
- Login metadata lookup does not expose account existence through response shape.
- Login metadata lookup does not expose MFA enrollment status before auth proof succeeds.
- Registration duplicate-handle behavior does not trivially enumerate accounts.
- Expensive server-side auth verification is protected by rate limits.
- Account secret key is not persisted server-side in plaintext.
- Used recovery code cannot be reused.
- Recovery code does not reveal or change vault decryption material.
- Logs never include TOTP seeds or recovery codes.
- Session cookie flags are enforced.
- CSRF-less authenticated mutation fails.
- Cross-site unsafe request with Fetch Metadata headers fails.

## Open Decisions

- Email verification timing.
- Whether recovery key is included in MVP.
- Exact pre-login KDF metadata behavior.
- Account secret key UX: emergency kit only, remember-device option, or both.
