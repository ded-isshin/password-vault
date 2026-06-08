# Observability And SRE Metrics Plan

Status: draft. Scope: password-vault MVP API and product-owned telemetry contract.

This plan follows the Google SRE Four Golden Signals: latency, traffic, errors, and saturation.
Product instrumentation must stay public-repository safe: no private IPs, hostnames, secrets, login
handles, account IDs, device IDs, item IDs, request bodies, encrypted payloads, OTP codes, or raw
paths as metric labels.

Official sources checked:

- Google SRE Book, "Monitoring Distributed Systems".
- Google SRE Workbook, "Monitoring".
- Google SRE Workbook, "Alerting on SLOs".
- Google SRE Workbook, "Implementing SLOs".

## Ownership Boundaries

- Product repo owns application metric names, safe labels, `/metrics` exposure behavior, Helm scrape
  contract, load-test checks, and this SRE plan.
- Infrastructure repo owns production values, Grafana dashboards, VictoriaMetrics/Prometheus rule
  deployment, notification routing, retention, and external synthetic probes.
- `/metrics` is exposed on the API service port today. If ingress is enabled, operators must block
  public access to `/metrics` or move scraping to an internal-only path/listener.

## Current Deployed State

Implemented and verified in the current GitOps preview as of 2026-06-08:

- Password Vault API is scraped with job label `password-vault-api`.
- The infrastructure repository provisions a basic Grafana dashboard named
  `Password Vault Overview`.
- The dashboard covers scrape target health, request rate, 5xx ratio, p95 request duration, pending
  requests, and unmatched 404 rate.
- The current dashboard queries have been verified against the live VictoriaMetrics datasource after
  edge health/readiness traffic:
  - `sum(up{job="password-vault-api"}) or vector(0)` returned `3`.
  - 5xx ratio returned `0`.
  - request-rate data returned for `/healthz` and `/readyz`.
  - p95 request duration returned roughly millisecond-level data for `/healthz` and `/readyz`.
  - pending requests returned `0`.
  - unmatched 404 rate returned `0`.
- The Grafana browser route is reachable through the mini-PC LAN-facing edge port. The Kubernetes
  `LoadBalancer` address is not the default client URL for a MacBook or other LAN-only browser.
- Argo CD is reachable through the mini-PC LAN-facing edge stream port and reports the product
  application as `Synced` and `Healthy`.
- A live browser-equivalent edge check from the mini-PC confirmed HTTP 200 for Password Vault,
  Grafana health, the `Password Vault Overview` dashboard URL, and Argo CD `/healthz`. A MacBook
  should use the mini-PC LAN-facing address and these edge ports, not Kubernetes/LXD service IPs.
- Grafana image rendering is not installed, so dashboard PNG rendering is not available from the
  Grafana MCP path. Dashboard verification must currently use live browser access plus datasource
  query checks rather than rendered-image evidence.
- The API Deployment has three ready replicas and is pinned to an immutable GHCR image digest.
- Strict node spreading is enabled. Production rollout values use `maxUnavailable: 1` and
  `maxSurge: 0` to avoid a surge-pod scheduling deadlock with
  `whenUnsatisfiable: DoNotSchedule`.
- Read-only cluster checks showed the product database is still a single `postgres:17-bookworm`
  StatefulSet on a node-local `local-path` PVC. CloudNativePG CRDs exist, but there is no active
  product `Cluster`, `Backup`, or `ScheduledBackup`, and no CloudNativePG operator/controller was
  observed in the current cluster scan.
- No `NetworkPolicy` exists in the `password-vault` namespace yet.
- Low-cardinality product counters for registration, account creation, login, MFA, session
  creation/upgrades, vault item changes, sync requests, and build information are merged,
  published, deployed, and covered by a low-cardinality `/metrics` test.
- The infrastructure GitOps dashboard is deployed with panels for those product counters. Grouped
  dashboard panels use `or on() vector(0)` for fallback, so they return `0` only when no left-hand
  series exists and do not add a permanent empty zero series.
- Live verification after deployment found non-zero registration and login-start series in
  VictoriaMetrics. MFA, vault item, and sync panels still need a fuller synthetic journey before
  non-zero live values can be verified.

Important label note:

- Application metrics use low-cardinality route labels from the Axum metrics layer.
- In the current VictoriaMetrics/Grafana path, the route label is queried as `exported_endpoint`
  because another scrape label already uses `endpoint`.
- Product docs may describe the application label as `endpoint`; production dashboards and alert
  rules must use the label name verified in the target datasource before they are accepted.

Planned:

- SLO and burn-rate panels.
- Business, product, and security panels.
- Database HA, backup, restore, and PostgreSQL health panels.
- External synthetic browser/API probes from outside the Kubernetes/LXD network.

## SLO And Error Budget Principles

Treat the MVP SLOs as candidate SLOs until the service has real traffic and stable dashboards.

| SLO | Candidate target | Good event | Exclusions |
| --- | --- | --- | --- |
| API availability | 99.5% over 30 days | product API request completes with status `< 500` | `/healthz`, `/readyz`, `/metrics`, unmatched 404s |
| Product API latency | 95% under 500 ms, 99% under 1500 ms | non-auth product endpoint request duration | health, readiness, metrics |
| Auth latency | 95% under 2 s, 99% under 5 s | auth endpoint request duration, allowing slow server-side hashing | invalid client input and rate-limited attempts may be tracked separately |
| Scrape health | 99.9% over 30 days | `up{job="password-vault-api"} == 1` | planned maintenance windows |

A 99.5% monthly availability SLO gives a 0.5% error budget, about 3 h 36 m over 30 days. Burn-rate
alerts should page on fast budget consumption and create tickets for slow budget consumption. Do not
page on expected user or attacker-caused 4xx responses by themselves; page when they cause saturation,
server errors, or security thresholds.

Useful PromQL building blocks:

```promql
sum(rate(axum_http_requests_total{job="password-vault-api"}[5m]))

sum(rate(axum_http_requests_total{
  job="password-vault-api",
  exported_endpoint!~"/(healthz|readyz|metrics|<unmatched>)",
  status=~"5.."
}[5m]))
/
sum(rate(axum_http_requests_total{
  job="password-vault-api",
  exported_endpoint!~"/(healthz|readyz|metrics|<unmatched>)"
}[5m]))

histogram_quantile(
  0.95,
  sum(rate(axum_http_requests_duration_seconds_bucket{
    job="password-vault-api",
    exported_endpoint!~"/(healthz|readyz|metrics|<unmatched>)"
  }[5m])) by (le)
)
```

## Technical Metrics

Map the Google SRE Golden Signals to product-specific telemetry as follows:

| Golden signal | MVP implementation | Product-specific interpretation |
| --- | --- | --- |
| Latency | HTTP duration histogram by route group and status class | Registration, login, MFA, unlock metadata, and vault sync must be tracked separately because auth hashing can be intentionally slower than normal reads. |
| Traffic | Request rate and product operation counters | Demand is not only total RPS; registration starts, login attempts, MFA verifies, vault item writes, and sync pulls are separate demand types. |
| Errors | 5xx ratio, policy errors, auth/MFA failure aggregates | 4xx from invalid users or attackers should not page by itself, but spikes are security/product signals. |
| Saturation | Pending requests, DB pool, DB latency, auth hash active work, pod CPU/memory, PostgreSQL lag/disk | Password-manager saturation includes expensive auth work and database write durability, not only HTTP queue depth. |

### Implemented

| Metric | Status | Golden signal | Primary use |
| --- | --- | --- | --- |
| `up{job="password-vault-api"}` | Implemented by scrape stack when `VMServiceScrape` is enabled | availability/saturation | Detect scrape target loss; not a full user-facing availability SLI by itself. |
| `axum_http_requests_total` | Implemented | traffic/errors | Request rate and HTTP error ratio by low-cardinality `endpoint`, `method`, and `status`. |
| `axum_http_requests_duration_seconds_bucket` | Implemented | latency | p50/p95/p99 request latency by endpoint/method. |
| `axum_http_requests_pending` | Implemented | saturation | In-flight request pressure and possible stuck downstream dependency. |

Current guardrail: unmatched routes collapse to `endpoint="/<unmatched>"`, avoiding unbounded path
cardinality.

### Implemented Product Metrics In Current Branch

These metrics are low-cardinality and intentionally do not use user, account, vault, item, device,
login-handle, OTP, path, host, or secret labels.

| Metric | Type | Labels | Why |
| --- | --- | --- | --- |
| `password_vault_build_info` | gauge | `version`, `revision` | Correlate deployed code with incidents and rollouts. |
| `password_vault_registration_events_total` | counter | `event`, `outcome` | Track the first-run journey without user identifiers. |
| `password_vault_accounts_created_total` | counter | `outcome` | Measure successful account creation. |
| `password_vault_login_starts_total` | counter | `outcome` | Separate login metadata/challenge issuance from proof verification. |
| `password_vault_login_attempts_total` | counter | `outcome`, `failure_class` | Track proof verification success and coarse failure classes. |
| `password_vault_session_events_total` | counter | `event`, `outcome` | Track session creation and MFA upgrade outcomes. |
| `password_vault_mfa_events_total` | counter | `event`, `outcome` | Track TOTP enrollment and login MFA outcomes. |
| `password_vault_vault_item_changes_total` | counter | `operation`, `outcome` | Track encrypted item create/update/delete success and conflict rates. |
| `password_vault_sync_requests_total` | counter | `outcome`, `page` | Track vault delta-sync success, conflict, and pagination. |

### Planned Technical Metrics To Add

| Metric | Type | Labels | Why |
| --- | --- | --- | --- |
| `password_vault_db_pool_connections` | gauge | `state="idle|used|max"` | Detect pool exhaustion before request failures. |
| `password_vault_db_pool_wait_duration_seconds_bucket` | histogram | `operation` | Track saturation when requests wait for a DB connection. |
| `password_vault_db_query_duration_seconds_bucket` | histogram | `operation`, `outcome` | Separate DB latency from application latency. |
| `password_vault_db_errors_total` | counter | `operation`, `error_class` | Alert on DB failures without leaking SQL or values. |
| `password_vault_auth_hash_duration_seconds_bucket` | histogram | `flow`, `outcome` | Watch slow server-side auth hashing cost and DoS risk. |
| `password_vault_auth_hash_active` | gauge | none | Track concurrent expensive hash work. |
| `password_vault_rate_limited_requests_total` | counter | `policy`, `endpoint` | Confirm rate limits are absorbing abusive traffic. |
| `password_vault_request_rejections_total` | counter | `reason`, `endpoint` | Track body-size, content-type, CSRF, and validation rejections. |
| `password_vault_background_job_runs_total` | counter | `job`, `outcome` | Track migrations, cleanup, or future maintenance jobs. |
| `password_vault_background_job_duration_seconds_bucket` | histogram | `job`, `outcome` | Detect slow operational jobs. |

## Business, Product, And Security Metrics

These metrics are aggregate counters and gauges for product health and abuse detection. They must
not be used with user-identifying labels.

| Metric | Type | Labels | Category | Status |
| --- | --- | --- | --- | --- |
| `password_vault_accounts_created_total` | counter | `outcome` | product | Implemented locally |
| `password_vault_login_attempts_total` | counter | `outcome`, `failure_class` | product/security | Implemented locally |
| `password_vault_active_sessions` | gauge | none | product/security | Planned |
| `password_vault_session_events_total` | counter | `event`, `outcome` | product/security | Implemented locally |
| `password_vault_mfa_events_total` | counter | `event`, `outcome` | security | Implemented locally |
| `password_vault_totp_verify_total` | counter | `outcome` | security | Superseded by `password_vault_mfa_events_total` for MVP |
| `password_vault_csrf_failures_total` | counter | `endpoint`, `reason` | security | Planned |
| `password_vault_security_events_total` | counter | `event_class`, `severity` | security | Planned |
| `password_vault_sync_requests_total` | counter | `outcome`, `page` | product | Implemented locally |
| `password_vault_sync_conflicts_total` | counter | `resource` | product | Planned |
| `password_vault_vault_item_changes_total` | counter | `operation`, `outcome` | product | Implemented locally |

Security dashboards should show rates and ratios, not raw operational logs. Example: failed login
rate, TOTP failure rate, rate-limit hit rate, CSRF failure rate, and session invalidation spikes.

Minimum useful product dashboards should be organized by funnel rather than by metric name:

| Funnel / health question | Metrics | Why it matters |
| --- | --- | --- |
| Can a new user become protected? | registration starts, registration finishes, TOTP enroll starts, TOTP confirms, recovery codes generated | Shows whether the first-run security journey is actually usable. |
| Can a returning user regain access? | login starts, login proof failures, MFA required, TOTP verifies, session created | Separates password/proof failures from MFA failures and backend faults. |
| Can users save and retrieve secrets? | vault item writes, reads, sync pulls, conflict responses, stale revision rejections | This is the product's core reliability signal after auth. |
| Is the system resisting abuse? | rate-limit hits, CSRF failures, invalid origin/fetch metadata rejections, repeated MFA failures | Tracks attack pressure without exposing account identifiers. |
| Is data durability healthy? | PostgreSQL primary health, replica lag, backup age, last successful restore drill timestamp, WAL archive failures | A password manager is not stable until saved data survives failover and restore. |

Suggested derived business SLIs after the flows exist:

- registration completion ratio: `register_finish_success / register_start_success`;
- protected activation ratio: `totp_confirm_success / register_finish_success`;
- returning access success ratio: `session_created_after_mfa / login_start_success`;
- first-secret activation ratio: `first_vault_item_created / register_finish_success`;
- vault write success ratio: `vault_write_success / vault_write_attempt`;
- sync freshness success ratio: `sync_success_without_stale_revision / sync_attempt`;
- recovery usage rate and recovery failure rate, without user labels.

These are product-health and abuse-detection signals, not vanity metrics. They should drive backlog
choices only after the underlying event counters are implemented and verified against synthetic
journeys.

## Password-Manager Reliability Scorecard

The dashboard should answer a password-manager-specific question: can a person safely get back to
their secrets when they need them? Golden Signals are the base layer, but the product scorecard must
also cover durability, cryptographic workflow health, and abuse resistance.

| Scorecard area | Good state | MVP measurement |
| --- | --- | --- |
| Access | A returning user can complete login, MFA, and vault unlock. | Login start, proof verify, MFA verify, session creation, unlock metadata fetch, and synthetic journey success. |
| Write durability | A saved secret remains available after rollout, pod restart, and database failover. | Vault write success, revision-chain continuity, PostgreSQL HA state, backup age, restore drill age, and failover drill result. |
| Sync correctness | Multi-device clients do not silently lose or overwrite item revisions. | Sync pull/push success, stale revision rejection, conflict rate, head hash continuity, and client-visible conflict responses. |
| Security posture | Abuse is visible without leaking user data. | Rate-limit hits, CSRF failures, invalid-origin rejections, MFA failures, recovery-code attempts, and lockout events. |
| Operational confidence | Operators can see the current release, dependency health, and rollback target. | Build/revision metric, rollout annotations, target health, PDB state, pod restarts, DB pool pressure, and alert delivery. |

The first useful business metric is not revenue. It is protected activation: a registration that
finishes with MFA enabled and at least one encrypted vault item saved. Until that journey works, the
business dashboard should stay focused on product readiness rather than marketing-style growth.

## Dashboard Maturity Levels

Use these levels to avoid calling a dashboard "done" when it only proves that scraping works:

| Level | Meaning | Required evidence |
| --- | --- | --- |
| L0 scrape | Targets are scraped. | `up{job="password-vault-api"}` returns expected replicas. |
| L1 Golden Signals | Basic API health is visible. | Request rate, 5xx ratio, p95/p99 latency, and pending requests return data. |
| L2 actionable alerts | A human or ticket receives useful failures. | Target-down and fast error-budget burn alerts are deployed and tested. |
| L3 product journey | Synthetic user journeys are measured. | Register, login, MFA, unlock, write, read, and sync probes publish pass/fail metrics. |
| L4 durability | Data survival is measured. | DB replication, backup age, WAL archive health, restore drill age, and failover drill results are visible. |
| L5 security/product | Aggregate abuse and activation signals are visible. | Low-cardinality auth, MFA, CSRF, rate-limit, recovery, and protected-activation metrics are implemented. |

The live preview is currently between L1 and L2, with part of L3 now started: basic Golden Signal
dashboard data exists, deployed product counters are visible, and registration/login-start journey
signals have non-zero live data. Product-specific alerts and full browser vault journey metrics are
not complete.

## Current Dashboard Gaps

- The current infrastructure dashboard is useful but still basic; it is not yet a full SLO
  dashboard.
- Grafana image rendering is unavailable, so automated screenshot/PDF-style evidence needs either a
  renderer deployment or a separate browser automation path.
- No SLO, error-budget, or burn-rate panels are implemented.
- No alert rules for target down, 5xx budget burn, latency regression, or in-flight request pressure.
- No DB pool, query latency, or DB error panels because DB metrics are not instrumented yet.
- Build/version panel is deployed and returns live `password_vault_build_info` data.
- Product auth/MFA/vault/sync panels are deployed. Registration and login-start values have live
  non-zero data; login proof, MFA, vault item, and sync panels are still zero until a fuller
  synthetic or browser journey exercises those paths.
- CSRF, rate-limit, recovery, and protected-activation security-event panels are not implemented
  yet.
- Edge access to `/metrics` is blocked in the current preview, but internal application
  `LoadBalancer` access still reaches `/metrics`; restrictive NetworkPolicy or a separate
  internal-only metrics listener is still needed.
- No dashboard check proving `/metrics` is inaccessible from the wrong network path.
- No synthetic end-to-end journey panel for register, login, MFA, unlock, and sync flows.

Minimum MVP dashboard rows:

- Golden Signals: request rate, 5xx ratio, p95/p99 latency, pending requests.
- Availability: `up{job="password-vault-api"}`, readiness success rate, target scrape freshness.
- Auth/security: login outcome rate, MFA outcome rate, rate-limit hits, CSRF failures.
- Saturation: DB pool usage, DB wait latency, auth hash active work, pod CPU/memory from platform
  metrics.
- Release context: deployed version/revision and rollout annotations.

## Alerting Priorities

Implement in this order:

1. Page: `up{job="password-vault-api"} == 0` for a sustained window.
2. Page: fast 5xx error-budget burn on product endpoints.
3. Page or urgent ticket: sustained p99 latency above the auth or product endpoint SLO with enough
   request volume.
4. Page: all replicas not ready or readiness failures causing zero serving endpoints.
5. Urgent ticket: sustained `axum_http_requests_pending` growth above baseline.
6. Urgent ticket: DB pool saturation, DB wait latency, or DB error spike once DB metrics exist.
7. Security ticket/page by severity: rate-limit bypass signal, CSRF spike, repeated TOTP failures,
   or session/token anomaly spike once security metrics exist.
8. Ticket: dashboard data missing, scrape stale, or release/version metric absent after deployment.

Use multi-window burn-rate alerts rather than single-threshold paging. For the 99.5% availability
SLO, the budget is `0.005`; example rule thresholds can compare the 5xx ratio to multiples of that
budget over short and long windows.

## MVP Acceptance Gates

Before calling the MVP observable:

- `/metrics` returns 200 and includes `axum_http_requests_total`,
  `axum_http_requests_duration_seconds_bucket`, and `axum_http_requests_pending`.
- Scraping produces `up{job="password-vault-api"} == 1` for deployed API targets.
- Dashboard has panels for request rate, 5xx ratio, p95/p99 latency, pending requests, and target
  health.
- Alert rules exist for target down and fast 5xx burn-rate.
- Metrics labels are low-cardinality and public safe; random 404 paths, login handles, account IDs,
  device IDs, item IDs, OTP values, and secrets do not appear in `/metrics`.
- Ingress or network policy blocks public access to `/metrics` when public ingress is enabled.
- k6 smoke covers `/healthz`, `/readyz`, `/metrics`, and at least one auth journey once the journey
  is implemented.
- Candidate SLO queries return data from real traffic or synthetic traffic.
- Product/security metrics are instrumented before using auth funnel or abuse dashboards as release
  gates.

Useful validation commands:

```bash
cargo test --locked --workspace metrics_records_low_cardinality_http_metrics
docker run --rm --network host \
  -v "$PWD/load/k6:/scripts:ro" \
  -w /scripts \
  -e BASE_URL=http://127.0.0.1:8080 \
  grafana/k6:2.0.0 run scenarios/health.js
helm lint deploy/helm/password-vault
helm template password-vault deploy/helm/password-vault \
  --namespace password-vault \
  --set image.tag=ci \
  --set observability.vmServiceScrape.enabled=true
```

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- Google SRE Workbook, Monitoring:
  <https://sre.google/workbook/monitoring/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
