# Load Testing

Status: MVP load-test harness.

The load suite uses pinned `grafana/k6:2.0.0` Docker images so the mini-PC does not need a local k6
installation. Scripts generate synthetic, non-secret data only.

## Safety

- Do not point these tests at production without an explicit test window and rate limit.
- Test login handles use the reserved `.invalid` domain.
- No real passwords, TOTP seeds, vault secrets, or customer data are generated.
- PR smoke load is intentionally small. Use manual runs for heavier tests.

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

The manual GitHub Actions workflow `load-smoke` builds a local image on a GitHub-hosted runner,
starts disposable PostgreSQL, runs the API container, and executes the selected k6 scenario.

## Scenarios

- `health.js`: health, readiness, and metrics scrape.
- `register_start.js`: synthetic registration challenge issuance.
- `login_start.js`: unknown-account login challenge issuance with synthetic metadata.
- `smoke.js`: mixed health/register/login smoke with low default rate.

## Defaults

- `BASE_URL`: `http://127.0.0.1:8080`
- `RUN_ID`: `manual`
- `LOAD_RATE`: `2`
- `LOAD_DURATION`: `15s`

PR checks should keep `LOAD_RATE` low. Nightly or manual runs can raise it after the database,
cleanup, and rate-limit behavior are reviewed.
