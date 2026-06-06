# Research Note: Initial Stack Analysis

Status: bootstrap draft.

## Summary

Recommended MVP stack:

- Rust backend with Axum, Tokio, and SQLx.
- TypeScript frontend with React, Vite, and WebCrypto.
- PostgreSQL as primary product database.
- CloudNativePG for production-like Kubernetes PostgreSQL.
- GitHub Actions for CI.
- GHCR for images.
- Helm and Argo CD for future GitOps deployment.
- Existing infrastructure runtime-secret path first; Vault/OpenBao evaluated later as a platform
  layer.

## Why Rust

Rust is recommended because `password-vault` is security-sensitive and benefits from strong memory
safety and type discipline. The MVP must stay narrow so Rust complexity does not delay the product
unnecessarily.

Go remains a viable fallback if implementation speed becomes the dominant constraint.

## Why PostgreSQL

PostgreSQL fits users, sessions, MFA records, vault memberships, encrypted item revisions, and audit
events. ClickHouse is not the right primary store for this OLTP workload.

## Why Not Vault As Core Storage

Vault/OpenBao is useful for platform secrets, but not as the core user-vault storage or decryption
system. If the backend or Vault can decrypt user vault item data, the product is no longer
zero-knowledge.

## Current Stack Blockers

- Login/key-derivation protocol is not selected.
- WebCrypto vs Argon2id/WASM is not selected.
- TOTP seed custody is not selected.
- PostgreSQL synchronous vs asynchronous replication is not selected.
- Backup target is not selected.
- Browser KDF implementation is not selected.
- GitHub Project creation is blocked by missing full `project` scope.

## Sources

- https://doc.rust-lang.org/book/
- https://docs.rs/axum/latest/axum/
- https://docs.rs/sqlx/latest/sqlx/
- https://www.w3.org/TR/webcrypto/
- https://www.rfc-editor.org/info/rfc6238/
- https://cloudnative-pg.io/docs/1.29/architecture/
- https://cloudnative-pg.io/docs/1.29/replication/
- https://cloudnative-pg.io/docs/1.29/backup/
- https://cloudnative-pg.io/docs/1.29/recovery/
- https://developer.hashicorp.com/vault/docs/about-vault/how-vault-works
- https://developer.hashicorp.com/vault/docs/secrets/transit
- https://docs.github.com/en/issues/planning-and-tracking-with-projects
