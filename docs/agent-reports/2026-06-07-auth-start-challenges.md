# Session Report: Auth Start Challenges

## Goal

Advance issue #16 with a DB-backed auth start slice before implementing registration finish, login
proof verification, sessions, or TOTP verification.

## Active Context

- Active repository: `password-vault`
- Branch: `feat/16-auth-challenges`
- Out of scope: frontend, Kubernetes, infrastructure repository, `register/finish`, `login/finish`,
  session cookies, TOTP enrollment, TOTP verification

## Work Completed

- Added `POST /v1/auth/register/start`.
- Added `POST /v1/auth/login/start`.
- Added Base64url no-padding helpers with fixed-length decode support.
- Added canonical login auth-message transcript helper for the future `login/finish` proof binding.
- Persisted register/login start state in `auth_challenges`.
- Added deterministic unknown-account synthetic metadata using `PV_SYNTHETIC_METADATA_KEY_B64` and
  HMAC-SHA-256 domain separation.
- Added `Cache-Control: no-store` middleware for auth start routes and generic error envelopes for
  strict JSON extraction failures.
- Added per-normalized-handle DB-backed challenge rate limiting.
- Tightened initial schema salts to exactly 32 bytes and verifier iterations to exactly `150000`.
- Tightened the MVP KDF profile shape in the initial schema so existing-account login metadata matches
  the synthetic unknown-account response shape.
- Tightened auth challenge server nonces to exactly 32 bytes.
- Updated PostgreSQL CI to run the full workspace test suite sequentially with a disposable database.
- Redacted `ApiConfig` debug output so runtime DB URLs and synthetic metadata keys are not printed.

## Decisions

- Keep this PR limited to start/challenge issuance. Finish flows and sessions remain a later #16
  slice.
- Treat `combined_nonce` as `base64url_no_pad(client_nonce || server_nonce)`, with both nonces decoded
  as 32-byte values.
- Duplicate `register/start` returns normal `200` shape and creates a short-lived registration
  challenge; duplicate conflict is enforced at `register/finish`.
- Unknown-account login metadata must be keyed by a runtime secret, not by public hashing.

## Files Changed

- `.github/workflows/rust.yml`
- `Cargo.lock`
- `Cargo.toml`
- `crates/api/Cargo.toml`
- `crates/api/src/auth/encoding.rs`
- `crates/api/src/auth/mod.rs`
- `crates/api/src/auth/routes.rs`
- `crates/api/src/auth/transcript.rs`
- `crates/api/src/lib.rs`
- `crates/api/tests/migrations.rs`
- `docs/api-contract.md`
- `docs/development.md`
- `docs/security/auth-protocol-v1.md`
- `docs/agent-reports/2026-06-07-auth-start-challenges.md`
- `migrations/202606070001_initial_schema.sql`

## Validation

Tested:

- `cargo fetch --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`
- Disposable `postgres:17-bookworm` with
  `PV_TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/password_vault_test`
- `cargo test --locked --workspace -- --test-threads=1` against the disposable PostgreSQL container
- `git diff --check`
- Public-safety grep for common token, private-key, password, and database URL patterns

All Rust commands were run in `rust:1.85-bookworm` with
`PATH=/usr/local/cargo/bin:$PATH`.

Results:

- Non-DB validation: 28 unit tests, 1 migration test, and doc-tests passed. The DB-backed auth route
  test skipped its database assertions because `PV_TEST_DATABASE_URL` was absent.
- DB validation: 28 unit tests, 1 migration test, and doc-tests passed against disposable PostgreSQL.
- Whitespace validation passed.
- Public-safety grep produced only expected false positives: dummy local PostgreSQL URLs, TOTP URI
  placeholder/test values, an RFC SCRAM test password, and token-generation code.

Not tested:

- `register/finish`.
- `login/finish` proof verification.
- Session cookie creation.
- TOTP enrollment and verification.
- Browser/frontend integration.
- Kubernetes deployment.

## Reviews

- Subagent architecture/security review completed before merge preparation.
- Claude Code independent review completed before final report preparation.
- Accepted findings:
  - synthetic metadata requires a runtime secret;
  - schema metadata must be exact-shape for constant login response shape;
  - client nonce and challenge metadata must be persisted for future proof verification;
  - `combined_nonce` order/encoding must be canonical;
  - duplicate `register/start` behavior must be explicit;
  - unauthenticated write routes need body size, TTL, cleanup/index, and rate-limit boundaries.
- Follow-up review found two blockers:
  - `ApiConfig` needed redacted `Debug` because it contains `synthetic_metadata_key`;
  - stored `kdf_profile` needed exact-shape enforcement to avoid existing-vs-unknown response shape
    drift.
  Both were fixed before PR publication.
- Accepted Claude Code findings:
  - exact response-shape parity depends on enforcing one MVP KDF profile shape;
  - exact response-shape parity also depends on pinning stored verifier iterations to the synthetic
    metadata default;
  - cleanup and rate-limit behavior are acceptable for the MVP but need revisit before public
    deployment;
  - `synthetic` in stored challenge metadata must not be echoed by future finish endpoints.
- Final Claude Code review after the hardening found no blocking issues for this auth-start PR slice.

## Risks

- `login/finish` still must verify the canonical transcript against the persisted challenge.
- Rate limiting is currently per normalized handle using `auth_challenges`; source/network-aware and
  global throttling remain future hardening tasks before public deployment.
- Expired challenge cleanup currently runs on auth start requests; this is acceptable for MVP but
  should move to a bounded/background cleanup path if auth traffic grows.
- Login challenge storage includes a `synthetic` boolean for finish-path policy decisions. Future
  endpoints must not echo it to clients.
- `PV_SYNTHETIC_METADATA_KEY_B64` is a required runtime secret for DB-backed auth routes and must be
  supplied by deployment configuration, never by repository content.
- The initial migration was edited in place while the project is still pre-deployment/greenfield. If
  any durable environment has already applied the old checksum, replace this with an additive
  follow-up migration before deployment.
- Auth start routes do not yet create sessions or grant vault access.

## Next Steps

- Open a PR that references #16 but does not close it.
- Next #16 code slice: `register/finish` and `login/finish` with proof verification and session setup
  state.
