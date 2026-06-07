# Session Report: Session And CSRF Foundation

Date: 2026-06-07

## Goal

Implement the session and CSRF foundation needed before TOTP enrollment and authenticated vault APIs.

## Active Context

- Product repository: `products/password-vault`
- Infrastructure repository: not modified in this slice yet
- Risk: medium, because the change touches session authentication and CSRF behavior

## Work Completed

- Added `GET /v1/session`.
- Added `GET /v1/csrf`.
- Added `POST /v1/auth/logout`.
- Added inbound `__Host-pv_session` cookie parsing with duplicate/malformed rejection.
- Added session lookup by SHA-256 verifier of the opaque cookie token.
- Added idle session refresh capped by absolute session expiry.
- Added CSRF token rotation: raw token returned once, only SHA-256 verifier stored.
- Added CSRF validation for logout when a valid session exists.
- Added Fetch Metadata and `Origin` rejection for cross-site unsafe logout attempts.
- Added idempotent logout behavior for missing/invalid sessions plus session-cookie clearing.
- Updated API contract, auth protocol notes, and MVP implementation status.

## Files Changed

- `crates/api/src/auth/routes.rs`
- `crates/api/src/lib.rs`
- `docs/api-contract.md`
- `docs/mvp-implementation-plan.md`
- `docs/security/auth-protocol-v1.md`
- `docs/agent-reports/2026-06-07-session-csrf-foundation.md`

## Commands Run

```bash
git -C /home/roman/ai-workspace/products/password-vault status --short --branch
git -C /home/roman/ai-workspace/worktrees/infrastructure-home-password-vault-observability status --short --branch
KUBECONFIG=/home/roman/.kube/config-prod kubectl -n argocd get app prod-root password-vault -o custom-columns=NAME:.metadata.name,SYNC:.status.sync.status,HEALTH:.status.health.status,REV:.status.sync.revision --no-headers
docker run --rm -v "$PWD:/workspace" -w /workspace rust:1.96-bookworm bash -c 'set -euo pipefail; export PATH=/usr/local/cargo/bin:$PATH; cargo fmt --all; cargo test --locked --workspace'
docker run -d --rm --name "pv-test-postgres-<pid>" -e POSTGRES_USER=pv -e POSTGRES_PASSWORD=pv -e POSTGRES_DB=pv -p 127.0.0.1::5432 postgres:18-alpine
docker run --rm --network host -e "PV_TEST_DATABASE_URL=postgres://pv:pv@127.0.0.1:<port>/pv" -v "$PWD:/workspace" -w /workspace rust:1.96-bookworm bash -c 'set -euo pipefail; export PATH=/usr/local/cargo/bin:$PATH; cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D warnings; cargo test --locked --workspace'
```

## Subagent Review

Purpose: independent backend/security checklist for session and CSRF foundation before TOTP.

Summary of output:

- Treat session/CSRF endpoints as blockers before TOTP enrollment.
- Accept only one valid base64url/no-padding `__Host-pv_session` cookie decoded to 32 bytes.
- Look sessions up by hash only; never persist raw session or CSRF tokens.
- Enforce idle and absolute expiry.
- Rotate CSRF verifier on `GET /v1/csrf`.
- Require CSRF, Origin, and Fetch Metadata protections for unsafe authenticated routes.
- Require CSRF for logout when a valid session exists.

Accepted suggestions:

- Duplicate/malformed session cookies now fail.
- Logout now requires the current CSRF token for a valid session.
- Logout rejects cross-site Fetch Metadata and mismatched `Origin`.
- CSRF rotation invalidates the previous token.
- Tests cover session status, CSRF issuance, CSRF rotation, stale token rejection, cross-site
  rejection, logout deletion, and cookie clearing.

Deferred suggestions:

- Full CSRF enforcement for future unsafe routes is deferred until those routes exist.
- HTTPS browser preview remains a deployment follow-up; cookie flags were not weakened for HTTP.

## Claude Code Usage

Purpose: independent architecture/security review of the session/CSRF diff.

Summary of output:

- Marked the foundation safe to merge before TOTP.
- Confirmed token hashing, cookie hardening, fail-closed cookie parsing, session validation,
  CSRF rotation, and logout behavior.
- Recommended adding negative tests for idle/absolute expiry, revoked devices, Origin mismatch,
  null stored CSRF hash, and idle refresh capping.

Accepted suggestions:

- Added negative DB tests for idle-expired sessions.
- Added negative DB tests for absolute-expired sessions with future compatibility `expires_at`.
- Added revoked-device invalidation tests.
- Added Origin mismatch rejection tests.
- Added null stored CSRF hash rejection tests.
- Added idle refresh capped-at-absolute tests.

Deferred suggestions:

- Multi-token CSRF support for concurrent tabs is deferred. The current contract is single-slot
  rotate-on-fetch and is documented in the API contract.

## Validation

Tested:

- Rust formatting through `cargo fmt --all`.
- Workspace tests through `cargo test --locked --workspace` in `rust:1.96-bookworm`.
- Real PostgreSQL validation with temporary `postgres:18-alpine`.
- `cargo fmt --all -- --check`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`.
- `cargo test --locked --workspace` with `PV_TEST_DATABASE_URL`.

Verified by tests:

- `GET /v1/session` returns unauthenticated without a cookie.
- `GET /v1/session` returns account/device/state/expiry with a valid cookie.
- `GET /v1/csrf` requires a session.
- `GET /v1/csrf` returns a 32-byte base64url token and stores only its hash.
- A second CSRF fetch rotates the token and invalidates the previous token.
- Logout with stale/missing CSRF fails and preserves the session.
- Logout with `Sec-Fetch-Site: cross-site` fails and preserves the session.
- Logout with the current CSRF token deletes the session and clears the cookie.
- Duplicate and wrong-length session cookies are rejected by the parser.
- Idle-expired sessions return unauthenticated and are revoked.
- Absolute-expired sessions return unauthenticated even when compatibility `expires_at` is future.
- Revoked devices invalidate their sessions.
- Origin mismatch rejects logout and preserves the session.
- A supplied CSRF token fails when the stored CSRF hash is null.
- Idle refresh is capped at absolute expiry.

Not tested yet:

- GitHub Actions, GHCR publish, GitOps deployment, and live cluster smoke for this branch.
- Browser cookie persistence over HTTPS; the current live preview remains plain HTTP.

## Risks

- The implementation provides session/CSRF foundation only; it does not implement TOTP enrollment,
  login finish, vault unlock, or vault item APIs.
- `POST /v1/auth/logout` is the only unsafe authenticated route currently enforcing CSRF because it
  is the only implemented unsafe authenticated route.
- Plain-HTTP preview cannot exercise real browser persistence for `Secure` cookies.

## Next Steps

1. Open, validate, and merge the product PR.
2. Publish a new image, update GitOps, and verify live session/CSRF endpoints.
3. Implement TOTP enrollment and confirmation on top of this session foundation.
