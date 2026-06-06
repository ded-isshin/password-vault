# ADR 0002: Initial Application Stack Direction

Status: proposed.

## Context

`password-vault` is a security-sensitive product. The backend must handle authentication, sessions,
MFA verification, authorization, encrypted sync metadata, audit events, PostgreSQL access, and
Kubernetes health behavior. The browser client must handle vault unlock, encryption, decryption, and
local decrypted state.

The backend must not decrypt user vault item payloads.

## Options Considered

### Rust

- Axum for HTTP routing.
- Tokio for async runtime.
- SQLx for PostgreSQL access.
- RustCrypto crates for server-owned cryptographic operations where needed.

Strengths:

- strong memory-safety model;
- strong type discipline;
- good fit for security-sensitive services;
- explicit SQL possible with SQLx;
- strict CI can catch many problems early.

Weaknesses:

- slower MVP development;
- more dependency review burden;
- more implementation friction than Go.

### Go

- `net/http` or small router.
- `pgx` for PostgreSQL.
- Go standard crypto and `golang.org/x/crypto`.

Strengths:

- faster delivery;
- strong operational simplicity;
- mature cloud-native ecosystem;
- excellent official security tooling such as `govulncheck`.

Weaknesses:

- less type-level discipline around secret material lifetime;
- garbage collection makes memory-zeroization expectations harder to reason about.

## Decision

Recommend this MVP stack:

- Backend: Rust, Axum, Tokio, SQLx.
- Frontend: TypeScript, React, Vite.
- Browser crypto: WebCrypto plus reviewed WASM only where required.
- Database: PostgreSQL.
- Production-like Kubernetes database: CloudNativePG.

Go remains the fallback if Rust makes MVP delivery unacceptably slow.

## Rationale

The product is a password manager, not a generic CRUD service. Rust's safety properties and type
discipline are worth the added complexity if the MVP remains narrow and the team avoids inventing
cryptography.

TypeScript is the practical choice for the browser client because the first product surface is a web
app and the client-side crypto boundary needs strong DTO and state typing.

PostgreSQL fits transactional product state: users, sessions, MFA records, vault membership,
encrypted item revisions, and audit events.

## Implementation Direction

- `axum` for routing.
- `tokio` runtime.
- `sqlx` with PostgreSQL.
- React + Vite for the web MVP.
- opaque server-side sessions in PostgreSQL.
- secure HttpOnly SameSite cookies.
- TOTP implementation reviewed against RFC 6238 vectors.
- no JWT browser sessions in the MVP.
- no `unsafe` in product code unless separately justified.

## Consequences

- CI must include `cargo fmt`, `clippy`, `cargo test`, dependency audit, and locked builds.
- Crypto/auth crates require explicit review before adoption.
- Product implementation should wait for auth and crypto ADRs.
- Frontend dependencies that touch the crypto boundary require supply-chain review.

## Sources

- https://docs.rs/axum/latest/axum/
- https://docs.rs/sqlx/latest/sqlx/
- https://docs.rs/argon2/latest/argon2/
- https://react.dev/learn/typescript
- https://vite.dev/guide/
- https://www.rfc-editor.org/info/rfc6238/
