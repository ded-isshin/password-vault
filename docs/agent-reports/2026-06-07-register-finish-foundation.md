# Session Report: Register Finish Foundation

Date: 2026-06-07

## Goal

Move Password Vault from auth challenge preview toward a working MVP by implementing the first
registration finish backend slice.

## Active Context

- Product repository: `products/password-vault`
- Infrastructure repository: not modified in this slice
- Risk: medium, because the change touches auth, session, and database schema

## Work Completed

- Added `POST /v1/auth/register/finish`.
- Added storage for encrypted account keyset metadata.
- Added storage for encrypted initial vault key wraps.
- Added device `client_type` and `public_metadata`.
- Added session `idle_expires_at` and `absolute_expires_at`.
- Created setup sessions in `mfa_enrollment_required` state.
- Set the `__Host-pv_session` cookie with `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`, and
  no `Domain`.
- Kept CSRF token issuance unimplemented and left `sessions.csrf_token_hash` null until
  `GET /v1/csrf` is implemented.
- Updated data model, API contract, auth protocol notes, MVP plan, and browser preview copy.

## Files Changed

- `migrations/202606070002_registration_key_material.sql`
- `crates/api/src/auth/routes.rs`
- `crates/api/src/lib.rs`
- `crates/api/tests/migrations.rs`
- `crates/api/static/index.html`
- `docs/api-contract.md`
- `docs/data-model.md`
- `docs/mvp-implementation-plan.md`
- `docs/security/auth-protocol-v1.md`

## Commands Run

```bash
KUBECONFIG=<redacted-path> kubectl get app -n argocd prod-root password-vault observability-vm-stack
KUBECONFIG=<redacted-path> kubectl get pods,svc,pvc -n password-vault
curl -fsS http://<redacted-ip>:8080/healthz
curl -fsS http://<redacted-ip>:8080/readyz
curl -fsS -X POST http://<redacted-ip>:8080/v1/auth/register/start -H 'content-type: application/json' --data '{"login_handle":"voice-smoke@example.com","auth_protocol":"derived-auth-v1"}'
docker run --rm -v "$PWD:/workspace" -w /workspace rust:1.96-bookworm bash -c 'set -euo pipefail; export PATH=/usr/local/cargo/bin:$PATH; rustc --version; cargo --version; cargo fmt --all; cargo test --locked --workspace'
docker run -d --rm --name "pv-test-postgres-<pid>" -e POSTGRES_USER=pv -e POSTGRES_PASSWORD=pv -e POSTGRES_DB=pv -p 127.0.0.1::5432 postgres:18-alpine
docker run --rm --network host -e "PV_TEST_DATABASE_URL=postgres://pv:pv@127.0.0.1:<port>/pv" -v "$PWD:/workspace" -w /workspace rust:1.96-bookworm bash -c 'set -euo pipefail; export PATH=/usr/local/cargo/bin:$PATH; cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D warnings; cargo test --locked --workspace'
```

## Claude Code Usage

Purpose: independent architecture/security review for `register/finish`.

Prompt/task given: review API contract, data model, ADR 0005, migration, and auth routes; identify
schema additions, tenant-boundary constraints, validation, tests, and deployment risks.

Summary of output:

- Require a new migration for account keysets, vault key wraps, device metadata, and session
  lifetime shape.
- Enforce composite tenant-boundary foreign keys for vault key wraps.
- Use one transaction and consume the registration challenge inside it.
- Keep `__Host-pv_session` secure and do not weaken it for plain HTTP preview.
- Treat client-supplied vault id collisions as generic registration failure.
- Avoid pretending CSRF is implemented before `GET /v1/csrf`.

Accepted suggestions:

- Added dedicated keyset and vault key-wrap tables.
- Added composite account/vault and account/key constraints.
- Added session idle/absolute columns.
- Mapped vault id uniqueness failures to `registration_unavailable`.
- Left CSRF hash null until CSRF endpoint is implemented.

Rejected or deferred suggestions:

- Server-generated vault id: deferred because the current API contract accepts
  `initial_vault.vault_id`; collisions are generic failures for now.
- Source/IP rate limiting and email verification: deferred to the public-registration hardening work.
- Production migration job: deferred; current live deployment still uses startup migrations as a
  temporary MVP mechanism.

## Subagent Review

An independent backend/security subagent reviewed the same slice and reached the same core
conclusions: add separate keyset/wrap storage, enforce composite foreign keys, implement one
transaction, and test cookie/session/schema constraints.

## Validation

Tested:

- Live Kubernetes app health/readiness and auth register start smoke test before code changes.
- Rust formatting and unit tests in `rust:1.96-bookworm`.
- Real PostgreSQL integration validation with temporary `postgres:18-alpine` on `127.0.0.1`.
- `cargo fmt --all -- --check`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`.
- `cargo test --locked --workspace` with `PV_TEST_DATABASE_URL`.

Verified:

- `register/finish` creates exactly one account, keyset, vault key wrap, device, setup session, and
  audit event.
- Reusing the same registration challenge returns `registration_unavailable`.
- Session cookie flags match the API contract.
- Database constraints reject invalid key material shapes, cross-account vault wraps, non-object
  device metadata, and invalid session lifetime ordering.

Not tested:

- Browser-side completion of `register/finish`; browser crypto is not implemented yet.
- Live deployment of this branch; image/GitOps update is a later step after PR merge.
- HTTPS cookie persistence, because the current live preview is HTTP.

## Risks

- Public self-registration still needs source-aware rate limiting and email/invite gating before real
  users.
- `runMigrationsOnStartup` remains a temporary deployment shortcut and should become a migration job
  before real users.
- PostgreSQL live deployment is still single-replica local-path and not HA.
- The current browser preview cannot store `Secure` cookies over plain HTTP.
- TOTP enrollment, login finish, CSRF, session inspection, and vault CRUD/sync are still missing.

## Next Steps

1. Push the product branch, open and merge a PR after CI passes.
2. Publish the new API image.
3. Update the infra GitOps image digest and sync/deploy.
4. Verify live migration and `register/finish` smoke.
5. Implement TOTP enrollment/confirmation and `GET /v1/csrf`.
