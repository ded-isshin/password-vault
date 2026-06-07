# Session Report: Browser Registration KDF And Secret-Key Gate

## Goal

Make the browser registration and TOTP enrollment slice compatible with the backend contract and fix
the account-secret-key data-loss blocker found by Codex and Claude Code review.

## Active Context

- Active repository: `password-vault`
- Active branch: `codex/mvp-vault-browser`
- Scope: backend KDF profile, database migration, static browser registration UI, API/docs.

## Work Completed

- Changed new registration challenges to use `pbkdf2-sha256-browser-v1`.
- Added a migration that converts pre-MVP `argon2id-browser-v1` rows to
  `pbkdf2-sha256-browser-v1` and then restricts `accounts.kdf_profile` to the new browser MVP
  profile.
- Removed legacy Argon2id login metadata support because returning mixed KDF profiles would make
  legacy accounts distinguishable from unknown login handles.
- Updated test helper accounts to use the new PBKDF2 browser-MVP profile.
- Updated the static browser registration flow so it pauses after deriving local key material and
  before `register/finish`.
- Added copy/download controls for the generated account secret key.
- Added a required "I saved this account secret key" confirmation before the server account is
  created.
- Preserved retry behavior for TOTP enrollment after `register/finish` succeeds.
- Updated API and crypto documentation so PBKDF2 is an explicit browser-MVP decision, not a silent
  fallback.

## Decision

The first browser MVP uses WebCrypto-native PBKDF2-HMAC-SHA-256 with 600,000 iterations because
WebCrypto does not provide Argon2id and the project has not yet reviewed a browser Argon2id WASM
dependency.

Argon2id remains the future hardening target after dependency pinning, known-answer tests, and
supply-chain review.

## Sources Consulted

- OWASP Password Storage Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html>
- W3C Web Cryptography:
  <https://www.w3.org/TR/webcrypto/>

## Validation

Tested:

- `node --check crates/api/static/app.js`
- `git diff --check`
- `docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/workspace:ro" -w /workspace rust:1.96-bookworm sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; export CARGO_TARGET_DIR=/tmp/password-vault-target; cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets -- -D warnings && cargo test --locked --workspace'`
- `cargo test --locked --workspace -- --test-threads=1` in `rust:1.96-bookworm` against a
  disposable `postgres:18-alpine` container with `PV_TEST_DATABASE_URL`.
- Manual disposable-PostgreSQL migration check: applied migrations 001 and 002, inserted one
  pre-MVP `argon2id-browser-v1` account row, applied migration 003, verified the row became
  `pbkdf2-sha256-browser-v1`, and verified a new Argon2id-profile insert is rejected.
- Claude Code initial review found two blockers: migration not included in the pasted diff and
  legacy Argon2id account enumeration risk.
- Claude Code follow-up review confirmed those blockers are resolved and found no new blocking
  findings in the updated diff.

Pending:

- Full browser/API registration and TOTP enrollment smoke after the backend starts with the new
  migration.

## Risks

- PBKDF2 is weaker than memory-hard Argon2id against GPU/ASIC guessing. The account secret key
  reduces copied-database password-only guessing risk, but Argon2id remains the hardening target.
- The browser MVP still lacks login finish, login-time TOTP verification, vault unlock, and encrypted
  item CRUD.
- Recovery code save/download UX is still weaker than the new account-secret-key gate.
