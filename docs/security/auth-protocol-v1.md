# Auth Protocol V1

Status: MVP implementation direction. Related issues: #2, #4, #13, #16, #24.

## Decision

Use `derived-auth-v1` for the first MVP implementation.

OPAQUE remains the preferred future authentication protocol, but it is not the MVP default until a
separate proof-of-concept proves browser/Rust interoperability, browser performance, and operational
handling.

## Goals

- Never send the raw master password to the backend.
- Keep server login separate from local vault unlock.
- Keep TOTP as login MFA only.
- Keep the API protocol-neutral enough to migrate to OPAQUE later.
- Store only a verifier derived from client-derived auth material.
- Keep vault key wrapping and item encryption client-side.

## Non-Goals

- No password-over-TLS auth for the public MVP.
- No OPAQUE implementation in the first MVP unless a separate PoC is accepted.
- No passkeys/WebAuthn in the first MVP.
- No admin recovery that can decrypt vault contents.
- No silent auth-protocol downgrade.

## Protocol Identifier

Every auth flow must include:

```text
auth_protocol = "derived-auth-v1"
```

Future OPAQUE migration should use:

```text
auth_protocol = "opaque-rfc9807-v1"
```

The server and client must reject silent protocol downgrades.

## Client Inputs

The browser uses:

- login handle;
- master password;
- account secret key;
- KDF salt and parameters;
- `auth_protocol`.

The master password and account secret key are never sent to the backend.

## Key Derivation Direction

```text
master password
  + account secret key
  -> Argon2id(combined input, account salt, params) -> master secret

master secret
  -> HKDF("password-vault/auth/v1") -> client auth secret
  -> HKDF("password-vault/unlock/v1") -> account unlock key
```

`client auth secret` is password-equivalent. It must not be sent to the backend raw and must never
be logged or stored raw.

The MVP verifier profile is:

```text
auth_verifier_profile = "pv-scram-sha-256-v1"
```

`pv-scram-sha-256-v1` adapts the SCRAM-SHA-256 proof model to the HTTP `/v1` API. The browser uses
`client_auth_secret` as the SCRAM password input and computes verifier material from a server-issued
auth-verifier salt and iteration count. The backend stores:

```text
auth_verifier_salt
auth_verifier_iterations
auth_stored_key
auth_server_key
```

The backend does not store the raw `client_auth_secret`. Login proof is bound to client/server
nonces and the login challenge. A copied verifier database may still enable offline guessing, but
the guessed input is the browser-derived `client_auth_secret`, which depends on the user's password
and account secret key. OPAQUE remains the preferred future mitigation for this class of risk.

## Registration Flow

```text
POST /v1/auth/register/start
  login_handle
  auth_protocol

server returns:
  registration_id
  auth_protocol
  kdf_profile
  account_salt
  auth_verifier_profile
  auth_verifier_salt
  auth_verifier_iterations
  csrf token or registration nonce if needed

client derives master_secret, client_auth_secret, account_unlock_key
client generates account_secret_key locally
client derives pv-scram-sha-256-v1 verifier material from client_auth_secret
client creates wrapped user/vault key material

POST /v1/auth/register/finish
  registration_id
  auth_protocol
  auth_stored_key
  auth_server_key
  encrypted account/vault key metadata
  initial device metadata

server stores:
  account record
  auth verifier material
  encrypted key metadata
  initial device record
```

The registration response must not cause the raw password, account secret key, account unlock key, or
unwrapped vault key to reach the backend.

Implementation note: the first runtime registration slice implements `register/finish` as one
transaction that consumes the registration challenge, creates the account, stores encrypted account
keyset metadata, creates the initial vault, stores the encrypted vault key wrap, creates the device
record, and creates a setup session in `mfa_enrollment_required` state. Session inspection, CSRF
issuance, logout, and TOTP enrollment/confirmation are now implemented as follow-up slices.

Registration must not become a login-handle enumeration endpoint. In the MVP, duplicate
`register/start` requests return the same `200` response shape as new-handle requests and create a
short-lived registration challenge with generated metadata. The server does not create an account at
`register/start`. Duplicate-handle conflict is enforced later at `register/finish` by the unique
`accounts.login_handle_normalized` constraint and returns a generic registration failure.

## Login Flow

```text
POST /v1/auth/login/start
  login_handle
  auth_protocol
  client_nonce

server returns constant-shape metadata:
  login_challenge_id
  auth_protocol
  kdf_profile
  account_salt
  auth_verifier_profile
  auth_verifier_salt
  auth_verifier_iterations
  server_nonce
  combined_nonce

client derives client_auth_secret and pv-scram-sha-256-v1 client proof

POST /v1/auth/login/finish
  login_challenge_id
  auth_protocol
  client_nonce
  server_nonce
  client_final_without_proof
  client_proof

server verifies auth material
server returns one of:
  mfa_required
  session_created
```

`login/start` must use constant-shape responses for existing and unknown accounts. Unknown-account
responses use deterministic synthetic metadata so account existence is not trivially exposed by the
response shape. Synthetic metadata must be derived with a server-side secret,
`PV_SYNTHETIC_METADATA_KEY_B64`, encoded as a 32-byte base64url-no-padding value. The key is a runtime
secret and must never be committed to the repository or printed in logs. Synthetic account and
auth-verifier salts use HMAC-SHA-256 with separate domain strings and the normalized login handle.
For the MVP, stored `auth_verifier_iterations` must remain pinned to the synthetic metadata default
of `150000`; allowing per-account iteration values would make `login/start` an account-existence
oracle even if the JSON key shape stays constant.

`login/start` must not return a pre-authenticated `mfa_required_hint`. MFA requirement is revealed
only after `login/finish` succeeds.

`derived-auth-v1` must not send the raw `client_auth_secret` as a reusable bearer credential. The
client submits a `pv-scram-sha-256-v1` proof derived from:

- `client_auth_secret`;
- `login_challenge_id`;
- server nonce;
- client nonce;
- `auth_protocol`;
- login handle;
- canonical request fields.

The implementation must define exact canonical encoding and include proof test vectors before #16 is
merged. TLS exporter/channel-binding input is deferred until browser support and deployment behavior
are reviewed.

For `derived-auth-v1`, `combined_nonce` is `base64url_no_pad(client_nonce || server_nonce)`, where
both nonces are decoded 32-byte values and `||` is byte concatenation in that order. The
`auth_challenges.public_metadata` JSON stores the decoded client nonce re-encoded as base64url, the
server nonce, the combined nonce, verifier profile metadata, and whether the challenge used synthetic
metadata.

## MFA Flow

If TOTP is enrolled, successful password/auth verification creates a pre-MFA login challenge, not a
full session.

```text
POST /v1/auth/mfa/totp/verify
  login_challenge_id
  totp_code

server verifies TOTP, replay window, and rate limits
server creates post-MFA server session
```

TOTP seed protection is server-owned secret management and is defined by
[ADR 0005](../adr/0005-mfa-session-and-csrf-policy.md). TOTP does not affect vault encryption.

Implementation note: setup-session TOTP enrollment endpoints are implemented. Enrollment start
creates a pending encrypted seed under `PV_TOTP_SEED_KEY_B64`; confirmation verifies the submitted
code, returns one-time recovery codes, rotates the session token, and upgrades the session to
`mfa_verified`. Login-finish and login-time TOTP verification remain planned.

## Session Flow

The MVP browser session uses a server-side session and a host-prefixed cookie.

Cookie direction:

```text
name: __Host-pv_session
Secure: true
HttpOnly: true
SameSite: Strict
Path: /
Domain: not set
```

State-changing browser requests require CSRF protection:

- CSRF token bound to the server-side session and sent in `X-PV-CSRF`;
- Origin check;
- Fetch Metadata checks where available;
- non-GET methods for mutations.

Implementation note: `GET /v1/session`, `GET /v1/csrf`, and `POST /v1/auth/logout` are implemented
as the session foundation for TOTP enrollment. CSRF tokens are random 32-byte values returned once
and stored only as SHA-256 verifiers. `GET /v1/csrf` rotates the verifier. Logout is idempotent
without a valid session but requires a current CSRF token when a valid session exists.

## Vault Unlock Boundary

Server session:

- authorizes API access;
- can list ciphertext and sync metadata;
- cannot decrypt vault item payloads.

Local unlock:

- happens in the browser;
- unwraps vault key material using `account_unlock_key`;
- enables local decrypt/search/edit.

## Stored Auth Fields

MVP schema should support:

- `auth_protocol_version`;
- `auth_verifier_profile`;
- `auth_verifier_salt`;
- `auth_verifier_iterations`;
- `auth_stored_key`;
- `auth_server_key`;
- `kdf_profile`;
- `account_salt`;
- nullable `opaque_credential_record`;
- auth migration status;
- failed-attempt/rate-limit state;
- pre-MFA challenge state;
- sessions.

## Required Tests

- Backend never receives the raw master password.
- Backend never receives the account secret key.
- Backend never stores raw `client_auth_secret`.
- Registration stores verifier material, not raw `client_auth_secret`.
- Login proof verifies against stored verifier material and is bound to login challenge nonces.
- `login/start` returns constant-shape metadata for existing and unknown accounts.
- `login/start` does not reveal MFA enrollment status.
- Registration duplicate-handle behavior does not trivially enumerate accounts.
- Wrong password fails with generic errors.
- Pre-MFA challenge cannot call post-MFA session-only endpoints.
- TOTP creates a session only after successful verification.
- CSRF-less mutation fails.
- Session cookie has required flags.
- Auth protocol downgrade is rejected.
- Recovery codes do not decrypt vault data.

## Accepted Residual Risk

`derived-auth-v1` stores verifier material that may enable offline guessing if an attacker obtains
the authentication database. The account secret key and browser KDF are required mitigations. A live
compromised backend may still abuse login flows or runtime secrets.

OPAQUE is the preferred future mitigation for this specific auth-channel risk. The MVP keeps auth
and vault unlock separate so migration does not require re-encrypting vault items.

## Sources

- https://www.rfc-editor.org/rfc/rfc5802.html
- https://www.rfc-editor.org/rfc/rfc7677.html
- https://www.rfc-editor.org/rfc/rfc9807.html
- https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
