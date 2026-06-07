# Observability And SRE Metrics Plan

Status: draft. Scope: password-vault MVP API and product-owned telemetry contract.

This plan follows the Google SRE Four Golden Signals: latency, traffic, errors, and saturation.
Product instrumentation must stay public-repository safe: no private IPs, hostnames, secrets, login
handles, account IDs, device IDs, item IDs, request bodies, encrypted payloads, OTP codes, or raw
paths as metric labels.

## Ownership Boundaries

- Product repo owns application metric names, safe labels, `/metrics` exposure behavior, Helm scrape
  contract, load-test checks, and this SRE plan.
- Infrastructure repo owns production values, Grafana dashboards, VictoriaMetrics/Prometheus rule
  deployment, notification routing, retention, and external synthetic probes.
- `/metrics` is exposed on the API service port today. If ingress is enabled, operators must block
  public access to `/metrics` or move scraping to an internal-only path/listener.

## Current Deployed State

Implemented in the current GitOps preview:

- Password Vault API is scraped with job label `password-vault-api`.
- The infrastructure repository provisions a basic Grafana dashboard named
  `Password Vault Overview`.
- The dashboard covers scrape target health, request rate, 5xx ratio, p95 request duration, pending
  requests, and unmatched 404 rate.
- The current dashboard queries have been verified against the live VictoriaMetrics datasource after
  synthetic traffic.

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

### Planned Application Metrics To Add

| Metric | Type | Labels | Why |
| --- | --- | --- | --- |
| `password_vault_build_info` | gauge | `version`, `revision` | Correlate deploys with incidents without exposing runtime hosts. |
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
| `password_vault_accounts_created_total` | counter | `outcome` | product | Planned |
| `password_vault_login_attempts_total` | counter | `outcome`, `failure_class` | product/security | Planned |
| `password_vault_active_sessions` | gauge | none | product/security | Planned |
| `password_vault_session_events_total` | counter | `event`, `outcome` | product/security | Planned |
| `password_vault_mfa_events_total` | counter | `event`, `outcome` | security | Planned |
| `password_vault_totp_verify_total` | counter | `outcome` | security | Planned |
| `password_vault_csrf_failures_total` | counter | `endpoint`, `reason` | security | Planned |
| `password_vault_security_events_total` | counter | `event_class`, `severity` | security | Planned |
| `password_vault_sync_requests_total` | counter | `operation`, `outcome` | product | Planned |
| `password_vault_sync_conflicts_total` | counter | `resource` | product | Planned |
| `password_vault_vault_item_changes_total` | counter | `operation`, `outcome` | product | Planned |

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

## Current Dashboard Gaps

- The current infrastructure dashboard is useful but still basic; it is not yet a full SLO
  dashboard.
- No SLO, error-budget, or burn-rate panels are implemented.
- No alert rules for target down, 5xx budget burn, latency regression, or in-flight request pressure.
- No DB pool, query latency, or DB error panels because DB metrics are not instrumented yet.
- No deploy/version annotation panel because `password_vault_build_info` is not implemented yet.
- No auth funnel or security-event panels because product/security metrics are not implemented yet.
- No dashboard check proving `/metrics` is inaccessible from public ingress.
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
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
