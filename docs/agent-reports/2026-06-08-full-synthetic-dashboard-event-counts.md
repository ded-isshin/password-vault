# Full Synthetic Journey And Dashboard Event Counts

Date: 2026-06-08

## Goal

Add a repeatable protected-user synthetic journey and verify that Password Vault product metrics can
drive useful Grafana panels.

## Active Context

- Product repository: `password-vault`
- Infrastructure repository: `infrastructure-home`, dashboard JSON only
- Live preview: checked through an explicitly approved LAN edge route, redacted here

## Work Completed

- Added `load/synthetic/browser-api-journey.mjs`.
- Wired the journey into PR container smoke and the manual `load-smoke` workflow.
- Kept k6 for low-rate load/API smoke and used Node/WebCrypto for the browser-equivalent protected
  journey.
- Pinned the Node synthetic runner image by digest.
- Tightened the local `/metrics` check to require expected product counter label sets to be greater
  than zero after the journey.
- Fixed a real `PVSK1` account-secret display/parser edge case: `-` was used both as a group
  separator and as a valid base64url character. The display now groups with spaces and the parser
  preserves base64url `-` while keeping a best-effort legacy dash-group fallback.
- Updated load, development, MVP, README, and observability docs.
- Found that dashboard product-counter panels using `rate(...[$__rate_interval])` showed zero for
  sparse one-off business events even though `increase(...[30m])` saw the events.
- Prepared an infrastructure dashboard fix to use `increase(...[$__range])` and count units for
  product/business event panels.

## Synthetic Journey Covered

```text
register -> confirm TOTP -> logout -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt
```

The journey also checks:

- `Cache-Control: no-store` on sensitive JSON responses;
- `__Host-pv_session` flags: `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`, no `Domain`;
- setup session has `vault_access=false`;
- verified session has `vault_access=true`;
- unsafe vault item write fails without CSRF;
- locally computed vault head hash matches the server response;
- synced item envelope hash, change MAC, head hash, AAD, and AES-GCM decryption all validate.

## Public Safety

- Synthetic login handles use the reserved `.invalid` domain.
- The script does not print account secret keys, TOTP seeds, TOTP codes, recovery codes, cookies,
  plaintext item passwords, account IDs, vault IDs, item IDs, or device IDs.
- Live edge examples in documentation use placeholders.
- The script refuses non-local `BASE_URL` unless `SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL=true`.

## Validation

Tested:

- `node --check load/synthetic/browser-api-journey.mjs`
- `node --check crates/api/static/app.js`
- `docker run ... node:22-bookworm-slim@sha256:7af03b14... node --check load/synthetic/browser-api-journey.mjs`
- deterministic synthetic self-check for `PVSK1` values containing base64url `-` and `_`
- live edge journey through the digest-pinned Node image with metrics check disabled because edge
  `/metrics` should not be public
- `git diff --check`
- workflow YAML parse check with Python/PyYAML
- infrastructure dashboard JSON validation with `jq empty`
- infrastructure GitOps render with `kubectl kustomize kubernetes/gitops/prod`

Verified through Grafana/VictoriaMetrics after live synthetic traffic:

- `sum(up{job="password-vault-api"})` returned `3`.
- `increase(password_vault_registration_events_total[30m])` showed registration start and finish.
- `increase(password_vault_mfa_events_total[30m])` showed TOTP enrollment start/confirm and login
  TOTP challenge/verify.
- `increase(password_vault_vault_item_changes_total[30m])` showed one successful create.
- `increase(password_vault_sync_requests_total[30m])` showed two successful complete syncs.
- Dashboard-equivalent event-count queries with `increase(...[1h])` returned non-zero data.

## Claude Code Review

Purpose: independent architecture/security/observability review.

Accepted suggestions:

- Pin the Node synthetic runner image by digest instead of using the floating
  `node:22-bookworm-slim` tag.
- Add context for the login-time TOTP next-step code. The server verifier accepts
  previous/current/next steps and rejects reused accepted steps, so the journey uses the next step
  to avoid sleeping in CI.
- Strengthen local `/metrics` validation beyond metric-family presence.

Rejected or deferred suggestions:

- Do not convert the unmatched 404 panel in this change. It remains a rate-oriented HTTP/security
  pressure signal, while product/business counters use event counts.
- Do not replace product event time series with stat panels yet. `increase(...[$__range])` fixes the
  immediate false-zero dashboard issue; panel type tuning can be handled in a later dashboard design
  pass.

Not tested:

- Browser DOM automation with Playwright.
- Automatic external synthetic probe deployment.
- PostgreSQL HA, backup, restore, or failover.
- Product alert rules and burn-rate routing.

## Risks And Follow-Up

- The live preview still uses a single PostgreSQL StatefulSet and must not accept real user secrets.
- The live system is not L3 observability until an external synthetic probe or equivalent scheduled
  journey is deployed, scraped, and shown on the dashboard.
- The application build info metric still reports `revision="unknown"`.
- Account cleanup is not implemented, so live synthetic runs intentionally create durable synthetic
  test accounts.
