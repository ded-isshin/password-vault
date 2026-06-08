# Research Note: Google SRE Observability For Password Vault

Date: 2026-06-08

Status: sidecar analysis. This note does not change rollout scope and should not block the current
main deployment. It summarizes how the official Google SRE observability guidance applies to the
Password Vault MVP.

## Why This Matters

Password Vault is a security-sensitive, stateful product. Basic HTTP uptime is not enough: users
must be able to register, pass MFA, unlock, save encrypted items, sync, and later recover access to
their secrets. Observability must therefore cover both the technical service path and aggregate
product/security outcomes without exposing user data.

## Official Documentation Checked

Source availability: available through the network on 2026-06-08.

Only official Google SRE sources were used:

- Google SRE Book, "Monitoring Distributed Systems":
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, "Monitoring":
  <https://sre.google/workbook/monitoring/>
- Google SRE Workbook, "Alerting on SLOs":
  <https://sre.google/workbook/alerting-on-slos/>
- Google SRE Workbook, "Implementing SLOs":
  <https://sre.google/workbook/implementing-slos/>

## Google SRE Principles Applied

Google SRE defines the four golden signals as latency, traffic, errors, and saturation. For Password
Vault, they should be interpreted through user-visible security journeys:

| Golden signal | Password Vault interpretation |
| --- | --- |
| Latency | Separate normal API latency from auth/MFA/unlock paths. Fast 500s are still failed requests; slow auth may be expected but must have its own SLO. |
| Traffic | Track total HTTP demand, but also registration, login, MFA, vault item writes, and sync demand. RPS alone does not describe the product. |
| Errors | Track 5xx, policy failures, MFA/auth failures, CSRF/rate-limit failures, synthetic journey failure, and bad content/correctness failures where HTTP status is insufficient. |
| Saturation | Track pending HTTP requests, DB pool pressure, DB query latency, auth challenge pressure, CPU/memory, PostgreSQL replica lag, disk/WAL/backup pressure. |

Google SRE also distinguishes symptoms from causes. The dashboard should first answer what users
see: can they reach the app, authenticate, unlock, save, and sync? Cause-level panels such as DB
latency, pool pressure, pod restarts, and replication lag should support triage without becoming the
only paging signal.

The Workbook warns that entity IDs with millions of possible values are not practical metric labels.
For this product, user/account/vault/item/device/IP/login-handle labels are forbidden in metrics.
Use aggregate counters, histograms, logs, and incident scripts for deeper investigation.

## Current Local Evidence

Inspected local files:

- `docs/observability-sre-metrics.md`
- `load/README.md`
- `load/k6/lib/config.js`
- `load/k6/scenarios/health.js`
- `load/k6/scenarios/smoke.js`
- `load/synthetic/browser-api-journey.mjs`
- `crates/api/src/telemetry.rs`

Current evidence from those files:

- The observability plan already uses Google SRE Golden Signals as its baseline.
- The intended Grafana dashboard covers target health, request rate, 5xx ratio, p95 latency,
  pending requests, unmatched 404s, build info, and first product counters.
- k6 health/smoke tests exercise health, readiness, internal metrics, registration start, and login
  start.
- The synthetic journey models:

```text
register -> confirm TOTP -> logout -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt
```

- The synthetic journey checks product metric families after the journey and, in the current local
  sidecar work, also expects public API `/metrics` denial and internal metrics success.

## Technical Metrics

### Already Present In Code Or Scrape Contract

| Metric | Category | Current use |
| --- | --- | --- |
| `up{job="password-vault-api"}` | availability/scrape health | Confirms VictoriaMetrics can scrape API targets. |
| `axum_http_requests_total` | traffic/errors | Request rate and status-class ratios. |
| `axum_http_requests_duration_seconds_bucket` | latency | p95/p99 HTTP latency by low-cardinality route label. |
| `axum_http_requests_pending` | saturation | In-flight request pressure. |
| `password_vault_build_info` | release context | Correlates runtime with version/revision. |

### Missing Or Not Yet Production-Strong

| Gap | Why it matters | Recommended priority |
| --- | --- | --- |
| External black-box synthetic metric | Low traffic makes error-budget alerts noisy; artificial traffic creates signal before real users are impacted. | High |
| DB pool usage and wait latency | Password Vault will fail or slow down when DB connections are exhausted. | High |
| DB query latency and DB error class counters | Needed to separate API slowness from database slowness. | High |
| PostgreSQL HA, replica lag, WAL/archive, backup age, restore drill age | A password manager is not stable until acknowledged writes survive node/database failure. | High |
| Auth/MFA step duration and challenge pressure | Server-side auth proof verification should stay bounded; the expensive password KDF is browser-side in the MVP. | Medium |
| CPU/memory/restarts/PDB/topology spread panels | Needed to explain saturation and rollout safety. | Medium |
| Metrics freshness and scrape age | Distinguishes "healthy zero" from missing telemetry. | Medium |
| Public `/metrics` denial evidence | Security-sensitive metrics should be internal only. | Medium |

## Product And Business Metrics

These are not marketing metrics. They are aggregate product-health signals that say whether the
security journey works.

### Already Present In Code

| Metric | Product question |
| --- | --- |
| `password_vault_registration_events_total{event,outcome}` | Can a new user start and finish registration? |
| `password_vault_accounts_created_total{outcome}` | Are accounts actually created? |
| `password_vault_login_starts_total{outcome}` | Can returning access start? |
| `password_vault_login_attempts_total{outcome,failure_class}` | Do login proofs succeed, and why do they fail in aggregate? |
| `password_vault_session_events_total{event,outcome}` | Are sessions created and upgraded after MFA? |
| `password_vault_mfa_events_total{event,outcome}` | Can users enroll and verify TOTP? |
| `password_vault_vault_item_changes_total{operation,outcome}` | Can encrypted items be written? |
| `password_vault_sync_requests_total{outcome,page}` | Can vault state be synced? |

### Missing Product/Business Metrics

| Metric concept | Why it matters |
| --- | --- |
| Protected activation ratio | Registration is not meaningful until MFA is confirmed and at least one encrypted item is saved. |
| Returning access success ratio | A password manager fails if users cannot log in, pass MFA, and unlock. |
| First encrypted item saved | Better MVP activation signal than raw account creation. |
| Vault read/decrypt synthetic success | HTTP 200 is insufficient if the browser cannot decrypt or validate item state. |
| Sync conflict and stale revision rate | Required before multi-device usage is safe. |
| Active sessions | Useful aggregate security/product pressure signal. |
| CSRF/rate-limit/security event counters | Needed to see abuse without exposing identities. |
| Recovery-code flow success/failure | Required before account recovery can be considered usable. |
| Account cleanup/synthetic account lifecycle | Prevents synthetic tests from becoming unbounded durable data. |

## Grafana Dashboard Intent

The dashboard should be organized by operator/product questions, not by metric namespace:

| Row | Required panels |
| --- | --- |
| User-visible availability | Edge black-box probe, `up`, readiness, 5xx ratio. |
| Golden signals | Request rate, p95/p99 latency, errors, pending requests. |
| Auth and MFA | Login starts, login outcomes, MFA enrollment/verification outcomes, auth latency. |
| Vault and sync | Item write success, sync success, conflict/stale revision rate, synthetic read/decrypt result. |
| Durability | PostgreSQL primary/replica health, replication lag, WAL/archive state, backup age, restore drill age. |
| Saturation | DB pool wait, auth challenge pressure, CPU/memory, pending requests, disk pressure. |
| Release context | Build revision, image digest, Argo revision, rollout generation, migration hook status. |
| Abuse/security | Rate-limit hits, CSRF failures, invalid origin/fetch metadata, MFA failure rate, unmatched 404s. |

Important dashboard rule: zeros are acceptable only when a metric is genuinely present and the
event count is zero. Missing telemetry must be visually different from a healthy zero.

## Alerting Classification

### Page

Use pages only for clear, user-impacting or imminent severe failures:

- zero scrapeable/ready API targets for a sustained short window;
- fast multi-window SLO burn on product endpoints with enough request or synthetic traffic;
- sustained high 5xx ratio on non-health product endpoints;
- sustained p99 latency breach on product/auth endpoints with enough traffic;
- no writable product database for real-secret environments;
- backup/WAL/durability failure that means acknowledged secrets may not survive a node/database
  failure;
- security event that is actively causing service impact or likely secret exposure.

### Ticket

Use tickets for important issues that need action but usually do not require waking someone:

- fewer than expected API replicas while enough capacity remains;
- slow SLO burn or sustained degraded latency below page threshold;
- DB pool pressure trend, query latency trend, or replica lag before user impact;
- backup age warning before the critical durability threshold;
- missing build info, stale scrape, dashboard data missing;
- migration hook failure when rollout is paused but users are not currently impacted;
- Alertmanager routing/delivery not tested;
- high MFA/login failure rate without service saturation or confirmed attack impact.

### Dashboard-Only

Use dashboard-only signals for context and product learning:

- raw registrations, login starts, MFA enroll starts, and account creation counts;
- product funnel ratios before they have validated denominators and thresholds;
- low-volume unmatched 404s;
- normal sync conflicts that are correctly rejected and surfaced to clients;
- one-off synthetic journey result during manual testing;
- business trend lines without an attached reliability decision.

## Forbidden High-Cardinality Or Sensitive Labels

Never use these as metric labels:

- login handle, email, username, account ID;
- vault ID, item ID, device ID, session ID, request ID;
- IP address, hostname, private domain, user agent;
- raw path, query string, URL, referer, origin;
- OTP/TOTP values, recovery codes, passwords, account secret keys, encrypted payloads;
- database row IDs, SQL text, exception text containing values;
- arbitrary client-provided strings.

Allowed labels should be small controlled enums, for example:

- `endpoint` or datasource-renamed `exported_endpoint` with route templates only;
- `method`;
- `status` or status class;
- `outcome`;
- `event`;
- `failure_class`;
- `operation`;
- `page="complete|partial"`;
- `policy`;
- `error_class`.

## Recommended Next Steps

1. Keep the current internal-only metrics listener work moving; this is a real security boundary
   improvement and should not be diluted with dashboard redesign.
2. Add an external or edge-equivalent synthetic metric for the full protected-user journey before
   treating SLO burn alerts as reliable in low-traffic MVP conditions.
3. Add DB pool/query/error metrics and PostgreSQL HA/backup/restore panels before accepting real
   secrets.
4. Split alert rules into page, ticket, and dashboard-only severities; do not page on raw product
   counters.
5. Add a metrics-label regression test that fails if forbidden labels or raw paths appear in
   `/metrics`.
6. Make Grafana panels distinguish missing series from healthy zero.
7. Keep business/product metrics aggregate-only until explicit privacy and analytics policy exists.

## What Not To Do

- Do not add dashboards just because a metric exists.
- Do not page on registration/login/MFA raw counts.
- Do not use user-identifying or object-identifying labels to make Grafana drilldowns convenient.
- Do not treat `up == 1` as proof that users can unlock or sync.
- Do not treat HTTP 200 as proof that encrypted vault data is correct or decryptable.
- Do not call the system L3/L4 observable until synthetic journey and durability evidence exist.
