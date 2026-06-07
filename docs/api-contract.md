# API Contract Draft

Status: bootstrap draft. This is not an implementation spec.

## Purpose

`password-vault` is API-first. The browser web app is the first client of the product API, not a
special backend-only flow. Future browser extension, mobile, desktop, CLI, and integration clients
should reuse the same versioned contracts.

API-first does not mean public unauthenticated access. It means stable, documented contracts for
authorized clients.

## Contract Strength

Before product code for security-sensitive endpoints is marked review-ready, the endpoint must have:

- documented request and response shapes;
- generic error behavior;
- rate-limit expectations;
- auth/session requirements;
- plaintext metadata versus encrypted payload boundary;
- tests or test vectors appropriate to the endpoint.

Human-readable Markdown is acceptable for early design. A machine-readable OpenAPI contract, or an
equivalent typed contract used by both backend and frontend, is required before implementation PRs for
the API surface are marked review-ready.

## Versioning

Initial namespace:

```text
/v1
```

Breaking API changes require a versioning or migration decision before implementation.

## Canonical Initial `/v1` Surface

### System

- `GET /v1/health`
- `GET /v1/ready`

Readiness must be safe for Kubernetes and must not expose private infrastructure details.

### Registration And Login

- `POST /v1/auth/register/start`
- `POST /v1/auth/register/finish`
- `POST /v1/auth/login/start`
- `POST /v1/auth/login/finish`
- `POST /v1/auth/mfa/totp/verify`
- `POST /v1/auth/logout`
- `GET /v1/session`
- `GET /v1/csrf`

The MVP auth protocol is `derived-auth-v1`. OPAQUE remains a future protocol:

```text
auth_protocol = "derived-auth-v1" | "opaque-rfc9807-v1"
```

Auth start endpoints must use protocol-neutral names so the future OPAQUE migration can reuse the
same public API shape.

`login/start` must use constant-shape responses and generic errors so it does not become an
account-enumeration endpoint. Unknown accounts use deterministic synthetic KDF/auth metadata.

`login/start` must not return pre-authenticated MFA enrollment status. The server reveals whether
TOTP is required only after `login/finish` succeeds.

Registration endpoints must also avoid trivial account enumeration. Duplicate login-handle behavior
must be generic until the implementation spec defines an accepted mitigation.

Sensitive boundary:

- raw master password: never sent to backend;
- account secret key: never sent to backend;
- account unlock key: never sent to backend;
- unwrapped vault key: never sent to backend;
- `client_auth_secret`: password-equivalent; allowed only as the documented `derived-auth-v1`
  challenge-bound proof input and never stored or sent raw;
- TOTP code: sent only to MFA verification endpoint and never logged.

### TOTP Enrollment And Recovery Codes

- `POST /v1/mfa/totp/enroll/start`
- `POST /v1/mfa/totp/enroll/confirm`
- `POST /v1/mfa/totp/disable`
- `POST /v1/mfa/recovery-codes/rotate`
- `POST /v1/auth/mfa/recovery-code/verify`

Recovery codes recover login-factor access only. They must not become a vault-decryption recovery
path.

TOTP is login MFA only. It is not part of vault item encryption.

### Devices And Sessions

- `GET /v1/devices`
- `PATCH /v1/devices/{device_id}`
- `DELETE /v1/devices/{device_id}`
- `GET /v1/sessions`
- `DELETE /v1/sessions/{session_id}`

The MVP device model may be a soft audit/revocation model. Strong cryptographic enrollment can be a
later ADR.

### Vaults And Item Revisions

- `GET /v1/vaults`
- `GET /v1/vaults/{vault_id}/sync`
- `POST /v1/vaults/{vault_id}/items`
- `POST /v1/vaults/{vault_id}/items/{item_id}/revisions`

Item payloads are encrypted client-side. The API stores ciphertext and allowed sync metadata only.

Write requests for item create, revision create, and delete must include:

- `base_head_seq`
- `base_head_hash`
- `new_head_hash`
- `change_mac`

Revision-create and deletion requests must also include `base_revision_seq`.

Deletion is represented as a signed deletion revision through
`POST /v1/vaults/{vault_id}/items/{item_id}/revisions` with `operation=delete`. The MVP should not
use a bare `DELETE` endpoint because the client must authenticate the deletion change.

Sync responses must include:

- `from_head`
- `to_head`
- ordered encrypted changes with each change's `head_seq`, `previous_head_hash`, `head_hash`, and
  `change_mac`
- all fields required to recompute `change_mac`, including operation, item/revision identifiers,
  base revision sequence, base head, key ID, crypto version, and envelope hash

The backend checks authorization and optimistic concurrency. The unlocked client verifies the
client-keyed change MACs and state hash chain before trusting or decrypting returned item envelopes.

Stale writes return `409 Conflict` with a generic conflict code and the current visible vault head.
Sync requests include both sequence and hash so a stale or forked cursor can return `409 Conflict`.
Client-side rollback/fork detection must be represented as a local client error state; it is not
resolved by the backend.

### Audit Events

- `GET /v1/audit-events`

Audit events must not include secret values, plaintext vault item contents, TOTP seeds, recovery
codes, or private infrastructure details.

## Open Decisions

- Exact registration and login message shapes.
- Exact derived-auth-v1 challenge-bound proof construction and test vectors.
- Exact duplicate-registration non-enumeration behavior.
- Exact account secret key UX and new-device flow.
- Exact TOTP replay and rate-limit behavior.
- Exact encrypted item payload format.
- Exact optimistic concurrency and conflict response shape.
- Exact canonical encoding for envelope hashes, `change_mac`, and head-hash inputs.
- Whether OpenAPI or another typed contract format is the first machine-readable source of truth.
