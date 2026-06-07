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

### Registration And Login Metadata

- `POST /v1/auth/registration`
- `POST /v1/auth/login-metadata`
- `POST /v1/auth/login`
- `POST /v1/auth/logout`
- `GET /v1/auth/session`

`login-metadata` must use constant-shape responses and generic errors so it does not become an
account-enumeration endpoint.

### MFA And Recovery Codes

- `POST /v1/auth/totp/enrollment`
- `POST /v1/auth/totp/verification`
- `DELETE /v1/auth/totp`
- `POST /v1/auth/recovery-codes`
- `POST /v1/auth/recovery-codes/verification`

Recovery codes recover login-factor access only. They must not become a vault-decryption recovery
path.

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
- `DELETE /v1/vaults/{vault_id}/items/{item_id}`

Item payloads are encrypted client-side. The API stores ciphertext and allowed sync metadata only.

### Audit Events

- `GET /v1/audit-events`

Audit events must not include secret values, plaintext vault item contents, TOTP seeds, recovery
codes, or private infrastructure details.

## Open Decisions

- Exact registration and login message shapes.
- Exact account secret key UX and new-device flow.
- Exact TOTP replay and rate-limit behavior.
- Exact encrypted item payload format.
- Exact optimistic concurrency and conflict response shape.
- Whether OpenAPI or another typed contract format is the first machine-readable source of truth.

