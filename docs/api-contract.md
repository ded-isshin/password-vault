# API Contract

Status: MVP implementation contract draft. This document is concrete enough for backend and
frontend MVP work, but a machine-readable OpenAPI or typed contract should be added before broad
client expansion.

Related issues: #4, #13, #16, #18.

## Purpose

`password-vault` is API-first. The browser web app is the first client of the product API, not a
special backend-only flow. Future browser extension, mobile, desktop, CLI, and integration clients
should reuse the same versioned contracts.

API-first does not mean public unauthenticated access. It means stable, documented contracts for
authorized clients.

## Versioning

Initial product namespace:

```text
/v1
```

Kubernetes probes are currently unversioned:

```text
GET /healthz
GET /readyz
```

Breaking product API changes require a versioning or migration decision before implementation.

## Common Conventions

- Request and response bodies are JSON unless explicitly stated.
- Requests with a body must use `Content-Type: application/json`.
- State-changing `/v1` routes reject browser-simple form content types:
  `application/x-www-form-urlencoded`, `multipart/form-data`, and `text/plain`.
- Byte strings are Base64url without padding.
- IDs are UUID strings unless the endpoint says otherwise.
- Timestamps are RFC 3339 UTC strings.
- Unknown JSON fields are rejected for security-sensitive endpoints.
- State-changing endpoints require non-GET methods.
- Authenticated state-changing browser requests require `X-PV-CSRF`.
- Auth, MFA, session, and CSRF responses include `Cache-Control: no-store`.
- Clients may send `X-Request-Id`; the server may echo or replace it. Request IDs are correlation
  metadata only and must not contain secrets or free-form secret-bearing text.
- Sensitive values must not be logged: passwords, account secret keys, raw client auth secrets,
  TOTP seeds, TOTP codes, recovery codes, unwrapped vault keys, plaintext item fields.
- Current auth router requests are capped at 128 KiB for the MVP. Auth start payloads should remain
  small, but a tighter per-route 16 KiB limit is planned rather than currently enforced.

### Error Envelope

Generic error shape:

```json
{
  "error": {
    "code": "auth_failed",
    "message": "Authentication failed."
  }
}
```

Security-sensitive endpoints use generic errors. They must not reveal whether a login handle exists,
whether TOTP is enrolled before auth succeeds, or whether a recovery code was close to valid.

Common codes:

```text
bad_request
auth_failed
mfa_required
session_required
csrf_required
forbidden
not_found
conflict
rate_limited
registration_unavailable
vault_conflict
```

Rate-limited responses use HTTP `429` and may include:

```text
Retry-After
X-RateLimit-Limit
X-RateLimit-Remaining
X-RateLimit-Reset
```

Header values must not expose account existence.

## Sensitive Boundary

Never sent to backend:

- raw master password;
- account secret key;
- account unlock key;
- unwrapped user or vault keys;
- plaintext item title, URL, username, password, notes, tags, or custom fields.

Sent only as protocol-safe derived material:

- `auth_stored_key` and `auth_server_key` during registration;
- `client_proof` during login finish.

Stored server-side:

- auth verifier metadata;
- encrypted TOTP seed;
- one-way recovery-code verifiers;
- session token hash;
- encrypted vault key wraps and item ciphertext;
- sync metadata required for authorization, conflict checks, and client-side integrity.

## Auth Protocol

MVP protocol:

```text
auth_protocol = "derived-auth-v1"
auth_verifier_profile = "pv-scram-sha-256-v1"
```

OPAQUE future protocol:

```text
auth_protocol = "opaque-rfc9807-v1"
```

The public API uses protocol-neutral start/finish endpoints so OPAQUE can be introduced later
without changing TOTP/session/vault endpoints.

`pv-scram-sha-256-v1` is the MVP verifier/proof profile documented in
[docs/security/auth-protocol-v1.md](security/auth-protocol-v1.md). The server stores verifier
material, not the raw `client_auth_secret`.

## Registration

### `POST /v1/auth/register/start`

Request:

```json
{
  "login_handle": "user@example.com",
  "auth_protocol": "derived-auth-v1"
}
```

Response `200`:

```json
{
  "registration_id": "00000000-0000-4000-8000-000000000001",
  "auth_protocol": "derived-auth-v1",
  "kdf_profile": {
    "id": "pbkdf2-sha256-browser-v1",
    "algorithm": "PBKDF2-HMAC-SHA-256",
    "iterations": 600000,
    "hash": "SHA-256"
  },
  "account_salt": "<base64url-32-bytes>",
  "auth_verifier_profile": "pv-scram-sha-256-v1",
  "auth_verifier_salt": "<base64url-32-bytes>",
  "auth_verifier_iterations": 150000,
  "expires_at": "2026-06-07T00:10:00Z"
}
```

The response shape is generic. Duplicate login handles are not revealed at start.

### `POST /v1/auth/register/finish`

Request:

```json
{
  "registration_id": "00000000-0000-4000-8000-000000000001",
  "auth_protocol": "derived-auth-v1",
  "auth_stored_key": "<base64url-32-bytes>",
  "auth_server_key": "<base64url-32-bytes>",
  "encrypted_account_keyset": {
    "crypto_version": "account-keyset-v1",
    "key_id": "user-key-v1",
    "nonce": "<base64url-12-bytes>",
    "ciphertext": "<base64url-bytes>"
  },
  "initial_vault": {
    "vault_id": "00000000-0000-4000-8000-000000000010",
    "encrypted_vault_key": {
      "crypto_version": "vault-key-wrap-v1",
      "key_id": "user-key-v1",
      "nonce": "<base64url-12-bytes>",
      "ciphertext": "<base64url-bytes>"
    }
  },
  "device": {
    "label": "Firefox on laptop",
    "client_type": "browser",
    "public_metadata": {
      "platform_hint": "web"
    }
  }
}
```

Response `201`:

```json
{
  "account_id": "00000000-0000-4000-8000-000000000100",
  "session": {
    "state": "mfa_enrollment_required",
    "vault_access": false,
    "idle_expires_at": "2026-06-07T00:30:00Z",
    "absolute_expires_at": "2026-06-07T12:00:00Z"
  },
  "next_step": "enroll_totp"
}
```

Response headers set `__Host-pv_session` when a session is created.

Duplicate or expired registration returns a generic failure such as `registration_unavailable`.
The implementation must rate-limit registration attempts by source and normalized handle.

Implementation status:

- `register/start` and `register/finish` are implemented as the first registration foundation slice.
- The browser MVP uses WebCrypto-native `pbkdf2-sha256-browser-v1` so registration can run without
  an unreviewed Argon2id WASM dependency. Argon2id remains the future hardening target after a
  pinned dependency and test-vector review. Pre-MVP `argon2id-browser-v1` rows are migrated to the
  PBKDF2 profile instead of being served as legacy login metadata because mixed login profiles would
  make legacy accounts distinguishable from unknown login handles.
- `register/finish` stores only encrypted account keyset and vault key-wrap ciphertext metadata.
- The initial vault `genesis_head_hash` is currently a deterministic server-side SHA-256 domain
  hash over the vault id because the request does not yet carry a client-supplied genesis hash.
- The client-supplied `initial_vault.vault_id` remains part of the contract for now; collisions are
  handled as a generic registration failure.
- The setup session is created with `mfa_enrollment_required`. CSRF issuance, TOTP enrollment,
  login finish, login-time TOTP verification, and vault item APIs are implemented.
- The `__Host-pv_session` cookie is intentionally `Secure`; browser testing should use the
  mini-PC HTTPS edge route or another HTTPS route for realistic cookie persistence.

## Login

### `POST /v1/auth/login/start`

Request:

```json
{
  "login_handle": "user@example.com",
  "auth_protocol": "derived-auth-v1",
  "client_nonce": "<base64url-32-bytes>"
}
```

Response `200`:

```json
{
  "login_challenge_id": "00000000-0000-4000-8000-000000000020",
  "auth_protocol": "derived-auth-v1",
  "kdf_profile": {
    "id": "pbkdf2-sha256-browser-v1",
    "algorithm": "PBKDF2-HMAC-SHA-256",
    "iterations": 600000,
    "hash": "SHA-256"
  },
  "account_salt": "<base64url-32-bytes>",
  "auth_verifier_profile": "pv-scram-sha-256-v1",
  "auth_verifier_salt": "<base64url-32-bytes>",
  "auth_verifier_iterations": 150000,
  "server_nonce": "<base64url-32-bytes>",
  "combined_nonce": "<base64url-64-bytes>",
  "expires_at": "2026-06-07T00:05:00Z"
}
```

`login/start` must return the same status, header shape, and JSON shape for existing and unknown
accounts. Unknown accounts use deterministic synthetic metadata.

For `derived-auth-v1`, `combined_nonce` is:

```text
base64url_no_pad(client_nonce || server_nonce)
```

where both inputs are decoded 32-byte values and `||` means byte concatenation in that order.

`login/start` must not reveal MFA enrollment status.

### `POST /v1/auth/login/finish`

Request:

```json
{
  "login_challenge_id": "00000000-0000-4000-8000-000000000020",
  "auth_protocol": "derived-auth-v1",
  "client_nonce": "<base64url-32-bytes>",
  "server_nonce": "<base64url-32-bytes>",
  "client_final_without_proof": "<base64url-bytes>",
  "client_proof": "<base64url-32-bytes>",
  "device": {
    "label": "Firefox on laptop",
    "client_type": "browser",
    "public_metadata": {
      "platform_hint": "web"
    }
  }
}
```

Response `200` when TOTP is required:

```json
{
  "result": "mfa_required",
  "mfa_challenge_id": "00000000-0000-4000-8000-000000000021",
  "available_methods": ["totp", "recovery_code"],
  "expires_at": "2026-06-07T00:05:00Z"
}
```

Response `200` when no MFA is active:

```json
{
  "result": "session_created",
  "session": {
    "state": "mfa_enrollment_required",
    "vault_access": false,
    "idle_expires_at": "2026-06-07T00:30:00Z",
    "absolute_expires_at": "2026-06-07T12:00:00Z"
  },
  "next_step": "enroll_totp"
}
```

The no-MFA session is a setup/recovery state only. It must not access vault item APIs until TOTP is
confirmed and the session is upgraded.

Wrong proof, expired challenge, unknown account, or unsupported protocol returns a generic auth
failure. The server must not log `client_proof`.

The login challenge is one-shot: success, proof failure, or metadata mismatch consumes it.

## MFA

### `POST /v1/auth/mfa/totp/verify`

Request:

```json
{
  "mfa_challenge_id": "00000000-0000-4000-8000-000000000021",
  "code": "123456"
}
```

Response `200`:

```json
{
  "result": "session_created",
  "session": {
    "state": "mfa_verified",
    "vault_access": true,
    "idle_expires_at": "2026-06-07T00:30:00Z",
    "absolute_expires_at": "2026-06-07T12:00:00Z"
  }
}
```

Response headers set `__Host-pv_session`.

TOTP verification policy:

- SHA1, 6 digits, 30 second period;
- accept current step and one adjacent step on either side;
- reject any step less than or equal to `last_accepted_step`;
- generic failure for invalid, malformed, replayed, expired, or rate-limited attempts;
- consume the MFA challenge after the fifth failed attempt.

### `POST /v1/auth/mfa/recovery-code/verify`

Request:

```json
{
  "mfa_challenge_id": "00000000-0000-4000-8000-000000000021",
  "recovery_code": "pvrc-xxxx-xxxx-xxxx-xxxx"
}
```

Response `200`:

```json
{
  "result": "session_created",
  "session": {
    "state": "mfa_recovery",
    "vault_access": false,
    "idle_expires_at": "2026-06-07T00:30:00Z",
    "absolute_expires_at": "2026-06-07T12:00:00Z"
  },
  "next_step": "reenroll_totp"
}
```

Recovery-code verification consumes the code permanently and does not reveal or change vault
decryption material. It can only be used after the primary login proof succeeds and a pre-MFA
challenge is issued. It is not a password reset, account-secret recovery, or vault recovery path.

Verification policy:

- unknown, malformed, reused, rate-limited, or expired-challenge recovery-code attempts return the
  same generic MFA failure;
- failed recovery-code attempts count against the same pre-MFA challenge attempt limit as TOTP;
- a valid unused recovery code is marked used before the recovery session is created;
- the resulting `mfa_recovery` session cannot access vault APIs;
- the user must enroll and confirm a new TOTP factor to return to `mfa_verified`.

Implementation status: implemented for the MVP browser/API preview.

## TOTP Enrollment And Recovery Codes

These endpoints require an active session and CSRF protection.

### `POST /v1/mfa/totp/enroll/start`

Request:

```json
{}
```

Response `200`:

```json
{
  "factor_id": "00000000-0000-4000-8000-000000000030",
  "status": "pending",
  "totp_profile": {
    "algorithm": "SHA1",
    "digits": 6,
    "period": 30
  },
  "otpauth_uri": "otpauth://totp/Password%20Vault:user%40example.com?secret=<redacted-secret>&issuer=Password%20Vault&algorithm=SHA1&digits=6&period=30",
  "manual_secret": "<base32-secret-shown-once>",
  "expires_at": "2026-06-07T00:10:00Z"
}
```

`otpauth_uri` and `manual_secret` are shown once and must never be logged.

Implementation status:

- Implemented.
- Requires a valid setup/recovery session, same-origin unsafe request checks, and the current
  `X-PV-CSRF` token.
- Generates a server-owned 20-byte TOTP seed and stores it only as XChaCha20Poly1305 ciphertext
  under the runtime `PV_TOTP_SEED_KEY_B64` key.
- Replaces any previous factor row for the account before inserting the new pending factor.
- Returns `otpauth_uri` and `manual_secret` once for QR/manual authenticator enrollment.
- The response `expires_at` is the current effective session idle expiry.

### `POST /v1/mfa/totp/enroll/confirm`

Request:

```json
{
  "factor_id": "00000000-0000-4000-8000-000000000030",
  "code": "123456"
}
```

Response `200`:

```json
{
  "factor_id": "00000000-0000-4000-8000-000000000030",
  "status": "active",
  "session": {
    "state": "mfa_verified",
    "vault_access": true,
    "idle_expires_at": "2026-06-07T00:30:00Z",
    "absolute_expires_at": "2026-06-07T12:00:00Z"
  },
  "recovery_codes": [
    "pvrc-aaaa-bbbb-cccc-dddd",
    "pvrc-eeee-ffff-gggg-hhhh"
  ]
}
```

The real response returns 10 recovery codes. They are shown once.

Enrollment confirmation rotates or upgrades the current session into `mfa_verified`.

Implementation status:

- Implemented.
- Requires a valid setup/recovery session, same-origin unsafe request checks, and the current
  `X-PV-CSRF` token.
- Decrypts the pending TOTP seed with XChaCha20Poly1305 and verifies the submitted code with the
  RFC 6238 adjacent-step window.
- A failed or malformed code consumes the pending factor; the user must start enrollment again.
  This avoids a reusable pending seed/factor without adding schema state for enrollment attempts.
- Marks the factor active, stores `last_accepted_step`, generates 10 one-time recovery codes, and
  stores only salted SHA-256 recovery-code hashes.
- Rotates the session token, clears the session CSRF verifier, upgrades the session to
  `mfa_verified`, and returns a new `__Host-pv_session` cookie.

### `POST /v1/mfa/recovery-codes/rotate`

Request:

```json
{}
```

Response `200`:

```json
{
  "recovery_codes": [
    "pvrc-aaaa-bbbb-cccc-dddd",
    "pvrc-eeee-ffff-gggg-hhhh"
  ]
}
```

Rotation invalidates old unused recovery codes.

### `POST /v1/mfa/totp/disable`

Request:

```json
{
  "code": "123456"
}
```

Response `200`:

```json
{
  "status": "disabled"
}
```

Disabling TOTP requires a fresh authenticated session and either current TOTP verification or a
future stronger step-up policy. The exact UX can be implemented after enroll/confirm.

## Sessions And CSRF

### Cookie

The server session is represented only by a host-prefixed cookie:

```text
__Host-pv_session=<opaque-token>
Secure
HttpOnly
SameSite=Strict
Path=/
Domain not set
```

The database stores only a hash of the token.

### `GET /v1/csrf`

Response `200`:

```json
{
  "csrf_token": "<base64url-32-bytes>",
  "expires_at": "2026-06-07T00:30:00Z"
}
```

Authenticated state-changing requests send:

```text
X-PV-CSRF: <token>
```

Implementation status:

- Implemented.
- Each successful `GET /v1/csrf` rotates the session CSRF token by replacing
  `sessions.csrf_token_hash`; the raw token is returned once and is not stored.
- The current contract is single-slot: fetching a new CSRF token invalidates the previous token for
  that session.
- The returned `expires_at` is the current effective idle expiry.
- CSRF validation is enforced for `POST /v1/auth/logout` and implemented vault item write routes.
  Future authenticated unsafe routes must use the same session, CSRF, Origin, and Fetch Metadata
  checks.

### `GET /v1/session`

Response `200`:

```json
{
  "authenticated": true,
  "account_id": "00000000-0000-4000-8000-000000000100",
  "device_id": "00000000-0000-4000-8000-000000000200",
  "session_state": "mfa_verified",
  "vault_access": true,
  "idle_expires_at": "2026-06-07T00:30:00Z",
  "absolute_expires_at": "2026-06-07T12:00:00Z"
}
```

Response `200` without a session:

```json
{
  "authenticated": false
}
```

Implementation status:

- Implemented.
- A valid session requires a valid `__Host-pv_session` cookie, matching session token hash, no
  revocation, non-revoked device, `idle_expires_at > now()`, and `absolute_expires_at > now()`.
- Successful authenticated access refreshes `last_seen_at`, `idle_expires_at`, and compatibility
  `expires_at` to at most the absolute expiry.
- Missing, malformed, duplicate, expired, or stale cookies return `authenticated: false`; stale
  cookies are cleared.

Session states:

| State | Vault access | Meaning |
| --- | --- | --- |
| `mfa_enrollment_required` | No | New or recovered account must enroll TOTP. |
| `mfa_recovery` | No | Recovery code was used; account must re-enroll TOTP. |
| `mfa_verified` | Yes | Session passed TOTP and can use account and vault APIs. |

### `POST /v1/auth/logout`

Request:

```json
{}
```

Response `204`.

The server deletes the current session and clears `__Host-pv_session`.

Implementation status:

- Implemented.
- No valid session is idempotent: the response is `204` and clears the cookie.
- A valid session requires the current `X-PV-CSRF` token before the session row is deleted.
- Cross-site Fetch Metadata or mismatched `Origin` is rejected with `csrf_required`.

## Devices

### `GET /v1/devices`

Response `200`:

```json
{
  "devices": [
    {
      "device_id": "00000000-0000-4000-8000-000000000200",
      "label": "Firefox on laptop",
      "client_type": "browser",
      "created_at": "2026-06-07T00:00:00Z",
      "last_seen_at": "2026-06-07T00:15:00Z",
      "revoked_at": null
    }
  ]
}
```

### `PATCH /v1/devices/{device_id}`

Request:

```json
{
  "label": "Laptop browser"
}
```

Response `200` returns the updated device object.

### `DELETE /v1/devices/{device_id}`

Response `204`.

Device deletion is soft revocation for API access. It cannot erase already copied local data from a
compromised client.

### `GET /v1/sessions`

Response `200`:

```json
{
  "sessions": [
    {
      "session_id": "00000000-0000-4000-8000-000000000300",
      "device_id": "00000000-0000-4000-8000-000000000200",
      "created_at": "2026-06-07T00:00:00Z",
      "last_seen_at": "2026-06-07T00:15:00Z",
      "idle_expires_at": "2026-06-07T00:30:00Z",
      "absolute_expires_at": "2026-06-07T12:00:00Z"
    }
  ]
}
```

### `DELETE /v1/sessions/{session_id}`

Response `204`.

## Vaults And Encrypted Item Revisions

The API stores ciphertext and sync metadata only. Plaintext item fields stay inside encrypted
client-side envelopes.

Vault endpoints require `mfa_verified` session state.

The backend persists row-level `key_id` and `crypto_version` from the encrypted envelope so sync and
schema constraints can validate versioned ciphertext metadata without decrypting payloads.

### `GET /v1/vaults`

Response `200`:

```json
{
  "vaults": [
    {
      "vault_id": "00000000-0000-4000-8000-000000000010",
      "head_seq": 0,
      "head_hash": "<base64url-32-bytes>",
      "genesis_head_hash": "<base64url-32-bytes>",
      "encrypted_vault_key": {
        "crypto_version": "vault-key-wrap-v1",
        "key_id": "user-key-v1",
        "nonce": "<base64url-12-bytes>",
        "ciphertext": "<base64url-bytes>"
      },
      "created_at": "2026-06-07T00:00:00Z",
      "updated_at": "2026-06-07T00:00:00Z"
    }
  ]
}
```

Implementation note: encrypted vault display metadata such as `name_ciphertext` is planned but is
not represented in the current schema. The current implementation omits `name_ciphertext` until an
encrypted vault metadata migration is designed.

`genesis_head_hash` is returned so a returning browser or future device can sync from
`from_head_seq=0` even when the current vault head has already advanced. It is sync metadata, not
vault plaintext.

### `GET /v1/vaults/{vault_id}/sync`

Query parameters:

```text
from_head_seq=0
from_head_hash=<base64url-32-bytes>
```

Response `200`:

```json
{
  "from_head": {
    "seq": 0,
    "hash": "<base64url-32-bytes>"
  },
  "to_head": {
    "seq": 2,
    "hash": "<base64url-32-bytes>"
  },
  "has_more": false,
  "changes": [
    {
      "item_id": "00000000-0000-4000-8000-000000000400",
      "revision_id": "00000000-0000-4000-8000-000000000401",
      "operation": "create",
      "revision_seq": 1,
      "head_seq": 1,
      "previous_head_hash": "<base64url-32-bytes>",
      "head_hash": "<base64url-32-bytes>",
      "base_revision_seq": 0,
      "base_head_seq": 0,
      "base_head_hash": "<base64url-32-bytes>",
      "change_mac": "<base64url-32-bytes>",
      "envelope_hash": "<base64url-32-bytes>",
      "encrypted_item_envelope": {
        "crypto_version": "item-envelope-v1",
        "key_id": "vault-key-v1",
        "aead": "AES-256-GCM",
        "nonce": "<base64url-12-bytes>",
        "ciphertext": "<base64url-bytes>"
      }
    }
  ]
}
```

If the supplied cursor does not match the visible vault head history, return `409 vault_conflict`
with the current visible head.

The implementation caps each sync response at 500 changes. If more changes remain, `has_more` is
`true` and `to_head` is the last returned change head; the client should call sync again using that
`to_head` as the next cursor.

An unlocked client must verify every returned change against its local keyed head-hash chain before
advancing its local checkpoint. On the final page, including an empty final page, the response
`to_head` must match the locally verified head. The client must not adopt a server-supplied
`to_head` that was not proven by the returned change chain.

### `POST /v1/vaults/{vault_id}/items`

Request:

```json
{
  "item_id": "00000000-0000-4000-8000-000000000400",
  "revision_id": "00000000-0000-4000-8000-000000000401",
  "base_head_seq": 0,
  "base_head_hash": "<base64url-32-bytes>",
  "new_head_hash": "<base64url-32-bytes>",
  "change_mac": "<base64url-32-bytes>",
  "envelope_hash": "<base64url-32-bytes>",
  "encrypted_item_envelope": {
    "crypto_version": "item-envelope-v1",
    "key_id": "vault-key-v1",
    "aead": "AES-256-GCM",
    "nonce": "<base64url-12-bytes>",
    "ciphertext": "<base64url-bytes>"
  }
}
```

Response `201`:

```json
{
  "item_id": "00000000-0000-4000-8000-000000000400",
  "revision_id": "00000000-0000-4000-8000-000000000401",
  "revision_seq": 1,
  "head_seq": 1,
  "head_hash": "<base64url-32-bytes>"
}
```

### `POST /v1/vaults/{vault_id}/items/{item_id}/revisions`

Request:

```json
{
  "revision_id": "00000000-0000-4000-8000-000000000402",
  "operation": "update",
  "base_revision_seq": 1,
  "base_head_seq": 1,
  "base_head_hash": "<base64url-32-bytes>",
  "new_head_hash": "<base64url-32-bytes>",
  "change_mac": "<base64url-32-bytes>",
  "envelope_hash": "<base64url-32-bytes>",
  "encrypted_item_envelope": {
    "crypto_version": "item-envelope-v1",
    "key_id": "vault-key-v1",
    "aead": "AES-256-GCM",
    "nonce": "<base64url-12-bytes>",
    "ciphertext": "<base64url-bytes>"
  }
}
```

`operation` is `update` or `delete`. Deletion is an authenticated revision. There is no bare item
`DELETE` endpoint in the MVP.

Response `201`:

```json
{
  "item_id": "00000000-0000-4000-8000-000000000400",
  "revision_id": "00000000-0000-4000-8000-000000000402",
  "revision_seq": 2,
  "head_seq": 2,
  "head_hash": "<base64url-32-bytes>"
}
```

Stale writes return `409 vault_conflict` with the current visible vault head. The unlocked client is
responsible for verifying `change_mac`, `envelope_hash`, and hash-chain continuity before trusting
or decrypting sync responses.

Implementation status:

- Merged and deployed in the current GitOps preview.
- Vault endpoints require an `mfa_verified` session.
- State-changing vault requests require the same JSON, session, CSRF, Fetch Metadata, and Origin
  protections as logout.
- `item_id` and `revision_id` are client-generated so the encrypted payload can bind them into AEAD
  associated data and `change_mac`.
- The backend stores encrypted item envelopes and sync metadata only. It does not decrypt or inspect
  item plaintext.
- Cross-account vault or item access returns `404 not_found`.
- Stale write bases and mismatched sync cursors return `409 vault_conflict` with the current visible
  vault head after membership is proven.

## Audit Events

### `GET /v1/audit-events`

Status: planned. The database audit log exists for implemented auth/MFA/session flows, but this
read endpoint is not implemented yet.

Response `200`:

```json
{
  "events": [
    {
      "event_id": "00000000-0000-4000-8000-000000000500",
      "event_type": "mfa_totp_login_verified",
      "created_at": "2026-06-07T00:00:00Z",
      "actor_device_id": "00000000-0000-4000-8000-000000000200",
      "metadata": {
        "client_type": "browser"
      }
    }
  ]
}
```

Audit events must not include secret values, plaintext vault item contents, TOTP seeds, recovery
codes, session tokens, private infrastructure details, or request bodies.

## Required API Tests

- `login/start` has constant response shape for existing and unknown accounts.
- Auth/MFA/session responses include `Cache-Control: no-store`.
- Routes with bodies reject non-JSON and browser-simple form content types.
- Auth router body-limit behavior is tested: valid JSON bodies below the current 128 KiB MVP cap
  can reach the route handler, while bodies above the cap are rejected with the current generic
  `bad_request` error before handler execution.
- `login/start` does not expose MFA state.
- Registration duplicate handling returns generic failure and is rate-limited.
- Auth verifier registration never stores raw `client_auth_secret`.
- Login proof verifies with stored verifier material and fails generically for wrong proof.
- TOTP enrollment returns seed material once and stores only ciphertext.
- TOTP replay fails for the same accepted time step.
- Recovery code can be used once and does not expose vault decryption material.
- Pre-MFA challenge cannot call authenticated endpoints.
- Setup/recovery sessions cannot call vault item APIs until TOTP is confirmed.
- `__Host-pv_session` cookie flags are enforced.
- Authenticated mutation without `X-PV-CSRF` fails.
- Cross-site unsafe request with Fetch Metadata headers fails.
- Vault item APIs reject plaintext item fields.
- Stale vault write returns `409 vault_conflict`.
- Sync cursor mismatch returns `409 vault_conflict`.
- Audit events and logs do not include secrets, OTPs, recovery codes, or plaintext item fields.

## Sources

- https://www.rfc-editor.org/rfc/rfc5802.html
- https://www.rfc-editor.org/rfc/rfc7677.html
- https://www.rfc-editor.org/rfc/rfc6238.html
- https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
- https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
- https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie
- https://github.com/google/google-authenticator/wiki/Key-Uri-Format
