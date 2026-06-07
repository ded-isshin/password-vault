# Agent Report: TOTP Enrollment Foundation

Date: 2026-06-07

## Goal

Implement the next MVP authentication slice: server-backed TOTP enrollment and confirmation for
fresh setup sessions, with encrypted TOTP seed storage, one-time recovery-code generation, session
upgrade to `mfa_verified`, Helm/runtime-secret wiring, CI updates, and documentation.

## Active context

- Product repository: `products/password-vault`
- Infrastructure repository: out of scope for this product PR, except later GitOps image rollout.
- Risk level: High, because this touches authentication, MFA seed custody, runtime secrets, and
  Kubernetes deployment readiness.

## Work completed

- Added `PV_TOTP_SEED_KEY_B64` config parsing, redacted debug output, and readiness checks.
- Added `POST /v1/mfa/totp/enroll/start`.
- Added `POST /v1/mfa/totp/enroll/confirm`.
- Stored pending TOTP seeds encrypted with `XChaCha20Poly1305`.
- Bound seed encryption AAD to account id, factor id, key id, AEAD label, and TOTP profile.
- Rotated the session cookie and cleared CSRF state on successful enrollment confirmation.
- Generated 10 one-time recovery codes and stored only salted account-bound hashes.
- Added DB-backed tests for happy path, CSRF fail-closed behavior, missing seed key, cross-site
  rejection, second-start overwrite behavior, cross-account factor rejection, old-cookie invalidation,
  and verified-session enrollment rejection.
- Wired the Helm chart to read `PV_TOTP_SEED_KEY_B64` from the `password-vault-auth` Kubernetes
  Secret key `totp-seed-key-b64`.
- Updated CI smoke/load workflows to provide a non-secret test TOTP seed key.
- Removed Rust job-container dependency on Docker Hub `rust:*` images after GitHub Actions failed
  during Docker Hub pull. Rust CI now installs Rust 1.96.0 with `rustup` on GitHub-hosted runners.
- Updated CI PostgreSQL service images to `postgres:18-alpine`, matching the current PostgreSQL 18
  major line used in local DB-backed validation.
- Updated API/security/MFA lifecycle docs and the MFA/CSRF ADR.

## Files changed

- `.github/workflows/container.yml`
- `.github/workflows/load.yml`
- `.github/workflows/rust.yml`
- `Cargo.lock`
- `Cargo.toml`
- `crates/api/Cargo.toml`
- `crates/api/src/auth/routes.rs`
- `crates/api/src/lib.rs`
- `deploy/helm/password-vault/README.md`
- `deploy/helm/password-vault/templates/deployment.yaml`
- `deploy/helm/password-vault/values.yaml`
- `docs/adr/0005-mfa-session-and-csrf-policy.md`
- `docs/api-contract.md`
- `docs/auth-mfa-lifecycle.md`
- `docs/mvp-implementation-plan.md`
- `docs/research/totp-seed-custody-mfa-hardening-2026-06-07.md`
- `docs/security/auth-protocol-v1.md`
- `docs/agent-reports/2026-06-07-totp-enrollment-foundation.md`

## Commands run

```bash
docker run --rm -v "$PWD:/workspace" -w /workspace rust:1.96-bookworm bash -c 'set -euo pipefail; export PATH=/usr/local/cargo/bin:$PATH; cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D warnings; cargo test --locked --workspace'
```

Result: passed. 38 library tests, 1 migrations test, and doctests passed.

```bash
set -euo pipefail
name="pv-test-postgres-$$"
cleanup() { docker stop "$name" >/dev/null 2>&1 || true; }
trap cleanup EXIT
docker run -d --rm --name "$name" -e POSTGRES_USER=pv -e POSTGRES_PASSWORD=pv -e POSTGRES_DB=pv -p 127.0.0.1::5432 postgres:18-alpine >/dev/null
port="$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$name")"
for i in $(seq 1 60); do
  if docker exec "$name" pg_isready -U pv -d pv >/dev/null 2>&1; then break; fi
  if [ "$i" = 60 ]; then echo "postgres did not become ready" >&2; exit 1; fi
  sleep 1
done
docker run --rm --network host -e "PV_TEST_DATABASE_URL=postgres://pv:pv@127.0.0.1:${port}/pv" -v "$PWD:/workspace" -w /workspace rust:1.96-bookworm bash -c 'set -euo pipefail; export PATH=/usr/local/cargo/bin:$PATH; cargo fmt --all -- --check; cargo clippy --locked --workspace --all-targets -- -D warnings; cargo test --locked --workspace'
```

Result: passed with real PostgreSQL. 38 library tests, 1 migrations test, and doctests passed.

```bash
docker run --rm -v "$PWD":/workspace -w /workspace alpine/helm:3.19.0 lint deploy/helm/password-vault
docker run --rm -v "$PWD":/workspace -w /workspace alpine/helm:3.19.0 template password-vault deploy/helm/password-vault --namespace password-vault --set image.tag=ci --set observability.vmServiceScrape.enabled=true >/tmp/password-vault-rendered.yaml
rg -n "PV_TOTP_SEED_KEY_B64|totp-seed-key-b64" /tmp/password-vault-rendered.yaml
git diff --check
```

Result: passed. Rendered deployment includes `PV_TOTP_SEED_KEY_B64` from Secret key
`totp-seed-key-b64`.

## Claude Code usage

Purpose: independent backend/security review and UI/design implications check.

Prompt/task given: review the current TOTP enrollment diff, deployment risks, missing tests, merge
safety before login finish, and 1Password-inspired but non-copying web UI implications.

Summary of output:

- Verdict: safe to merge as the next MVP slice before login finish.
- No blocking security or correctness issue found.
- Confirmed fail-closed config/readiness, AEAD binding, encrypted seed storage, session rotation,
  CSRF/same-origin checks, and recovery-code hashing.
- Deployment ordering must create the `totp-seed-key-b64` runtime secret key before rollout.
- Login-time TOTP verification must add rate limiting or lockout.
- Track future cleanup/TTL behavior for abandoned pending factors.
- UI should render QR/manual secret once, gate recovery-code saving, re-fetch CSRF after confirm,
  and stay visually inspired by but not copied from 1Password.

Accepted suggestions:

- Added edge tests for missing CSRF, cross-site request, missing TOTP seed key, second start
  overwrite, cross-account factor rejection, and verified-session enrollment rejection.
- Preserved explicit deployment-ordering requirement for the GitOps rollout.
- Kept login-finish rate limiting as a required follow-up instead of expanding this slice.

Rejected or deferred suggestions:

- Confirm replay/already-verified direct test: deferred because the session state machine already
  prevents normal repeated confirmation after success; a lower-level test can be added when login
  TOTP verification exists.
- Pending-factor cleanup TTL: deferred to the recovery/re-enrollment slice.
- Zeroize pass: deferred to a broader secret-material memory-hardening slice.

## Validation

Tested:

- Rust formatting, clippy, workspace tests.
- DB-backed tests against a temporary PostgreSQL container.
- Helm lint and rendered manifest.
- Git diff whitespace hygiene.
- Public-safety grep for common secret/private-key/kubeconfig/private-IP patterns.
- GitHub Actions failure mode where `postgres-migrations` could fail before checkout because the
  job container could not pull `rust:1.96-bookworm` from Docker Hub.

Verified:

- `PV_TOTP_SEED_KEY_B64` is required for DB-backed readiness.
- `PV_TOTP_SEED_KEY_B64` is redacted in `ApiConfig` debug output.
- TOTP seeds are not stored as plaintext.
- Recovery codes are returned once and only hashed values are stored.
- Successful TOTP confirmation rotates the session and invalidates the old cookie.
- CI has a PostgreSQL-backed Rust job using `PV_TEST_DATABASE_URL`.
- CI Rust jobs no longer depend on a Docker Hub Rust job container.

Not tested:

- Live Kubernetes rollout for this slice.
- Login-finish TOTP verification, because it is not implemented yet.
- Browser UI enrollment screens, because the current UI remains a static preview.

## Risks

- Rollout will fail closed unless the `password-vault-auth` Secret has key `totp-seed-key-b64`
  before the new deployment starts.
- Login-finish TOTP verification must implement rate limiting or lockout before real users.
- Runtime TOTP seed key backup/rotation custody remains a platform operation.
- The deployed public/demo instance must not be used for real user secrets until backup, restore,
  HTTPS, and failover gates are proven.

## Next steps

1. Commit and merge the product PR.
2. Wait for GHCR image publication and capture the immutable digest.
3. Add `totp-seed-key-b64` to the runtime Kubernetes Secret outside Git.
4. Update the infrastructure GitOps image digest and roll out via Argo CD.
5. Run live smoke for health, readiness, registration, TOTP enrollment, TOTP confirmation, session
   upgrade, and metrics.
6. Continue the next MVP slices: login finish, browser-side crypto/unlock, vault CRUD/sync, HTTPS,
   backup/restore, and UI implementation.
