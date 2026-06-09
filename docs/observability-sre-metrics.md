# Observability And SRE Metrics Plan

Status: draft observability contract for the Password Vault MVP. Current runtime evidence was last
refreshed on 2026-06-09.

This document defines what Password Vault should measure, alert on, and show in dashboards before
the MVP is treated as operationally stable. It is not runtime evidence. Any claim that a Grafana
panel, alert route, or Kubernetes object works must be verified in the target cluster and recorded in
a release or session report.

The plan follows the Google SRE Four Golden Signals: latency, traffic, errors, and saturation.
Password Vault also needs product-specific signals because a password manager can return HTTP 200
while still failing the user journey: unable to enroll MFA, unlock a vault, save an encrypted item,
sync a revision, recover access, or survive a database failure.

## Official SRE Basis

Google SRE guidance used for this plan:

- A service dashboard should answer core service-health questions and normally include the Four
  Golden Signals.
- Latency must distinguish successful request latency from failed request latency.
- Traffic must use a high-level demand metric appropriate to the service.
- Errors include explicit failures, implicit wrong results, and policy failures.
- Saturation measures how full the most constrained resource is and should include leading
  indicators such as tail latency and impending capacity exhaustion.
- Paging should stay simple, actionable, urgent, and low-noise. Weird but non-urgent signals should
  become tickets or debugging dashboards, not pages.
- SLOs should be user-centric. For Password Vault, the meaningful user journey is not "API is up";
  it is "a user can register or return, pass MFA, unlock, save, sync, and later recover secrets."
- Multi-window, multi-burn-rate alerting is the preferred shape for defending SLOs once traffic and
  error-budget data are meaningful.

Official sources checked on 2026-06-08 and refreshed on 2026-06-09:

- Google SRE Book, "Monitoring Distributed Systems":
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, "Service Level Objectives":
  <https://sre.google/sre-book/service-level-objectives/>
- Google SRE Workbook, "Implementing SLOs":
  <https://sre.google/workbook/implementing-slos/>
- Google SRE Workbook, "Monitoring":
  <https://sre.google/workbook/monitoring/>
- Google SRE Workbook, "Alerting on SLOs":
  <https://sre.google/workbook/alerting-on-slos/>
- Google SRE resources for Service Level Objectives:
  <https://sre.google/resources/book-update/slos/>

The practical consequence for this repository is simplicity. A metric that is not used in a
dashboard, alert, SLO, synthetic check, capacity review, security review, or product-health review is
a candidate for removal or deferral. Instrumentation volume is not the goal.

## Ownership Boundaries

| Area | Product repository owns | Infrastructure repository owns |
| --- | --- | --- |
| Application metrics | Metric names, labels, low-cardinality rules, `/metrics` behavior, tests. | Scrape configuration and retention. |
| Dashboards | Dashboard requirements and PromQL examples. | Grafana dashboard deployment and live validation. |
| Alerts | Alert intent, severity policy, and runbook expectations. | `VMRule`/Alertmanager deployment, routing, delivery tests. |
| Synthetic/load | Test scripts, safe synthetic data rules, CI/manual usage docs. | Scheduled external probes, production windows, credentials/secrets, cleanup scheduling. |
| Database durability | Application-visible DB health metrics and readiness semantics. | HA PostgreSQL, backup storage, restore drills, failover drills, DB dashboards. |

Public repository safety rule: metrics must not expose private IPs, hostnames, login handles,
account IDs, vault IDs, item IDs, device IDs, request bodies, encrypted payloads, TOTP codes, raw
paths, cookies, tokens, or secrets as labels or values.

## Evidence Levels

Use these terms consistently:

| Term | Meaning |
| --- | --- |
| Implemented in code | The repository contains instrumentation or scripts that can emit the metric/check. |
| Rendered by GitOps | The repository contains chart or infrastructure objects that should create dashboards/rules. |
| Verified in runtime | A live datasource, browser, or cluster check proved the panel/rule/check returns expected data. |
| Needs verification | The expected runtime behavior has not been proven for the current deployment. |
| Planned | The metric, panel, rule, or probe does not exist yet. |

This file may describe intended dashboards and alerts, but it must not call them verified unless a
current validation command and result are available outside this document.

## Candidate SLOs

These SLOs are candidate targets until real traffic, external synthetic probes, and alert delivery
exist.

| SLO | Candidate target | Good event | Exclusions |
| --- | --- | --- | --- |
| API availability | 99.5% over 30 days | Product API request completes with status `< 500`. | `/healthz`, `/readyz`, `/metrics`, unmatched 404s. |
| Product API latency | 95% under 500 ms, 99% under 1500 ms | Non-auth product request completes under threshold. | Health, readiness, metrics, synthetic cleanup. |
| Auth latency | 95% under 2 s, 99% under 5 s | Auth endpoint completes under threshold, allowing slow password hashing. | Invalid client input and rate-limited requests should be tracked separately. |
| Protected journey | 99% of scheduled synthetic runs succeed | Register, confirm TOTP, login, unlock, create item, sync, and read/decrypt succeed. | Explicit maintenance windows and intentionally disabled probes. |
| Data durability | 100% of accepted durability drills pass | Latest backup, restore drill, WAL archive, and failover drill are fresh and successful. | None before real secrets; failed durability should block production use. |

A 99.5% monthly availability SLO has a 0.5% error budget, about 3 h 36 m over 30 days. Pages should
defend fast budget burn and user-visible symptoms. Slow burn, missing panels, and product funnel
regressions should usually create tickets unless they imply data loss, lockout, or broad outage.

## Four Golden Signals For Password Vault

| Golden signal | Password Vault interpretation | Current repository-visible state | Gaps |
| --- | --- | --- | --- |
| Latency | HTTP route latency, successful vs failed latency, auth/MFA proof latency, DB query latency, synthetic journey duration. | HTTP duration histogram exists through the Axum metrics layer. Readiness DB pool wait and `SELECT 1` query duration metrics are implemented in code. | Auth/MFA step duration, broad per-operation DB query latency, and journey duration metrics are planned. |
| Traffic | Request rate and meaningful product operation rates: registration, login, MFA, session, vault item, sync. | HTTP counters and product counters are implemented in code. The Helm chart can render a disabled-by-default full synthetic journey CronJob. | Active session gauge and deployed scheduled external synthetic traffic are planned. |
| Errors | 5xx ratio, policy failures, rate-limit hits, MFA failures, CSRF/security rejections, synthetic failures, DB errors. | HTTP status counters, login/MFA outcome counters, rate-limit counter, vault/sync outcome counters, and readiness DB error counters are implemented in code. Scheduled journey pass/fail can be observed through Kubernetes Job status once the CronJob is enabled. | CSRF/security rejection counters and custom synthetic pass/fail metrics are planned. |
| Saturation | Pending requests, DB pool pressure, DB wait, auth challenge pressure, pod CPU/memory, PostgreSQL lag/disk, backup/WAL backlog. | HTTP pending requests, DB pool connection gauges, and readiness DB pool wait histograms are implemented in code. | Auth/MFA step duration, PostgreSQL HA/backup/failover panels, and capacity alerts are planned. |

## Implemented Application Metrics

These metrics are implemented in the product code and use low-cardinality labels. Runtime scraping
still needs to be verified per deployment.

| Metric | Type | Labels | Signal | Primary use |
| --- | --- | --- | --- | --- |
| `password_vault_build_info` | gauge | `version`, `revision` | release context | Correlate incidents and dashboards with deployed code. |
| `axum_http_requests_total` | counter | route/method/status labels from the metrics layer | traffic/errors | Request rate and HTTP error ratio. |
| `axum_http_requests_duration_seconds_bucket` | histogram | route/method/status labels from the metrics layer | latency | p50/p95/p99 request latency. |
| `axum_http_requests_pending` | gauge | route/method labels from the metrics layer | saturation | In-flight request pressure. |
| `password_vault_registration_events_total` | counter | `event`, `outcome` | product traffic/errors | Registration and first-run setup events. |
| `password_vault_accounts_created_total` | counter | `outcome` | product traffic/errors | Account creation success/failure trend. |
| `password_vault_login_starts_total` | counter | `outcome` | product traffic/errors | Login challenge issuance demand. |
| `password_vault_login_attempts_total` | counter | `outcome`, `failure_class` | product/security errors | Login proof success and coarse failure classes. |
| `password_vault_rate_limited_requests_total` | counter | `policy`, `flow` | security errors/saturation | Abuse absorbed by rate limits. |
| `password_vault_session_events_total` | counter | `event`, `outcome` | product/security | Session creation and MFA upgrade outcomes. |
| `password_vault_mfa_events_total` | counter | `event`, `outcome` | security/product | TOTP enrollment, verification, recovery-code login, and re-enrollment outcomes. |
| `password_vault_vault_item_changes_total` | counter | `operation`, `outcome` | product errors/traffic | Encrypted item create/update/delete behavior. |
| `password_vault_sync_requests_total` | counter | `outcome`, `page` | product errors/traffic | Vault delta-sync success, conflict, and pagination. |
| `password_vault_db_pool_connections` | gauge | `state="idle|used|max"` | saturation | Pool pressure visible at scrape time. |
| `password_vault_db_pool_wait_duration_seconds` | histogram | `operation`, `outcome` | saturation/latency | Connection-pool wait latency for readiness DB checks. |
| `password_vault_db_query_duration_seconds` | histogram | `operation`, `outcome` | latency | Database `SELECT 1` latency for readiness DB checks. |
| `password_vault_db_errors_total` | counter | `operation`, `error_class` | errors | Low-cardinality DB readiness failures without SQL, connection strings, or values. |

Guardrails:

- Unmatched routes must collapse to a bounded label such as `/<unmatched>`.
- If the scrape pipeline renames an application label, dashboards must use the label name verified
  in the target datasource. Previous deployments have used `exported_endpoint` where a scrape label
  already consumed `endpoint`.
- DB pool connection gauges are sampled on scrape. They do not replace DB pool wait-duration
  histograms because short waits can happen between scrapes.

## Planned Technical Metrics

| Metric | Type | Labels | Why it matters |
| --- | --- | --- | --- |
| Broader `password_vault_db_pool_wait_duration_seconds` coverage | histogram | `operation`, `outcome` | Expand beyond readiness checks to user-critical DB-backed operations after we define stable operation names. |
| Broader `password_vault_db_query_duration_seconds` coverage | histogram | `operation`, `outcome` | Separates database latency from application latency for product flows, not only readiness. |
| Broader `password_vault_db_errors_total` coverage | counter | `operation`, `error_class` | Detects DB failures in product flows without leaking SQL or values. |
| `password_vault_auth_step_duration_seconds_bucket` | histogram | `step`, `outcome` | Tracks server-side auth/MFA proof verification and challenge handling latency. The expensive password KDF is browser-side in the MVP. |
| `password_vault_request_rejections_total` | counter | `reason`, `endpoint` | Tracks body-size, content-type, CSRF, origin, and validation rejections. |
| `password_vault_security_events_total` | counter | `event_class`, `severity` | Aggregated security posture without user-identifying labels. |
| `password_vault_background_job_runs_total` | counter | `job`, `outcome` | Tracks migrations, cleanup, and future maintenance jobs. |
| `password_vault_background_job_duration_seconds_bucket` | histogram | `job`, `outcome` | Detects slow or stuck operational jobs. |

## Business And Product Metrics

Business metrics for this MVP should measure whether the password manager is usable and safe. They
are not marketing vanity metrics.

| Product question | Metric concept | Good interpretation | Bad interpretation |
| --- | --- | --- | --- |
| Can a new user become protected? | Protected activation ratio | Registration finished, TOTP confirmed, recovery codes generated, first encrypted item saved. | Raw account creation counted as success before any secret is protected. |
| Can a returning user regain access? | Returning access success ratio | Login proof, MFA, session upgrade, and vault unlock all complete. | `login/start` counted as success even if MFA or unlock fails. |
| Can users save secrets safely? | Core write success ratio | Encrypted item write creates a valid revision and later sync returns it. | Server HTTP 200 counted without verifying sync/decrypt. |
| Does sync preserve data? | Sync freshness/conflict ratio | Normal sync succeeds; stale revisions are rejected and conflicts are visible. | All conflicts treated as outages even when they prevent overwrite. |
| Can users recover access? | Recovery success/failure ratio | Recovery-code login and TOTP re-enrollment work and are monitored. | Recovery-code issuance treated as proof recovery is usable. |
| Is abuse visible? | Rate-limit, MFA failure, CSRF/origin rejection rates | Attack pressure is visible without user labels. | Security logs or per-user labels exported into metrics. |
| Will saved data survive? | Backup/restore/failover freshness | Backup, WAL archive, restore drill, and failover drill are recent and successful. | Database pod readiness treated as durability proof. |

Suggested derived SLIs once the counters and synthetic probes exist:

- registration completion ratio: successful registration finish / successful registration start;
- protected activation ratio: confirmed TOTP plus first encrypted item / successful registration;
- returning access ratio: session created after MFA / login start;
- vault write success ratio: successful item writes / item write attempts;
- sync success ratio: successful sync without stale revision / sync attempts;
- recovery success ratio: successful recovery login and TOTP re-enrollment / recovery attempts;
- abuse pressure ratio: rate-limited or rejected requests / eligible flow attempts.

These ratios should be reviewed as product-health signals and release gates. They should page only
when they imply broad user lockout, data loss risk, or security incident conditions.

## Synthetic And Load Metrics

The repository contains two load/synthetic surfaces:

- k6 smoke scenarios for health, readiness, metrics scrape, registration-start, login-start, and
  mixed low-rate smoke.
- A dependency-free Node browser-API journey:
  `register -> confirm TOTP -> logout -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt -> logout -> login -> verify recovery code -> deny vault access -> re-enroll TOTP`.
- A disabled-by-default Helm `syntheticJourney` CronJob that can run the full browser-API journey
  from Kubernetes against a configured browser/edge base URL.

Synthetic and load work must follow these rules:

- Use only reserved `.invalid` login handles and fake vault data.
- Do not print account secret keys, TOTP seeds, TOTP codes, recovery codes, cookies, plaintext item
  secrets, account IDs, vault IDs, item IDs, or device IDs.
- Do not run scheduled live-edge probes until the target route, cleanup lifecycle, alert routing,
  and rate limits are explicitly configured.
- Scheduled journey pass/fail is initially observable through Kubernetes CronJob/Job status and
  kube-state-metrics. Treat that as L3-enabling evidence only after the CronJob is deployed,
  unsuspended, and verified through Grafana/VictoriaMetrics.
- Load tests should expose aggregate results: request rate, latency percentiles, failure ratio,
  synthetic journey pass/fail, step duration, and cleanup result counts.
- The synthetic account cleanup command is operational hygiene, not proof that synthetic monitoring
  works. It currently emits aggregate stdout/log counts; dashboard-visible cleanup metrics are still
  planned.

Planned synthetic metrics:

| Metric | Type | Labels | Why |
| --- | --- | --- | --- |
| `password_vault_synthetic_journey_runs_total` | counter | `journey`, `outcome` | Shows pass/fail trend for scheduled end-to-end checks. |
| `password_vault_synthetic_journey_step_duration_seconds_bucket` | histogram | `journey`, `step`, `outcome` | Identifies which step regressed. |
| `password_vault_synthetic_cleanup_runs_total` | counter | `outcome`, `dry_run` | Tracks cleanup safety and failures. |
| `password_vault_synthetic_cleanup_accounts_total` | counter | `action` | Shows matched/deleted counts without account labels. |

## Dashboard Shape

The main dashboard should be organized by questions, not metric names.

| Row | Question | Primary panels | Evidence required |
| --- | --- | --- | --- |
| User-visible availability | Can users reach the browser/API route? | External black-box probe, `up`, readiness, 5xx ratio. | Probe from client-equivalent path and datasource query result. |
| Golden Signals | Are latency, traffic, errors, and saturation healthy? | Request rate, p95/p99 latency, 5xx ratio, pending requests. | PromQL returns live data for product traffic, not only health checks. |
| Auth and unlock | Can returning users login, pass MFA, and unlock? | Login starts/attempts, MFA outcomes, session events, auth latency. | Synthetic journey or manual test generates visible metrics. |
| Save and sync | Can users save and retrieve encrypted items? | Vault item outcomes, sync outcomes, conflict/stale-revision rate. | Synthetic write/read/sync run and datasource verification. |
| Durability | Will acknowledged saves survive failure? | PostgreSQL HA state, replica lag, backup age, WAL archive health, restore drill age. | DB operator metrics plus recorded restore/failover drill. |
| Saturation/capacity | Are we close to limits? | DB pool usage/wait, auth challenge pressure, CPU/memory, disk, tail latency. | Saturation panels use implemented metrics and platform metrics. |
| Security posture | Is abuse visible? | Rate-limit hits, MFA failures, CSRF/origin rejects, recovery failures, unmatched 404s. | Low-cardinality counters exist and are scraped. |
| Release context | What changed? | Build info, image digest, Argo revision, migration/maintenance job outcome. | Current deployment revision matches expected release. |

Keep the first dashboard small enough to use during an incident. Add drill-down dashboards only when
the primary dashboard cannot answer the current question without becoming noisy. Dashboard panels
that always render zero because the feature is not implemented should stay out of the live dashboard
and remain documented as planned metrics instead.

## Dashboard Maturity Levels

| Level | Meaning | Required evidence |
| --- | --- | --- |
| L0 scrape | Targets are scraped. | `up{job="password-vault-api"}` returns expected API targets. |
| L1 Golden Signals | Basic API health is visible. | Request rate, 5xx ratio, p95/p99 latency, and pending requests return data. |
| L2 actionable alerts | Failures reach a human or ticket queue. | Target-down and fast burn alerts are deployed, routed, and smoke-tested. |
| L3 product journey | User-critical journeys are measured. | Register, MFA, login, unlock, write, read, and sync probes publish pass/fail metrics. |
| L4 durability | Data survival is measured. | Replication, backup age, WAL archive, restore drill, and failover drill are visible. |
| L5 security/product | Aggregate abuse and activation health are visible. | Auth, MFA, CSRF, rate-limit, recovery, and protected-activation metrics are implemented and verified. |

Current runtime state as of 2026-06-09 supports L0 scraping, L1 Golden Signals, candidate
multi-window availability burn alerts, product counter instrumentation, black-box internal and edge
readiness probes, scheduled browser/API synthetic journey evidence through Kubernetes Job status,
scheduled synthetic cleanup evidence through Kubernetes Job status, and an active CloudNativePG
scrape for the API database. It is not yet SRE-ready: alert delivery, trusted edge TLS,
object-store base backups, PITR restore, and failover evidence remain open. The live deployment
level must be re-evaluated after each GitOps rollout.

Verified runtime evidence from the 2026-06-08 GitOps rollout and follow-up checks:

- Grafana dashboard UID `password-vault-overview` is provisioned.
- The dashboard has 36 panels and was visible through the Grafana API. Grafana Image Renderer is not
  installed, so evidence is based on dashboard metadata and live datasource queries rather than PNG
  rendering.
- The API uses the `password-vault-cnpg` CloudNativePG application Secret, and the API Deployment has
  three ready replicas.
- CNPG dashboard panels are deployed for targets, streaming replicas, PostgreSQL version, backup
  availability, replication lag, and WAL archive failures.
- `sum(up{job="password-vault-api"}) or vector(0)` returned `3`.
- `sum(up{job="password-vault-cnpg"}) or vector(0)` returned `3`.
- `max(cnpg_pg_replication_streaming_replicas{job="password-vault-cnpg"}) or vector(0)` returned
  `2`.
- `max by (pod) (cnpg_pg_replication_lag{job="password-vault-cnpg"})` returned `0` for all three
  CNPG pods.
- `max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"}) > bool 0 or
  vector(0)` returned `0`, so backup availability remains an intentional red gate.
- `sum by (pod) (increase(cnpg_pg_stat_archiver_failed_count{job="password-vault-cnpg"}[30m]))`
  returned `0` for the current primary.
- `max(probe_success{job="password-vault-blackbox",service="password-vault",probe="internal-readyz"})
  or vector(0)` returned `1`.
- `max(probe_duration_seconds{job="password-vault-blackbox",service="password-vault",probe="internal-readyz"})
  or on() vector(0)` returned a single-digit millisecond value during the follow-up check.
- All dashboard PromQL expressions parsed and returned live data or an explicit justified zero when
  evaluated with representative `5m` rate and `6h` range windows.
- A recent live edge synthetic journey generated visible product counters. A follow-up `6h` window
  query returned registration start/finish successes, login successes, TOTP enrollment/login
  outcomes, recovery-code login verification, encrypted item create success, and sync success.
  Scheduled synthetic pass/fail metrics are still planned.
- A later 2026-06-08 runtime re-check returned:
  - `sum(up{job="password-vault-api"}) or vector(0)` = `3`;
  - `sum(up{job="password-vault-cnpg"}) or vector(0)` = `3`;
  - `max(probe_success{job="password-vault-blackbox",service="password-vault",probe="internal-readyz"})
    or vector(0)` = `1`;
  - API p95 over a `15m` histogram rate stayed in single-digit milliseconds during the checked
    one-hour window;
  - `sum(increase(password_vault_registration_events_total{job="password-vault-api"}[6h]))` = `12`;
  - `sum(increase(password_vault_login_attempts_total{job="password-vault-api"}[6h]))` = `12`;
  - `sum(increase(password_vault_vault_item_changes_total{job="password-vault-api"}[6h]))` = `6`;
  - `sum(increase(password_vault_sync_requests_total{job="password-vault-api"}[6h]))` = `12`;
  - `ALERTS{alertname=~"PasswordVault.*",alertstate="firing"}` showed
    `PasswordVaultCnpgBackupMissing`.
- A later 2026-06-08 edge and dashboard re-check returned:
  - Password Vault edge `GET /` and `/readyz` over HTTPS returned HTTP 200 from the mini-PC;
  - Grafana edge `/` and `/api/health` over HTTPS returned HTTP 200 from the mini-PC;
  - Argo CD edge `/` and `/healthz` over HTTPS returned HTTP 200 from the mini-PC;
  - that earlier check saw wildcard edge sockets before the edge-bind correction landed; the later
    2026-06-08T22:20Z check below supersedes this listener-binding evidence;
  - `max by (probe) (probe_success{job="password-vault-blackbox",service="password-vault",
    probe=~"internal-readyz|edge-readyz"})` returned `1` for both paths;
  - `max by (probe) (probe_duration_seconds{job="password-vault-blackbox",
    service="password-vault",probe=~"internal-readyz|edge-readyz"})` returned single-digit
    millisecond values for both paths;
  - `sum(rate(axum_http_requests_total{job="password-vault-api",exported_endpoint!="/metrics"}[5m]))`
    returned live traffic;
  - `histogram_quantile(0.95, sum by (le)
    (rate(axum_http_requests_duration_seconds_bucket{job="password-vault-api",
    exported_endpoint!="/metrics"}[5m])))` returned a single-digit millisecond value during the
    checked low-traffic window;
  - product counters over a 6-hour window showed successful registration, login, TOTP enrollment,
    TOTP login, recovery-code login, encrypted item creation, and sync events generated by live
    synthetic runs;
  - `max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"}) or vector(0)`
    returned `0`, so the active backup gate is still red.
- `PasswordVaultCnpgBackupMissing` is expected to be pending or firing while no available base
  backup exists. This is not noise; it is the visible real-secret-use blocker.
- The mini-PC edge route for Grafana was reachable from the mini-PC with `https` and the local
  self-signed certificate path. MacBook/browser reachability must still be verified from the client
  side before this becomes full external-access evidence.
- A later 2026-06-08T22:20Z check returned current edge and Grafana/VictoriaMetrics evidence:
  - Password Vault `GET /`, Grafana `/api/health`, and Argo CD `/healthz` returned HTTP 200 through
    the mini-PC LAN edge paths from the mini-PC.
  - The browser-facing Password Vault, Grafana, and Argo CD sockets were bound to the reviewed
    mini-PC LAN address, not wildcard sockets.
  - Dashboard UID `password-vault-overview` existed with 31 provisioned panels.
  - `sum(up{job="password-vault-api"}) or vector(0)` returned `3`.
  - `sum(up{job="password-vault-cnpg"}) or vector(0)` returned `3`.
  - `max by (probe) (probe_success{job="password-vault-blackbox",service="password-vault"})`
    returned `1` for both `internal-readyz` and `edge-readyz`.
  - The non-health API 5xx rate was `0`, and p95 request latency was about `0.005` seconds during
    the low-traffic check window.
  - `max(cnpg_pg_replication_streaming_replicas{job="password-vault-cnpg"}) or vector(0)` returned
    `2`, and `max(cnpg_pg_replication_lag{job="password-vault-cnpg"}) or vector(0)` returned `0`.
  - `max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"}) or vector(0)`
    returned `0`.
  - `ALERTS{alertname=~"PasswordVault.*",alertstate="firing"}` returned only
    `PasswordVaultCnpgBackupMissing`.
  - Six-hour product counters showed successful synthetic registration, MFA, recovery-code login,
    encrypted item creation, and sync events.
- A follow-up 2026-06-08T23:26Z session-restart check confirmed:
  - a fresh terminal session had no default `kubectl` context, so read-only cluster checks used the
    explicit production kubeconfig path;
  - Password Vault, Grafana, and Argo CD edge sockets were bound to the mini-PC LAN address, not the
    LXD bridge address and not wildcard sockets for the browser-facing ports;
  - Password Vault `GET /`, Grafana `/api/health`, and Argo CD `/healthz` returned HTTP 200 through
    the mini-PC LAN edge paths from the mini-PC;
  - Argo CD reported `prod-root` and `password-vault` as `Synced` and `Healthy`;
  - the `password-vault-cnpg` CloudNativePG cluster reported three ready instances and a healthy
    phase;
  - no `ObjectStore`, `ScheduledBackup`, or `Backup` resources existed in the `password-vault`
    namespace, so the base-backup/restore gate remained open;
  - Grafana datasource checks returned API targets `3`, CNPG targets `3`, build revision data,
    black-box `internal-readyz` and `edge-readyz` success `1`, cleanup CronJob present `1`, and
    backup availability `0`.
- A 2026-06-09 live re-check confirmed:
  - the browser-facing listeners for Password Vault, Grafana, and Argo CD are bound on the mini-PC
    LAN address, while the Kubernetes `LoadBalancer` addresses remain internal LXD/Kubernetes
    addresses;
  - Password Vault `GET /`, `/healthz`, and `/readyz` returned HTTP 200 through the mini-PC LAN edge
    path from the mini-PC; `/health` and `/ready` are not valid Password Vault routes;
  - Grafana `/api/health` and Argo CD `/healthz` returned HTTP 200 through the mini-PC LAN edge
    paths from the mini-PC;
  - the correct MacBook/browser routes are the mini-PC LAN edge URLs:
    `https://<mini-pc-lan-ip>:11443/` for Password Vault,
    `https://<mini-pc-lan-ip>:3000/` for Grafana, and `https://<mini-pc-lan-ip>:9443/` for Argo CD;
    the `<lxd-kubernetes-ip-range>` addresses are not expected to be directly reachable from the
    MacBook;
  - dashboard UID `password-vault-overview` existed with 36 provisioned panels;
  - live datasource queries returned API targets `3`, `edge-readyz=1`, `internal-readyz=1`, request
    rate about `0.47` requests/second during the check window, p95 request latency about `0.005`
    seconds, CNPG targets `3`, streaming replicas `2`, replication lag `0`, synthetic journey
    successes `2` over six hours, synthetic journey failures `0`, and backup availability `0`;
  - `max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"}) > bool 0 or
    vector(0)` still returned `0`, so backup availability remains the active durability red gate.
  - a dashboard expression validation through the Grafana datasource proxy evaluated all 38 PromQL
    targets from the 36-panel `password-vault-overview` dashboard with representative `5m` rate and
    `1h` range substitutions; the result was `failures=0`, `empty=0`.

## Current Dashboard And Alert Gaps

Do not mark these complete without runtime evidence:

- Grafana API and live PromQL verification proved the deployed CNPG dashboard queries exist and
  return data; PNG rendering is not available because the Grafana Image Renderer plugin is not
  installed.
- No current verification in this document proves Alertmanager delivers notifications.
- No SLO or error-budget dashboard is documented as verified.
- Multi-window, multi-burn-rate rules exist for the candidate API availability SLO, but alert
  delivery and low-traffic behavior still need operational proof.
- Scheduled external browser/API probes are enabled and visible through Kubernetes CronJob/Job
  status. Custom application-level synthetic pass/fail, step duration, and cleanup metrics are still
  planned.
- DB query latency, DB errors, DB pool wait, and auth/MFA step duration metrics are planned.
- Business/product panels currently use aggregate counters. Next maturity should add derived
  product SLIs for protected activation, returning access, vault write+sync success, recovery
  success, and scheduled synthetic pass/fail results.
- Use `or vector(0)` only where an explicit zero is the intended dashboard fallback. For gate
  panels, alerts, and telemetry-existence checks, missing data must remain distinguishable from a
  healthy zero.
- Low-traffic windows need special handling. A `5m` rate query can legitimately return no data when
  no requests were scraped in the range; SLO and dashboard queries should either use an appropriate
  longer window, add minimum-volume guards, or make missing data visible instead of silently
  pretending it is healthy.
- PostgreSQL HA scrape data exists for the active CloudNativePG cluster, and dashboard panels for
  targets, streaming replicas, version, backup availability, replication lag, and WAL archive
  failures are deployed. Backup availability still returns `0`; restore drill and failover drill
  evidence are still required before real secrets.
- Security panels for CSRF/origin rejection, recovery failures, and session/token anomalies are
  planned.
- A panel cannot prove `/metrics` is blocked from the wrong network path. That needs an explicit
  black-box/security check.
- Grafana image rendering may be unavailable in the environment; if so, dashboard evidence must use
  browser automation or datasource query checks instead of screenshots.

Prioritized next observability work:

1. Make alert delivery real: target-down, fast burn-rate, and durability-gate alerts must reach a
   human or ticket path in a controlled smoke test.
2. Prove MacBook/browser reachability from the client side, not only from the mini-PC edge host, and
   replace or trust the self-signed certificate model before real secrets.
3. Complete the database durability track: object-store base backups, WAL/PITR, restore drill, and
   failover drill must be observable before real secrets.
4. Expose one low-cardinality synthetic pass/fail metric only if Kubernetes Job status is not enough
   for dashboard and alerting needs; avoid per-step metric volume until it answers a real triage
   question.
5. Promote existing product counters into derived SLIs for protected activation, returning access,
   vault write+sync success, and recovery success.
6. Keep the current multi-window burn-rate alerts, but verify their low-traffic behavior with
   synthetic traffic and Alertmanager delivery.
7. Add broader DB pool wait and DB error metrics only for user-critical DB-backed operations with
   stable low-cardinality operation names.
8. Add edge exposure monitoring after the access model is decided: LAN/VPN-only reachability should
   be verified separately from service readiness.

## Alerting Policy

Paging rules must be urgent, actionable, and user-impacting. Non-urgent observability failures
should create tickets.

Recommended order:

1. Page: no scrapeable/ready API targets for a sustained short window.
2. Page: fast burn of product API availability SLO, using multi-window burn-rate rules.
3. Page: broad protected-journey synthetic failure when the external probe is scheduled and trusted.
4. Page: data durability gate failure after real secrets are allowed, including stale backups or
   failed restore/failover drill.
5. Urgent ticket: replica count below target but service still serving.
6. Urgent ticket: sustained p95/p99 latency regression with enough traffic.
7. Urgent ticket: DB pool wait, auth challenge pressure, or resource saturation approaching limits.
8. Security ticket/page by severity: rate-limit spikes, repeated MFA/recovery failures, CSRF/origin
   rejection spikes, or suspicious session events.
9. Ticket: missing build info, stale scrape, missing dashboard data, or missing synthetic cleanup.

For a 99.5% availability SLO, the budget is `0.005`. Burn-rate thresholds should compare product
endpoint error ratios to multiples of that budget over paired long/short windows. Low traffic must
be handled carefully: one failed request can look like a severe burn rate, so external synthetic
checks and minimum-volume guards are required.

## MVP Observability Acceptance Gates

Before calling the MVP observable:

- Internal `/metrics` returns 200 and includes HTTP counters, HTTP duration buckets, pending
  requests, build info, product counters, and DB pool gauges.
- Browser/API-port `/metrics` returns 404 or another non-success response.
- Scraping produces expected `up{job="password-vault-api"}` targets.
- Dashboard panels for request rate, 5xx ratio, p95/p99 latency, pending requests, target health,
  build info, DB pool usage, auth/MFA outcomes, vault item outcomes, and sync outcomes return data
  or an explicit justified zero.
- Alertmanager has a real route and a controlled notification smoke test has passed.
- Multi-window burn-rate rules exist for product endpoint SLOs.
- External synthetic journey is scheduled from a client-equivalent route, has a documented cleanup
  lifecycle, and has either reliable Kubernetes Job status panels or one low-cardinality pass/fail
  metric.
- Load tests are bounded, use fake `.invalid` accounts, and record latency/failure thresholds.
- Labels are low-cardinality and public-safe.
- PostgreSQL HA, backup, WAL archive, restore drill, and failover drill metrics are present before
  real secrets are accepted.

## Waste-Control Rules

Add a metric, panel, alert, or report only if it supports at least one of these:

- an MVP acceptance gate;
- a user-visible access, save, sync, durability, security, or rollout failure;
- a decision to page, ticket, roll back, scale, stop accepting real secrets, or investigate abuse;
- repeatable regression evidence in CI, synthetic checks, or live validation.

Remove or defer panels that are not used by an alert, release gate, debugging question, product
decision, or security review. Agent reports are historical evidence; this plan, ADRs, runbooks, and
the API contract should remain the current source of truth.

## Validation Commands

Safe local checks for this repository:

```bash
cargo test --locked --workspace metrics_records_low_cardinality_http_metrics
node --check load/synthetic/browser-api-journey.mjs
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

Runtime validation belongs in deployment/session reports and should include exact datasource queries,
dashboard URLs or screenshots when available, alert route evidence, and synthetic run IDs.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- CloudNativePG 1.29 Backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- Repository source inspected:
  `crates/api/src/telemetry.rs`, `load/README.md`,
  `load/synthetic/browser-api-journey.mjs`.
