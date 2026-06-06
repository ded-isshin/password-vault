# Architecture

Status: bootstrap draft.

## Current State

No product code exists yet. This document records the intended architecture direction.

## High-Level Architecture

```text
Browser client
  - registration/login UI
  - TOTP enrollment UI
  - WebCrypto-based vault encryption/decryption
  - local unlock and search for decrypted data

API service
  - account/session endpoints
  - TOTP verification
  - encrypted vault sync endpoints
  - authorization checks
  - audit events without secret values

PostgreSQL
  - users
  - sessions
  - MFA metadata
  - vaults and memberships
  - encrypted item revisions
  - audit events

Kubernetes
  - stateless app replicas
  - PostgreSQL operator-managed cluster
  - GitOps deployment through infrastructure repository
```

## Stack Direction

- Backend: Rust, Axum, Tokio, SQLx.
- Frontend: TypeScript, React, Vite.
- Browser crypto: WebCrypto.
- Primary database: PostgreSQL.
- Production-like database operator: CloudNativePG.
- Container registry: GHCR.
- CI/CD: GitHub Actions and GitOps PRs.
- Deployment controller: Argo CD.

## Storage Model

PostgreSQL is the source of truth for product state. User vault item payloads are stored as
ciphertext.

Do not use:

- ClickHouse as primary vault storage.
- KeePass/KDBX files as the primary SaaS storage model.
- Vault/OpenBao as the core user-vault database or decrypt path.

## Secret Management Direction

Vault/OpenBao may be considered later for platform runtime secrets, dynamic database credentials,
PKI, or server-owned encryption. It must not be able to decrypt user vault item data.

TOTP seed protection is a legitimate server-owned secret-management problem because the server must
verify TOTP during login. User vault item decryption is not.

## Cryptography Boundaries

The web MVP depends on browser-delivered JavaScript. A compromised web bundle can steal unlock
secrets before encryption. This is an accepted residual risk until a stronger client distribution
model exists.

WebCrypto does not automatically solve every desired cryptographic primitive. If the product uses
Argon2id for browser-side key derivation, it will require a reviewed WASM dependency and
bundle-integrity plan. Otherwise the KDF must use browser-native primitives with documented
tradeoffs.

Server session state is not the same as vault unlock state. A valid server session may authorize
sync API access, but the client still needs local unlock material to decrypt vault item payloads.

## Deployment Direction

The product should be deployed through GitOps:

1. product PR changes source/docs/CI/chart
2. CI builds and validates
3. image is published to GHCR after approved merge
4. separate GitOps PR updates infrastructure repository
5. human approval
6. Argo CD sync

No direct `kubectl apply` from this repository.
