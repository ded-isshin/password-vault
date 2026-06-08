# Agent Report: DB Readiness Metrics And Runtime Access

Date: 2026-06-08

## Goal

Verify current LAN browser access for Password Vault, Grafana, and Argo CD; continue the SRE
stabilization track with a small product-code improvement that turns planned DB readiness telemetry
into implemented low-cardinality metrics.

## Active Context

- Product repository: `products/password-vault`
- Infrastructure worktree: read-only runtime checks from the current GitOps state
- Out of scope: legacy `infrastructure-home` working copy, archived repositories, direct cluster
  mutation

## Findings

- The mini-PC LAN HTTPS endpoints for Password Vault, Grafana, and Argo CD answered successfully
  from the mini-PC.
- The Kubernetes `LoadBalancer` IPs are on the LXD/Kubernetes network and are not the right browser
  target for a MacBook without a route into that network.
- The LAN browser targets should use the mini-PC LAN address and the host-level HTTPS ports:
  - Password Vault: `https://<mini-pc-lan-ip>:11443/`
  - Grafana: `https://<mini-pc-lan-ip>:3000/`
  - Argo CD: `https://<mini-pc-lan-ip>:9443/`
- Argo CD reports `prod-root` and `password-vault` as `Synced` and `Healthy`.
- Password Vault API has three ready replicas.
- The CloudNativePG cluster has three ready instances and reports a healthy state.
- No `Backup` or `ScheduledBackup` resource is present for the Password Vault namespace. This is
  still a blocker before storing real secrets.
- A legacy single-instance PostgreSQL pod/service still exists as rollback/pruning debt, but the
  application is using the CNPG application database path.

## Work Completed

- Added readiness DB metrics in product code:
  - `password_vault_db_pool_wait_duration_seconds`
  - `password_vault_db_query_duration_seconds`
  - `password_vault_db_errors_total`
- Configured explicit Prometheus buckets for the new DB duration metrics so they can be aggregated
  across API replicas with normal `_bucket`/`histogram_quantile` PromQL.
- Kept labels low-cardinality:
  - `operation="readyz_ping"`
  - `outcome="success|error"`
  - `error_class` from a fixed enum-like mapping
- Updated metric coverage tests to assert that the new metric families and labels are exposed
  without user, account, vault, or item identifiers.
- Strengthened the metric test to assert `_bucket` series for the new DB duration metrics rather
  than only matching the base metric names.
- Updated `docs/observability-sre-metrics.md` to mark readiness DB metrics as implemented while
  keeping broader per-product DB telemetry as planned.

## PostgreSQL And Migration Analysis

Clustered PostgreSQL is still required for a password manager because API pod HA without database HA
does not protect acknowledged writes. The current issue is not a conflict with another product; the
safe model is per-product isolation for namespace, CNPG cluster, application database, credentials,
PVCs, services, backup prefix, and migration target.

Stable PostgreSQL versions do not remove application schema migrations. PostgreSQL engine stability
controls the database engine; application migrations control tables, constraints, indexes, MFA
state, session state, encrypted vault metadata, and future compatibility rules.

The desired policy is:

- few deliberate migrations, not constant migrations;
- no speculative schema churn;
- immutable migration files after real user data exists;
- no startup migrations from normal API pods in real-user environments;
- controlled GitOps migration jobs for schema-changing releases;
- backup/WAL/restore evidence before risky schema changes.

## SRE Basis

The observability plan follows the Google SRE Four Golden Signals: latency, traffic, errors, and
saturation. For Password Vault, those must be supplemented by product/business signals because HTTP
200 does not prove a user can register, pass MFA, unlock, save, sync, recover, or survive a database
failure.

## Stability Backlog

Minimum blockers before real secrets:

1. Configure CNPG backup/WAL archive target and scheduled backups.
2. Run and document restore and failover drills.
3. Configure real Alertmanager delivery instead of blackhole-only routing.
4. Remove or explicitly document the legacy PostgreSQL StatefulSet rollback debt.
5. Promote product DB readiness metrics through image publish, GitOps rollout, and Grafana panel
   verification.
6. Add scheduled synthetic journey metrics after cleanup, credentials, alert routing, and rate
   limits are confirmed.

## Waste-Control Improvements

- Prefer one stabilization queue and update existing docs instead of creating duplicate reports for
  the same question.
- Do not create dashboard panels for metrics that do not exist or cannot yet produce live data.
- Do not add schema changes for organizations, plugins, mobile apps, browser extension behavior, or
  sharing until the browser MVP journey is stable.
- Use runtime evidence gates: implemented in code, rendered by GitOps, verified in runtime.
- Let external reviewers finish full reviews, then summarize accepted/rejected suggestions instead
  of restarting parallel analysis.

## Claude Code Usage

Purpose: independent observability/backend review before PR.

Prompt/task given: review the uncommitted Password Vault diff for correctness, compile risk,
low-cardinality metrics, SRE usefulness, public repository safety, and documentation overclaims.

Summary of output:

- Blocking finding: verify that `metrics_exporter_prometheus` renders the new duration metrics as
  Prometheus histograms with `_bucket` series, not summaries with per-pod quantiles.
- Non-blocking notes: public report exposes operational posture; tests should tie labels more
  tightly to metric families; query-error latency should be filtered by `outcome`.

Accepted suggestions:

- Added explicit Prometheus bucket configuration for the two DB duration metrics.
- Updated the test to assert `_bucket` series.

Rejected suggestions:

- Did not remove the public report's high-level runtime posture. It uses placeholders and no
  secrets, but future reports should keep operational weaknesses concise and issue-linked.

Reason:

- Aggregatable histograms are required for multi-replica SRE dashboards.
- The report remains useful as a durable public-safe engineering artifact.

## Commands Run

- `curl -k` health checks for Password Vault, Grafana, and Argo CD via the mini-PC LAN endpoints
- `ss -ltn` for host-level HTTPS listener checks
- `ip -brief addr` to distinguish LAN and LXD/Kubernetes networks
- `KUBECONFIG=<redacted-path> kubectl -n argocd get applications prod-root password-vault -o wide`
- `KUBECONFIG=<redacted-path> kubectl -n password-vault get deploy,pods,svc,cluster,scheduledbackup,backup -o wide`
- `KUBECONFIG=<redacted-path> kubectl -n observability get pods,svc -o wide`
- `git diff --check`
- `node --check crates/api/static/app.js`
- `node --check load/synthetic/browser-api-journey.mjs`
- `SYNTHETIC_SELF_TEST_ONLY=true node load/synthetic/browser-api-journey.mjs`
- `claude -p --effort high ...`

## Validation

Tested:

- LAN endpoint reachability from the mini-PC.
- Kubernetes/Argo/CNPG read-only status with the production kubeconfig.

Not tested yet:

- Rust compilation and unit tests for the DB readiness metric change, because the current host
  session does not have `cargo`/`rustc` in `PATH`.
- GitHub CI for this branch.
- Live Grafana panels for the new DB readiness metric families, because the product image has not
  yet been built and rolled out with this change.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- SQLx documentation for error variants and non-exhaustive error handling:
  <https://docs.rs/sqlx/latest/sqlx/enum.Error.html>
- Axum Prometheus documentation for custom Prometheus exporter buckets:
  <https://docs.rs/axum-prometheus/latest/axum_prometheus/struct.GenericMetricLayer.html>
- Metrics Exporter Prometheus documentation for `set_buckets_for_metric` behavior:
  <https://docs.rs/metrics-exporter-prometheus/latest/metrics_exporter_prometheus/struct.PrometheusBuilder.html>
