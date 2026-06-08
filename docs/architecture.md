# Architecture

Status: MVP architecture draft.

## Current State

The repository contains a deployed browser/API MVP preview with registration, TOTP enrollment,
login, browser-side vault unlock, encrypted item create/update/delete, delta sync, health,
readiness, metrics, Helm, GHCR image publishing, GitOps deployment, and a basic Grafana dashboard.
It is not approved for real user secrets yet because PostgreSQL backup/WAL/restore/failover drills,
SLO alert delivery, scheduled synthetic journey checks, and stricter network/metrics controls are
still missing.

## High-Level Architecture

```text
Browser client
  - registration/login UI
  - TOTP enrollment UI
  - WebCrypto-based vault encryption/decryption
  - local unlock and search for decrypted data
  - first supported client for the MVP

API service
  - account/session endpoints
  - TOTP verification
  - encrypted vault sync endpoints
  - authorization checks
  - audit events without secret values
  - versioned API contracts for web, extension, mobile, and future CLI clients

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
  - public routing through edge reverse proxy and Kubernetes ingress/service

Future clients
  - browser extension
  - mobile app
  - desktop app
  - same account, device, key-wrap, and sync API model
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
- Platform secrets: existing infrastructure secret path first; Vault/OpenBao platform ADR later.

## API-First Direction

`password-vault` should be designed API-first.

The browser web app is the first client, not the only client. Chrome extension, mobile apps, desktop
apps, and future CLI/integration clients should use the same versioned product API instead of
separate backend behavior.

The canonical initial API surface and contract-strength rule are documented in
[API Contract Draft](api-contract.md).

API-first does not mean public unauthenticated APIs. It means stable, documented contracts for
authorized product clients.

## Functional Documents

- [Product whitepaper](whitepaper.md)
- [Foundational decisions](foundational-decisions.md)
- [API contract draft](api-contract.md)
- [Feature map](feature-map.md)
- [Architecture diagrams](diagrams.md)
- [Data model draft](data-model.md)
- [Sync protocol draft](sync-protocol.md)
- [Auth and MFA lifecycle](auth-mfa-lifecycle.md)
- [Lock and unlock state model](lock-unlock-state.md)

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

## PostgreSQL HA Direction

The current preview deployment uses a product-owned CloudNativePG cluster with three PostgreSQL
18.4 instances distributed across the three worker nodes. The API is cut over to the CNPG
application Secret and the legacy single-instance PostgreSQL StatefulSet is retained only as a short
rollback artifact.

For real user data, the target remains quorum synchronous replication with one synchronous standby.

- `required` favors acknowledged-write durability and can pause writes if the required standby set
  is unavailable.
- `preferred` favors self-healing and write availability during degraded states, but may accept a
  temporary asynchronous window.

The initial recommendation for real vault data is `required`. Password-manager writes are
user-visible saved secrets; acknowledging a write and then losing it during failover is worse than a
temporary write pause during degraded operation. This recommendation still needs target-cluster
failure testing before production-like use.

Local-path storage is node-local. It is acceptable for PostgreSQL instances in a shared-nothing
CloudNativePG design, but it is not portable storage. If one worker fails, the database survives by
failing over to a replicated PostgreSQL instance on another worker, not by remounting the failed
worker's volume elsewhere.

Backups are mandatory before real user secrets. The deployment design should use WAL archiving and
physical base backups to object storage, plus periodic restore drills. The current preview has a
healthy CNPG cluster, but backup availability, restore drills, and failover drills are still red
gates.

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

The MVP is web-client first, not single-device. The first implementation may only ship a browser UI,
but the sync protocol, data model, and key hierarchy must allow multiple browser sessions or devices
per account from the beginning. Chrome extension, iOS, desktop, and organization clients should
reuse the same encrypted sync model later.

The recommended MVP metadata boundary is conservative: titles, URLs, usernames, passwords, notes,
custom fields, and tags are encrypted in the client payload. This removes server-side content search
from the MVP and makes search available only after vault unlock.

Explicit device enrollment, device-specific key wraps, and mobile/extension client hardening can be
deferred. The MVP must still avoid single-device assumptions by using revision-based sync and
client-side key wrapping concepts that can support later device records.

## Deployment Direction

The product should be deployed through GitOps:

1. product PR changes source/docs/CI/chart
2. CI builds and validates
3. image is published to GHCR after approved merge
4. separate GitOps PR updates infrastructure repository
5. human approval
6. Argo CD sync

No direct `kubectl apply` from this repository.

Current artifact direction:

- local release builds are avoided;
- GitHub Actions builds and publishes `ghcr.io/ded-isshin/password-vault-api`;
- Docker Hub is used only for trusted base/test images, not for product autobuilds;
- deployment values should pin immutable image digests, not mutable tags;
- the product chart owns `Deployment`, `Service`, probes, PDB, and `VMServiceScrape`;
- infrastructure owns production values, runtime secrets, Argo CD `Application`, and Grafana
  dashboard provisioning.

## More Detail

- [Technical whitepaper](whitepaper.md)
- [Foundational decisions](foundational-decisions.md)
- [Decision briefs](decision-briefs/README.md)
- [Architecture diagrams](diagrams.md)
- [ADR 0002: Backend Stack Direction](adr/0002-backend-stack-rust.md)
- [ADR 0003: Auth And Crypto Direction](adr/0003-auth-and-crypto-direction.md)
- [ADR 0004: Kubernetes Data Platform Direction](adr/0004-kubernetes-data-platform-direction.md)
- [Auth login protocol options](research/auth-login-protocol-options.md)
