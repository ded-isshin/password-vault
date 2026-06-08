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
- Kubernetes documentation, "Network Policies".
- CloudNativePG documentation, "Replication".
- CloudNativePG Barman Cloud Plugin documentation, "Main Concepts".
- PostgreSQL documentation, "Versioning Policy".

## Ownership Boundaries

- Product repo owns application metric names, safe labels, `/metrics` exposure behavior, Helm scrape
  contract, load-test checks, and this SRE plan.
- Infrastructure repo owns production values, Grafana dashboards, VictoriaMetrics/Prometheus rule
  deployment, notification routing, retention, and external synthetic probes.
- `/metrics` must stay off the browser/API service port. The intended chart contract is a separate
  metrics listener and internal ClusterIP metrics service scraped by VictoriaMetrics.

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
  application as `Synced`, `Healthy`, and with the latest operation `Succeeded`.
- A live browser-equivalent edge check from the mini-PC confirmed HTTP 200 for Password Vault,
  Grafana health, the `Password Vault Overview` dashboard URL, and Argo CD `/healthz`. A MacBook
  should use the mini-PC LAN-facing address and these edge ports, not Kubernetes/LXD service IPs.
- Grafana image rendering is not installed, so dashboard PNG rendering is not available from the
  Grafana MCP path. Dashboard verification must currently use live browser access plus datasource
  query checks rather than rendered-image evidence.
- The API Deployment has three ready replicas and is pinned to an immutable GHCR image digest.
- Topology spreading is enabled. The chart supports `nodeAffinityPolicy: Honor` and
  `nodeTaintsPolicy: Honor` so production can use hard `DoNotSchedule` spreading without counting
  tainted control-plane nodes as empty topology domains. The chart also supports
  `matchLabelKeys: [pod-template-hash]` so rolling updates spread the new ReplicaSet independently
  from old pods. The chart default remains soft `ScheduleAnyway`; enforced production spreading
  requires `DoNotSchedule` plus `matchLabelKeys` in the production values.
- Read-only cluster checks showed the product database is still a single `postgres:17-bookworm`
  StatefulSet on a node-local `local-path` PVC. CloudNativePG CRDs exist, but there is no active
  product `Cluster`, `Backup`, or `ScheduledBackup`, and no CloudNativePG operator/controller was
  observed in the current cluster scan.
- A first PostgreSQL `NetworkPolicy` exists in the `password-vault` namespace.
- Low-cardinality product counters for registration, account creation, login, MFA, session
  creation/upgrades, vault item changes, sync requests, and build information are merged,
  published, deployed, and covered by a low-cardinality `/metrics` test.
- The infrastructure GitOps dashboard is deployed with panels for those product counters. Grouped
  dashboard panels use `or on() vector(0)` for fallback, so they return `0` only when no left-hand
  series exists and do not add a permanent empty zero series.
- Live verification after deployment found the dashboard and product metric series. Fresh 5-minute
  checks can legitimately return zero for registration, MFA, vault item, and sync panels when no
  synthetic or manual traffic is exercising those paths.
- A later live one-hour query returned non-zero product-event series for registration, account
  creation, login, MFA, encrypted item create, and sync. This proves the current dashboard can show
  product events when traffic exists, but it is not a replacement for a scheduled external
  synthetic probe.
- The repository includes `load/synthetic/browser-api-journey.mjs`, a dependency-free Node
  browser-API synthetic journey for
  `register -> confirm TOTP -> logout -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt`.
  It is a CI/local proof and can be run manually against the live edge route with explicit
  `SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL=true`.
- The current branch includes a `cleanup-synthetic` maintenance command for reserved-domain
  synthetic accounts. It is dry-run by default, requires `--confirm` for deletion, only accepts
  `.invalid` domains, and emits aggregate cleanup counts. It is not a replacement for a scheduled
  external probe with first-class pass/fail metrics.
- The build info panel returns `password_vault_build_info` with
  `revision="69b576558c58333e0498025364dc1e7e3aec000e"` in the current live preview check.
  CI and published images should set the `revision` label from the GitHub commit SHA through the
  `PASSWORD_VAULT_BUILD_REVISION` Rust compile-time environment variable. Local ad-hoc builds that
  do not pass the build arg can still report `revision="unknown"`.
- The infrastructure GitOps state now includes a first `NetworkPolicy` that restricts PostgreSQL
  ingress on TCP/5432 to Password Vault API pods and Argo CD migration hook pods. This is a useful
  first isolation step, but it is not a namespace-wide default deny.
- The product chart now supports an API `NetworkPolicy` that keeps browser/API HTTP ingress
  compatible with the current edge `LoadBalancer` route, restricts metrics ingress to the
  observability scraper selector, and restricts API egress to PostgreSQL plus DNS. This is not a
  full edge redesign; it is the safe isolation step available before moving traffic behind a
  selector-based in-cluster ingress path.
- The infrastructure GitOps state now includes `VMRule` alert rules named `password-vault-alerts`.
  Runtime validation showed the `password-vault.rules` group loaded in `vmalert`, with rules for
  API target loss, replica loss, 5xx ratio, p95 latency, pending requests, missing build info,
  preview PostgreSQL readiness, and migration hook failure. No `PasswordVault*` alerts were active
  at validation time.
- Alert delivery is not complete yet. The shared `VMAlertmanager` route may still use the default
  blackhole receiver until notification routing is configured in the observability stack.

Important label note:

- Application metrics use low-cardinality route labels from the Axum metrics layer.
- In the current VictoriaMetrics/Grafana path, the route label is queried as `exported_endpoint`
  because another scrape label already uses `endpoint`.
- Product docs may describe the application label as `endpoint`; production dashboards and alert
  rules must use the label name verified in the target datasource before they are accepted.

Planned:

- SLO and burn-rate panels.
- Alertmanager notification routing and a controlled test notification.
- Business, product, and security panels beyond the first deployed product counters.
- Database HA, backup, restore, and PostgreSQL health panels.
- External synthetic browser/API probes from outside the Kubernetes/LXD network.
- Live dashboard panels for synthetic pass/fail once an external probe is approved.
- Dashboard-visible synthetic cleanup and external probe outcome metrics. The maintenance command
  currently emits stdout/log aggregate counts only.

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
| `password_vault_rate_limited_requests_total` | counter | `policy`, `flow` | Confirm rate limits are absorbing abusive traffic. |
| `password_vault_session_events_total` | counter | `event`, `outcome` | Track session creation and MFA upgrade outcomes. |
| `password_vault_mfa_events_total` | counter | `event`, `outcome` | Track TOTP enrollment, login MFA, and recovery-code login outcomes. |
| `password_vault_vault_item_changes_total` | counter | `operation`, `outcome` | Track encrypted item create/update/delete success and conflict rates. |
| `password_vault_sync_requests_total` | counter | `outcome`, `page` | Track vault delta-sync success, conflict, and pagination. |
| `password_vault_db_pool_connections` | gauge | `state="idle|used|max"` | Detect pool exhaustion before request failures. |

`password_vault_db_pool_connections` is sampled when the metrics endpoint is scraped. It is useful
for visible pool pressure and dashboard context, but short spikes between scrapes require the planned
DB pool wait-duration histogram.

### Planned Technical Metrics To Add

| Metric | Type | Labels | Why |
| --- | --- | --- | --- |
| `password_vault_db_pool_wait_duration_seconds_bucket` | histogram | `operation` | Track saturation when requests wait for a DB connection. |
| `password_vault_db_query_duration_seconds_bucket` | histogram | `operation`, `outcome` | Separate DB latency from application latency. |
| `password_vault_db_errors_total` | counter | `operation`, `error_class` | Alert on DB failures without leaking SQL or values. |
| `password_vault_auth_hash_duration_seconds_bucket` | histogram | `flow`, `outcome` | Watch slow server-side auth hashing cost and DoS risk. |
| `password_vault_auth_hash_active` | gauge | none | Track concurrent expensive hash work. |
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
| `password_vault_rate_limited_requests_total` | counter | `policy`, `flow` | security | Implemented locally |
| `password_vault_db_pool_connections` | gauge | `state` | technical/saturation | Implemented locally |
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

## Daily Operating View

The main Grafana dashboard should be useful in the first minute of an incident review. Organize it
by questions an operator or product owner actually asks, not by metric namespace.

| Row | Question | Primary panels | Action when bad |
| --- | --- | --- | --- |
| User-visible availability | Can users reach the app and API from the client path? | Black-box edge probe, `up`, ready endpoints, 5xx ratio | Check edge NGINX, LoadBalancer, API readiness, Argo sync state. |
| Unlock and access | Can a returning user authenticate, pass MFA, and unlock vault metadata? | Login start/proof/MFA rates, login journey synthetic result, auth latency | Check auth errors, TOTP seed key availability, DB latency, session/CSRF failures. |
| Save and sync | Can users save an encrypted item and retrieve it later? | Vault item write success, sync request success, stale revision/conflict rate, synthetic write/read/sync result | Check revision-chain logic, DB write path, migration state, client conflict handling. |
| Durability | Will acknowledged saves survive a node/database failure? | PostgreSQL primary/replica health, replication lag, backup age, WAL archive health, restore drill age | Stop accepting real secrets if backup/replication gates fail. |
| Saturation | Are we near resource limits before users see errors? | p95/p99 latency, pending requests, DB pool wait, auth hash active work, CPU/memory, disk | Scale API, tune pool/hash cost, investigate DB/worker pressure. |
| Abuse resistance | Are attackers or broken clients distorting the system? | Rate-limit hits, CSRF failures, invalid-origin rejects, MFA failure rate, unmatched 404 rate | Review rate-limit policy, block abusive paths, check security logs without exposing user data. |
| Release context | What changed recently? | Build info, image digest, Argo revision, rollout generation, migration hook status | Compare with previous known-good digest and rollback/migration notes. |

## Product Metrics That Matter Now

Use these as MVP product-health metrics. They are intentionally aggregate-only and must not include
user, account, vault, item, email, login handle, device, IP, path, or encrypted-payload labels.

| Metric concept | Good interpretation | Bad interpretation |
| --- | --- | --- |
| Protected activation | Registration completed, MFA confirmed, first encrypted item saved. | Counting raw registrations as success before the user has any protected secret. |
| Returning access | Login proof and MFA succeed, vault metadata decrypts in the browser. | Counting `login/start` as success even if users cannot pass MFA or unlock. |
| Core write success | Encrypted item create/update/delete produces a valid revision and later sync returns it. | Counting server `200` only, without proving client can decrypt/sync the result. |
| Sync conflict rate | Low stale-revision/conflict rate under normal synthetic and manual use. | Treating all conflicts as failures; some conflicts are expected protection against overwrite. |
| Recovery readiness | Recovery-code verification and TOTP re-enrollment flow exists and is monitored. | Treating recovery-code issuance as proof that account recovery is usable. |
| Data survival | Backup/restore/failover drills are recent and successful. | Treating database pod readiness as proof that saved secrets are durable. |

For the current MVP, the north-star synthetic journey is:

```text
register -> confirm TOTP -> lock/return -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt item
```

That journey should produce one low-cardinality success/failure metric and should be run from a
client path equivalent to the browser/LAN route, not only from inside the Kubernetes namespace.

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

The live preview is currently L1 with product alert rules deployed and part of L3 instrumentation
started. Basic Golden Signal dashboard data exists and deployed product counters are visible. It is
not fully L2 yet because Alertmanager notification routing and a controlled delivered alert have not
been configured and tested. The repository now has a CI/local full browser API journey, but the live
system is not L3 until that journey or an equivalent external probe is deployed, scraped, and shown
on the dashboard.

## Current Dashboard Gaps

- The current infrastructure dashboard is useful but still basic; it is not yet a full SLO
  dashboard.
- Grafana image rendering is unavailable, so automated screenshot/PDF-style evidence needs either a
  renderer deployment or a separate browser automation path.
- No SLO, error-budget, or burn-rate panels are implemented.
- First product alert rules are deployed and loaded by `vmalert`, covering target loss, replica
  loss, 5xx ratio, latency, pending requests, build-info absence, preview PostgreSQL readiness, and
  migration hook failure.
- Alert delivery is not tested yet. Alertmanager notification routing still needs a real receiver
  and a controlled smoke alert.
- No multi-window SLO burn-rate rules or panels are implemented yet.
- DB pool connection gauges are implemented locally. Query latency and DB error panels remain planned
  because per-operation DB metrics are not instrumented yet.
- Build/version panel is deployed and returns live `password_vault_build_info` data with the product
  commit SHA for published images. Local images may report `unknown` when built without the build
  revision environment.
- Product auth/MFA/vault/sync panels are deployed. Registration and login-start values have live
  series; current 5-minute rates can be zero until a fuller synthetic or browser journey exercises
  those paths.
- CSRF, rate-limit, recovery, and protected-activation security-event panels are not implemented
  yet.
- The chart is expected to expose `/metrics` only on the internal metrics service, while the API
  service returns 404 for `/metrics`. After rollout, verify both paths explicitly.
- PostgreSQL ingress now has a first NetworkPolicy. The product chart also supports API pod
  ingress/egress isolation: HTTP ingress remains source-open for the current edge/LoadBalancer
  browser path, metrics ingress is scraper-restricted, and API egress is limited to PostgreSQL plus
  DNS. A full namespace default-deny and selector-based API ingress allow-list require an edge
  routing redesign.
- No dashboard panel proves `/metrics` is inaccessible from the wrong network path. This should be a
  synthetic/security check, not only a dashboard assumption.
- No live synthetic end-to-end journey panel for register, login, MFA, unlock, and sync flows.

## Waste-Control Rules For Observability Work

Observability work should not create panels or reports merely because a metric name exists. A new
dashboard panel, alert, or agent report should meet at least one of these tests:

- it proves or disproves an MVP acceptance gate;
- it catches a user-visible access, save, sync, durability, security, or rollout failure;
- it reduces the chance of data loss, secret exposure, or silent broken deployment;
- it creates regression evidence that CI or live checks can repeat.

Agent reports are historical evidence. The current truth should be updated in this plan, the API
contract, ADRs, and runbooks instead of spawning parallel "current state" documents.

Minimum MVP dashboard rows:

- Golden Signals: request rate, 5xx ratio, p95/p99 latency, pending requests.
- Availability: `up{job="password-vault-api"}`, readiness success rate, target scrape freshness.
- Auth/security: login outcome rate, MFA outcome rate, rate-limit hits, CSRF failures.
- Saturation: DB pool usage, DB wait latency, auth hash active work, pod CPU/memory from platform
  metrics.
- Release context: deployed version/revision and rollout annotations.

## Alerting Priorities

Implement in this order:

1. Deployed rule: `up{job="password-vault-api"} == 0` for a sustained window.
2. Deployed rule: fewer than three scrapeable `password-vault-api` targets for the current preview
   replica contract.
3. Deployed rule: sustained 5xx ratio on non-health product endpoints with enough request volume.
4. Deployed rule: sustained p95 latency above the MVP product endpoint threshold with enough
   request volume.
5. Deployed rule: sustained pending request pressure.
6. Deployed rule: missing `password_vault_build_info`.
7. Deployed rule: preview PostgreSQL not ready.
8. Deployed rule: migration hook failed.
9. Next: configure Alertmanager notification routing and send a controlled smoke alert.
10. Next: add multi-window SLO burn-rate rules on product endpoints.
11. Page or urgent ticket: sustained p99 latency above the auth or product endpoint SLO with enough
   request volume.
12. Page: all replicas not ready or readiness failures causing zero serving endpoints.
13. Urgent ticket: DB pool saturation once the DB pool dashboard and rule are deployed.
14. Security ticket/page by severity: auth rate-limit spike or repeated MFA/recovery-code failures
   once the security rules are deployed. CSRF and session/token anomaly counters remain planned.
15. Ticket: dashboard data missing, scrape stale, or release/version metric absent after deployment.

Use multi-window burn-rate alerts rather than single-threshold paging. For the 99.5% availability
SLO, the budget is `0.005`; example rule thresholds can compare the 5xx ratio to multiples of that
budget over short and long windows.

## MVP Acceptance Gates

Before calling the MVP observable:

- Internal `/metrics` returns 200 and includes `axum_http_requests_total`,
  `axum_http_requests_duration_seconds_bucket`, and `axum_http_requests_pending`.
- Browser/API-port `/metrics` returns 404 or another non-success response.
- Scraping produces `up{job="password-vault-api"} == 1` for deployed API targets.
- Dashboard has panels for request rate, 5xx ratio, p95/p99 latency, pending requests, and target
  health.
- Alert rules exist for target down and basic 5xx/latency/pending-request warnings.
- Alertmanager delivery route is configured and a controlled notification smoke test has passed.
- Multi-window burn-rate alerts exist for product endpoint SLOs.
- Metrics labels are low-cardinality and public safe; random 404 paths, login handles, account IDs,
  device IDs, item IDs, OTP values, and secrets do not appear in `/metrics`.
- Ingress or network policy blocks public access to internal metrics when public ingress is enabled.
- API egress policy has no catch-all rule and still allows PostgreSQL plus kube-dns/NodeLocalDNS.
- k6 smoke covers `/healthz`, `/readyz`, public API `/metrics` denial, internal `/metrics`, and at
  least one auth journey once the journey is implemented.
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
- Kubernetes documentation, Network Policies:
  <https://kubernetes.io/docs/concepts/services-networking/network-policies/>
- Kubernetes API reference, NetworkPolicy:
  <https://kubernetes.io/docs/reference/kubernetes-api/networking/network-policy-v1/>
- CloudNativePG documentation, Architecture:
  <https://cloudnative-pg.io/docs/1.29/architecture/>
- CloudNativePG documentation, Replication:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG documentation, Backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG documentation, Recovery:
  <https://cloudnative-pg.io/docs/1.29/recovery/>
- PostgreSQL documentation, Versioning Policy:
  <https://www.postgresql.org/support/versioning/>
