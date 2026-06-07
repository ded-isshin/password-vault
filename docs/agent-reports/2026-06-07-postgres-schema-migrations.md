# Agent Report: PostgreSQL Schema And Migrations

Status: completed in PR. Related issue: #15.

## Goal

Add the first deterministic PostgreSQL schema migration and CI validation path for the MVP backend.

## Active Context

- Active repository: `password-vault`
- Out of scope: `infrastructure-home`, Kubernetes mutation, Argo CD sync, runtime secrets
- Risk level: high for product security model, low for host/runtime impact

## Work Completed

- Added root SQLx migration directory.
- Added initial schema for accounts, devices, auth challenges, TOTP factors, recovery codes,
  sessions, vaults, vault items, immutable item revisions, and audit events.
- Added SQLx migration helper code in the Rust API crate.
- Added readiness DB ping support when the API is built with a database pool.
- Changed normal runtime DB pool creation to lazy connection so temporary DB outages make `/readyz`
  fail instead of forcing an API process crash-loop. Startup migrations still require an eager
  connection.
- Added CI PostgreSQL service-container migration test.
- Added locked Cargo commands in CI to preserve the Rust 1.85-compatible lockfile.
- Added integration-test checks for duplicate login handles, cross-account session/device links,
  invalid revision operations, cross-vault revision links, and plaintext item-column absence.

## Design Notes

- UUIDs are supplied by the app/client; the migration does not require PostgreSQL extensions.
- Vault item payloads are stored as encrypted envelopes and sync metadata only.
- Item revisions are immutable rows; deletion is represented by an authenticated `operation='delete'`
  revision.
- The database enforces local row constraints with primary keys, foreign keys, unique constraints,
  and check constraints. Cross-row protocol checks remain application-transaction logic.
- `PV_RUN_MIGRATIONS_ON_STARTUP` exists for local/bootstrap use but defaults to false. Kubernetes
  deployment should prefer an explicit migration job or approved rollout pattern.

## Commands Run

- `gh issue view 15 -R ded-isshin/password-vault --json ...`
- `docker run --rm rust:1.85-bookworm ... cargo search sqlx --limit 5`
- `docker run --rm rust:1.85-bookworm ... cargo info sqlx`
- `docker run --rm rust:1.85-bookworm ... cargo info sqlx@0.8.6`
- `docker run --rm ... cargo generate-lockfile`
- `docker run --rm ... cargo update -p home@0.5.12 --precise 0.5.11`
- `docker run --rm ... cargo update -p idna_adapter@1.2.2 --precise 1.2.1`
- `docker run --rm ... cargo update -p icu_*@2.2.0 --precise 2.1.x`
- `docker run --rm ... cargo check --workspace`
- `docker run --rm ... cargo fetch --locked`
- `docker run --rm ... cargo fmt --all -- --check`
- `docker run --rm ... cargo clippy --locked --workspace --all-targets -- -D warnings`
- `docker run --rm ... cargo test --locked --workspace`
- Disposable `postgres:17-bookworm` container plus Rust container:
  `cargo fetch --locked && cargo test --locked --workspace --test migrations -- --nocapture`
- `claude -p --permission-mode plan --effort high ...`

## Official Documentation Consulted

- SQLx migration macro and migration source docs.
- PostgreSQL 17 constraints docs.
- GitHub Actions service containers docs.
- SQLx `PgPoolOptions::connect_lazy` docs.

## Validation

Tested:

- `cargo check --workspace`: passed in `rust:1.85-bookworm`.
- `cargo fetch --locked`: passed in `rust:1.85-bookworm`.
- `cargo fmt --all -- --check`: passed in `rust:1.85-bookworm`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed in
  `rust:1.85-bookworm`.
- `cargo test --locked --workspace`: passed in `rust:1.85-bookworm`; 5 API tests passed, including
  unreachable configured DB readiness.
- `cargo test --locked --workspace --test migrations -- --nocapture`: passed against disposable
  `postgres:17-bookworm`.

Verified:

- Temporary Docker test container and network were removed.

Needs verification:

- GitHub Actions checks after PR creation.
- Runtime migration strategy for Kubernetes deployment.
- Backup and restore drill before real user data.

Not tested:

- Auth verifier implementation.
- TOTP seed encryption implementation.
- Vault item create/update/delete runtime transactions.
- Kubernetes deployment or Argo CD sync.

## Risks

- `sqlx` latest `0.9.0` requires Rust `1.94.0`, while this repo is pinned to Rust `1.85`.
  The migration implementation therefore uses `sqlx 0.8.6`.
- Some transitive crates selected by a fresh lockfile required Rust newer than 1.85. The lockfile
  pins compatible versions for `home`, `idna_adapter`, and ICU-related crates.
- Application code must still enforce authorization and transactional optimistic concurrency.
- Exact auth verifier, TOTP seed encryption envelope, and encrypted item envelope formats remain
  follow-up implementation work.

## Subagent Review

Purpose: independent read-only schema and CI review.

Summary of output:

- Required composite tenant-boundary constraints for devices, sessions, vaults, items, and revisions.
- Required append-only item revisions with operation, revision/head sequences, hashes, `change_mac`,
  crypto metadata, and encrypted envelope.
- Required `vaults.head_seq/head_hash` plus unique `(vault_id, head_seq)`.
- Required auth/MFA/session tables without raw password, raw client auth secret, account secret key,
  raw TOTP seed, or plaintext item fields.
- Recommended SQLx migration directory, `build.rs` migration-change tracking, PostgreSQL service
  container CI, and `/readyz` DB ping.

Accepted suggestions:

- Composite FK/unique constraints.
- Append-only revision table and delete-as-revision modeling.
- Unique vault head sequence.
- Migration CI with PostgreSQL service.
- Readiness DB ping.
- Negative migration tests for cross-scope links and plaintext item columns.

Rejected suggestions:

- None for this PR scope.

## Claude Code Usage

Purpose: independent architecture/security/code review for the #15 diff.

Prompt/task given: read-only review of the current uncommitted diff, focused on tenant/account
constraints, encrypted payload boundary, append-only revisions, SQLx/CI, readiness behavior, Rust
1.85 compatibility, and public repo secret safety.

Summary of output:

- Claude Code found no merge-blocking issues and called the PR mergeable as-is.
- Medium follow-up M1: avoid eager DB connection crash-loop on boot-time DB outage.
- Medium follow-up M2: add `--locked` to CI Cargo commands so lockfile pins cannot drift.

Accepted suggestions:

- Use lazy DB pool creation for normal runtime startup while keeping startup migrations eager.
- Add `cargo fetch --locked`, `cargo clippy --locked`, and `cargo test --locked` to CI.

Rejected suggestions:

- None.

## Approval Needed

No host, cluster, or infrastructure mutation was performed. Kubernetes/GitOps changes still need a
separate safe rollout step.
