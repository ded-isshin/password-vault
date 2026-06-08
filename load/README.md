# Load Testing

Status: MVP load-test and synthetic journey harness.

The load suite uses pinned `grafana/k6:2.0.0` Docker images so the mini-PC does not need a local k6
installation. Scripts generate synthetic, non-secret data only.

The full protected-user journey uses a dependency-free Node script in `load/synthetic`. It mirrors
the browser cryptographic API path instead of using k6 because the journey requires PBKDF2, HKDF,
AES-GCM, SCRAM proof construction, TOTP, a cookie jar, CSRF, vault change MAC verification, and item
decryption.

## Safety

- Do not point these tests at production without an explicit test window and rate limit.
- Test login handles use the reserved `.invalid` domain.
- No real passwords, TOTP seeds, vault secrets, or customer data are generated.
- PR smoke load is intentionally small. Use manual runs for heavier tests.
- The synthetic journey does not print account secret keys, TOTP seeds, TOTP codes, recovery codes,
  cookies, plaintext item passwords, account IDs, vault IDs, item IDs, or device IDs.
- Do not run the synthetic journey automatically against a public or production endpoint. Live-edge
  runs must be explicit because account deletion is not implemented yet.

## Local Commands

Run a smoke scenario against an already running API:

```bash
docker run --rm --network host \
  -v "$PWD/load/k6:/scripts:ro" \
  -w /scripts \
  -e BASE_URL=http://127.0.0.1:8080 \
  -e RUN_ID=local-$(date +%s) \
  -e LOAD_RATE=2 \
  -e LOAD_DURATION=15s \
  grafana/k6:2.0.0 run scenarios/smoke.js
```

Run the full browser API journey against a local API:

```bash
NODE_SYNTHETIC_IMAGE="node:22-bookworm-slim@sha256:7af03b14a13c8cdd38e45058fd957bf00a72bbe17feac43b1c15a689c029c732"
docker run --rm --network host \
  -v "$PWD:/workspace:ro" \
  -w /workspace \
  -e BASE_URL=http://127.0.0.1:8080 \
  -e RUN_ID=local-$(date +%s) \
  -e SYNTHETIC_TIMEOUT_MS=120000 \
  "$NODE_SYNTHETIC_IMAGE" node load/synthetic/browser-api-journey.mjs
```

Run the same journey against an explicitly approved LAN/edge preview route:

```bash
NODE_SYNTHETIC_IMAGE="node:22-bookworm-slim@sha256:7af03b14a13c8cdd38e45058fd957bf00a72bbe17feac43b1c15a689c029c732"
docker run --rm --network host \
  -v "$PWD:/workspace:ro" \
  -w /workspace \
  -e BASE_URL=https://<redacted-host>:<redacted-port> \
  -e RUN_ID=edge-$(date +%s) \
  -e SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL=true \
  -e SYNTHETIC_TLS_INSECURE=true \
  -e SYNTHETIC_CHECK_METRICS=false \
  "$NODE_SYNTHETIC_IMAGE" node load/synthetic/browser-api-journey.mjs
```

`SYNTHETIC_CHECK_METRICS=false` is expected for edge routes where `/metrics` is intentionally not
publicly exposed.

The manual GitHub Actions workflow `load-smoke` builds a local image on a GitHub-hosted runner,
starts disposable PostgreSQL, optionally runs the full browser API journey, and executes the selected
k6 scenario.

## Scenarios

- `health.js`: health, readiness, and metrics scrape.
- `register_start.js`: synthetic registration challenge issuance.
- `login_start.js`: unknown-account login challenge issuance with synthetic metadata.
- `smoke.js`: mixed health/register/login smoke with low default rate.
- `synthetic/browser-api-journey.mjs`: one protected-user journey:
  `register -> confirm TOTP -> logout -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt -> logout -> login -> verify recovery code -> deny vault access -> re-enroll TOTP`.

## Defaults

- `BASE_URL`: `http://127.0.0.1:8080`
- `RUN_ID`: `manual`
- `LOAD_RATE`: `2`
- `LOAD_DURATION`: `15s`
- `SYNTHETIC_TIMEOUT_MS`: `120000`
- `SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL`: `false`
- `SYNTHETIC_CHECK_METRICS`: `true` for local API URLs, `false` for non-local URLs unless set

PR checks should keep `LOAD_RATE` low. Nightly or manual runs can raise it after the database,
cleanup, and rate-limit behavior are reviewed.
