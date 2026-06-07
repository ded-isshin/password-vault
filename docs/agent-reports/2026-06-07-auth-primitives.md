# Session Report: Auth Primitives

## Goal

Start issue #16 with a small, testable backend slice: implement authentication primitives before
adding database-backed HTTP auth handlers.

## Active Context

- Active repository: `password-vault`
- Branch: `feat/16-auth-primitives`
- Out of scope: frontend, Kubernetes, infrastructure repository, runtime secret configuration,
  full `/v1/auth/*` handlers

## Work Completed

- Added `crates/api/src/auth/scram.rs` with `pv-scram-sha-256-v1` verifier/proof helpers.
- Added `crates/api/src/auth/totp.rs` with RFC 6238 TOTP generation/verification, replay-window
  behavior, Base32 encoding, and Google Authenticator-compatible provisioning URI generation.
- Added `crates/api/src/auth/tokens.rs` for 32-byte random tokens and SHA-256 token verifiers.
- Added direct workspace dependencies for HMAC/SHA/rand/subtle and test-only Base64 coverage.
- Exposed the auth module from the API crate.
- Addressed independent review hardening findings:
  - `ScramVerifier` does not expose public verifier-material fields, does not implement `Debug`,
    and does not derive equality over verifier material.
  - SCRAM primitives reject iteration counts below `4096` and above `1_000_000`.
  - SCRAM primitives reject salts shorter than 16 bytes; future handlers should generate 32-byte
    salts by default.
  - TOTP primitives reject seeds shorter than 20 bytes.
  - TOTP provisioning URIs are built from raw seed bytes after validation, not from caller-supplied
    arbitrary `secret` strings.

## Decisions

- Keep this PR limited to primitives and test vectors. DB-backed endpoints will be the next #16
  slice.
- Use RFC 7677 SCRAM-SHA-256 example as the verifier/proof regression test.
- Use RFC 6238 Appendix B vectors for SHA1/SHA256/SHA512 TOTP regression tests.
- Use constant-time comparison for SCRAM stored-key verification and TOTP code comparison.
- Keep Base32/URI generation local and small instead of adding a new dependency for one enrollment
  helper.
- Keep `base64` as a dev-only API crate dependency because it is only used for RFC vector tests.

## Files Changed

- `Cargo.lock`
- `Cargo.toml`
- `crates/api/Cargo.toml`
- `crates/api/src/auth/mod.rs`
- `crates/api/src/auth/scram.rs`
- `crates/api/src/auth/tokens.rs`
- `crates/api/src/auth/totp.rs`
- `crates/api/src/lib.rs`
- `docs/agent-reports/2026-06-07-auth-primitives.md`

## Validation

Tested:

- `cargo fetch --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`

All Rust commands were run in `rust:1.85-bookworm` with
`PATH=/usr/local/cargo/bin:$PATH`.

Final passing test result: 20 unit tests, 1 migration test, and doc-tests passed.

Earlier validation attempts failed on formatting and one clippy warning about assertions on constants;
both were fixed before the passing validation above.

Not tested:

- HTTP auth/session handlers.
- Database-backed registration/login/TOTP enrollment flows.
- Browser/frontend integration.
- Kubernetes deployment.

## Reviews

- Subagent review: blocking findings addressed.
  - Removed `Debug`/public fields from `ScramVerifier`.
  - Added explicit SCRAM iteration policy constants and boundary tests.
  - Added SCRAM salt-length validation.
  - Added TOTP seed-length validation and replay/window edge-case tests.
- Follow-up subagent review: no blockers after hardening. Accepted the report-truthfulness nit and
  SCRAM salt-length recommendation.
- Claude Code review: no blockers. Accepted non-blocking recommendations for `Debug`, iteration
  policy, additional tests, equality-footgun removal, and token-hash usage documentation. Deferred
  `zeroize`, canonical HTTP auth transcript encoding, and DB-atomic replay persistence to later #16
  slices.

## Claude Code Usage

Purpose: independent architecture/security review of the auth primitive diff.

Prompt/task given: review the working-tree diff for `crates/api/src/auth/*`, dependency changes, and
alignment with auth/MFA documentation.

Summary of output: Claude found no merge blockers and verified SCRAM/TOTP correctness against the
published RFC vectors. It recommended hardening around debug output, iteration policy, test gaps,
equality over verifier material, token-hash usage documentation, and future canonical
transcript/persistence boundaries.

Accepted suggestions: debug-output hardening, iteration policy, extra TOTP window/replay tests,
dev-only Base64 dependency placement, equality-footgun removal, and token-hash usage documentation.

Rejected/deferred suggestions: `zeroize` integration and DB/HTTP transcript work, because this PR is
intentionally limited to primitives.

## Risks

- `pv-scram-sha-256-v1` still needs HTTP transcript canonicalization in the handler PR.
- TOTP seed encryption and storage are not implemented in this slice.
- Rate limits, sessions, recovery-code storage, cookies, and CSRF are not implemented in this slice.
- TOTP replay protection still depends on a future atomic database update of `last_accepted_step`.

## Next Steps

- Open a PR that references #16 but does not close it.
- Next code slice: DB-backed registration/login challenge handlers and session cookie creation.
