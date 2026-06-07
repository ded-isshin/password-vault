# Auth Protocol V1

Status: MVP implementation direction. Related issues: #2, #13, #24.

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
- Store only a slow server-side hash of client-derived auth material.
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

`client auth secret` is password-equivalent. It may be sent to the backend only through the
`derived-auth-v1` login/registration proof path and must never be logged or stored raw.

The backend stores:

```text
server_auth_hash = slow_hash(client_auth_secret, server_params)
```

The exact server-side slow hash algorithm and parameters are an implementation prerequisite. Argon2id
is preferred if available in the backend build; otherwise a documented password-hashing alternative
must be selected before code.

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
  csrf token or registration nonce if needed

client derives master_secret, client_auth_secret, account_unlock_key
client generates account_secret_key locally
client creates wrapped user/vault key material

POST /v1/auth/register/finish
  registration_id
  auth_protocol
  registration proof/material derived from client_auth_secret and registration_id
  encrypted account/vault key metadata
  initial device metadata

server stores:
  account record
  server_auth_hash
  encrypted key metadata
  initial device record
```

The registration response must not cause the raw password, account secret key, account unlock key, or
unwrapped vault key to reach the backend.

Registration must not become a login-handle enumeration endpoint. Duplicate-handle handling must use
generic responses and defer user-visible conflict details until a point where enumeration risk is
explicitly accepted or mitigated. The implementation contract must define exact duplicate behavior
before code.

## Login Flow

```text
POST /v1/auth/login/start
  login_handle
  auth_protocol

server returns constant-shape metadata:
  login_challenge_id
  auth_protocol
  kdf_profile
  account_salt

client derives client_auth_secret

POST /v1/auth/login/finish
  login_challenge_id
  auth_protocol
  challenge-bound auth proof derived from client_auth_secret and login_challenge_id

server verifies auth material
server returns one of:
  mfa_required
  session_created
```

`login/start` must use constant-shape responses for existing and unknown accounts. Unknown-account
responses use deterministic synthetic metadata so account existence is not trivially exposed by the
response shape.

`login/start` must not return a pre-authenticated `mfa_required_hint`. MFA requirement is revealed
only after `login/finish` succeeds.

`derived-auth-v1` must not send the raw `client_auth_secret` as a reusable bearer credential. The
client submits a challenge-bound proof derived from:

- `client_auth_secret`;
- `login_challenge_id`;
- server nonce;
- client nonce;
- `auth_protocol`;
- login handle;
- optional future TLS exporter/channel-binding input if available and approved.

The exact proof construction, nonce encoding, and server verifier behavior must be specified with
test vectors before implementation. Until then, this document treats live-backend observation of
derived auth material as an accepted residual risk.

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

TOTP seed protection is server-owned secret management and is decided by #4. TOTP does not affect
vault encryption.

## Session Flow

The MVP browser session uses a server-side session and a host-prefixed cookie.

Cookie direction:

```text
name: __Host-pv_session
Secure: true
HttpOnly: true
SameSite: Strict or Lax by explicit decision
Path: /
Domain: not set
```

State-changing browser requests require CSRF protection:

- CSRF token or signed double-submit token;
- Origin check;
- Fetch Metadata checks where available;
- non-GET methods for mutations.

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
- `derived_auth_hash`;
- server-side auth hash profile;
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

`derived-auth-v1` sends password-equivalent auth material to the backend. A live compromised backend
that observes this material may be able to replay it until the account rotates credentials.

OPAQUE is the preferred future mitigation for this specific auth-channel risk. The MVP keeps auth
and vault unlock separate so migration does not require re-encrypting vault items.
