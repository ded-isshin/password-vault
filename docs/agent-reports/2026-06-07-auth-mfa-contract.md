# Session Report: Auth, MFA, Session, And API Contract Prerequisite

## Goal

Resolve MVP prerequisites for TOTP seed custody, MFA hardening, session/CSRF policy, and concrete
`/v1` API shapes before implementing auth/session/TOTP backend flows.

Related issues: #4, #13, #16.

## Active Context

- Active repository: `password-vault`
- Branch: `docs/4-13-auth-mfa-contract`
- Out of scope: infrastructure repository, Kubernetes deployment, runtime secrets, cluster changes

## Work Completed

- Added ADR 0005 for TOTP, recovery-code, session, and CSRF policy.
- Added a TOTP seed custody and MFA hardening research note.
- Replaced the bootstrap API-contract draft with concrete `/v1` auth, MFA, session, device, vault
  sync, and audit request/response shapes.
- Updated the auth protocol document to use `pv-scram-sha-256-v1` verifier/proof material instead
  of vague challenge-bound proof language.
- Updated the first PostgreSQL migration to match the auth/session contract:
  - `auth_stored_key` and `auth_server_key` verifier fields;
  - direct session/account FK with cascade delete;
  - `session_state` for vault-access gating;
  - TOTP `factor_id`, seed AEAD metadata, and account uniqueness;
  - recovery-code per-code salt;
  - text `crypto_version` for encrypted revision metadata.
- Updated migration tests and data-model docs for the revised schema.
- Updated README, threat model, lifecycle, and ADR 0003 links/status.

## Decisions

- TOTP MVP profile: RFC 6238, SHA1, 6 digits, 30 second period, `T0 = 0`.
- TOTP seed custody: server-generated seed, shown once, stored encrypted with app-level AEAD under
  `PV_TOTP_SEED_KEY_B64` and `PV_TOTP_SEED_KEY_ID`.
- Vault/OpenBao Transit remains future hardening for server-owned auth secrets, not a user-vault
  decrypt path and not an MVP blocker.
- Recovery codes are login-factor recovery only and never vault decrypt recovery.
- Browser session cookie: `__Host-pv_session`, `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`,
  no `Domain`.
- CSRF: per-session `X-PV-CSRF`, Origin checks, Fetch Metadata where available, non-GET mutations.
- API contract is canonical in `docs/api-contract.md`; subagent draft material was integrated and
  the separate draft file was not kept.

## Subagents Used

- TOTP/MFA subagent created `docs/research/totp-seed-custody-mfa-hardening-2026-06-07.md`.
- API-contract subagent drafted endpoint/session/CSRF behavior. Its useful content was integrated
  into `docs/api-contract.md`; the temporary draft was removed to avoid duplicate sources of truth.

## Claude Code Used

Purpose: independent security/API architecture review for #4, #13, and #16 prerequisites.

Initial review findings:

- Blocking: sessions could orphan because the composite device FK did not protect nullable
  `device_id`.
- Blocking: schema had no per-session state despite the contract requiring `mfa_enrollment_required`,
  `mfa_recovery`, and `mfa_verified`.
- Blocking: `vault_item_revisions.crypto_version` was integer while the API contract used string
  crypto version identifiers.

Accepted fixes:

- Added direct session/account FK with cascade delete and a behavioral migration test.
- Added `sessions.session_state`.
- Changed revision `crypto_version` to text and documented row-level metadata persistence.
- Also accepted low-cost consistency fixes for TOTP factor IDs, seed AEAD metadata, and per-code
  recovery-code salts.

Follow-up review:

- Blocking findings: none.
- Claude confirmed B1-B3 were resolved with matching schema, docs, and test coverage.
- Non-blocking follow-up accepted in this branch: document PostgreSQL 15+ minimum for the current
  migration, while CI remains on PostgreSQL 17.

## Files Changed

- `README.md`
- `crates/api/tests/migrations.rs`
- `docs/adr/0003-auth-and-crypto-direction.md`
- `docs/adr/0005-mfa-session-and-csrf-policy.md`
- `docs/api-contract.md`
- `docs/auth-mfa-lifecycle.md`
- `docs/data-model.md`
- `docs/research/totp-seed-custody-mfa-hardening-2026-06-07.md`
- `docs/security/auth-protocol-v1.md`
- `docs/threat-model.md`
- `migrations/202606070001_initial_schema.sql`

## Commands Run

- `gh issue view 4 --json number,title,body,labels,state,url`
- `gh issue view 13 --json number,title,body,labels,state,url`
- `gh issue view 16 --json number,title,body,labels,state,url`
- `sed -n ... docs/api-contract.md`
- `sed -n ... docs/auth-mfa-lifecycle.md`
- `sed -n ... docs/security/auth-protocol-v1.md`
- `sed -n ... docs/threat-model.md`
- `docker run ... rust:1.85-bookworm ... cargo test --locked --workspace --test migrations`
- `docker run ... postgres:17-bookworm ... cargo test --locked --workspace --test migrations -- --nocapture`
- `docker run ... rust:1.85-bookworm ... cargo fmt --all -- --check`
- `docker run ... rust:1.85-bookworm ... cargo clippy --locked --workspace --all-targets -- -D warnings`
- `docker run ... rust:1.85-bookworm ... cargo test --locked --workspace`
- `git diff --check`
- Local equivalent of the public-safety grep workflow
- `claude -p --permission-mode plan --tools "Read,Glob,Grep" ...`

## Research And Sources Consulted

- RFC 5802 SCRAM
- RFC 7677 SCRAM-SHA-256
- RFC 6238 TOTP
- RFC 9807 OPAQUE
- OWASP Authentication Cheat Sheet
- OWASP Multifactor Authentication Cheat Sheet
- OWASP Session Management Cheat Sheet
- OWASP CSRF Cheat Sheet
- MDN Set-Cookie reference
- Google Authenticator Key URI Format

## Validation

Tested:

- Required-docs check passed.
- Public-safety grep passed.
- `git diff --check` passed.
- Full Rust CI equivalent in `rust:1.85-bookworm` passed:
  - `cargo fetch --locked`
  - `cargo fmt --all -- --check`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo test --locked --workspace`
- PostgreSQL migration test against disposable `postgres:17-bookworm` passed.

Not tested:

- Runtime auth/session/TOTP handlers, because this branch defines the prerequisite contract and
  schema but does not implement handlers.
- Browser/frontend behavior.
- Kubernetes/Argo CD deployment.

## Risks

- `pv-scram-sha-256-v1` still needs exact code-level transcript encoding and test vectors in #16.
- TOTP is not phishing-resistant; WebAuthn/passkeys remain a post-MVP improvement.
- App-level AEAD protects against database-only TOTP seed disclosure but not a compromised runtime
  process.
- The runtime TOTP seed-encryption key backup/restore process must be tested before real users.
- Browser-delivered JavaScript remains a structural web-MVP risk.

## Open Questions

- Exact browser KDF/crypto test vectors are still tracked by #3 and #17.
- Exact Chrome extension/mobile session model remains post-MVP and should not weaken cookie/CSRF
  decisions silently.
- Whether to relax SCRAM verifier columns for future OPAQUE accounts should be handled by a future
  auth migration.

## Next Steps

- Finish and merge this prerequisite PR after review/checks.
- Implement #16 auth sessions and TOTP MFA server flows using this contract.
- Implement #17 browser crypto package and encrypted payload test vectors.
- Implement #18 encrypted vault item API and sync conflict checks.

## Approval Needed

No infrastructure approval is needed for this PR. Infrastructure and Kubernetes deployment remain
blocked until the explicit GitOps/Argo CD work in later issues.
