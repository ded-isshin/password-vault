# Session Report: Rollout, Access, SRE, PostgreSQL, And Stability

Date: 2026-06-08.

Status: public-safe current-state report. This report redacts private network and kubeconfig details.

## Goal

Verify browser access to Password Vault, Grafana, and Argo CD; recover the Password Vault rollout;
validate the Grafana dashboard queries; refresh the PostgreSQL HA, migration, SRE, and waste-reduction
analysis for the MVP stabilization queue.

## Active Context

- `password-vault`: product documentation and stabilization plan.
- `infrastructure-home`: GitOps production intent and read-only cluster verification.

Explicitly out of scope:

- unrelated product repositories;
- `hiringtrace-site-archive`;
- direct manual mutation of live Kubernetes application state as a substitute for GitOps.

## Work Completed

- Confirmed that client browsers on the home LAN should use the mini-PC LAN edge routes, not
  Kubernetes or LXD-side service addresses.
- Fixed a Password Vault rollout deadlock through GitOps PRs in the infrastructure repository.
- Verified Argo CD reports Password Vault as `Synced` and `Healthy`.
- Verified the API Deployment has three ready replicas, one per worker node.
- Verified the current production rollout strategy is compatible with strict topology spreading:
  `maxUnavailable: 1`, `maxSurge: 0`.
- Verified the deployed API image remains pinned by immutable GHCR digest.
- Verified edge health/readiness responses for Password Vault, Grafana, and Argo CD.
- Verified the Grafana `Password Vault Overview` dashboard and its VictoriaMetrics queries.
- Verified CloudNativePG CRDs exist, but no active product `Cluster`, `Backup`, or
  `ScheduledBackup` resources are present.
- Verified the current Password Vault PostgreSQL remains a single preview `StatefulSet`, not HA.
- Updated the canonical MVP and observability docs with current verified deployment state.

## Files Changed

- `docs/mvp-implementation-plan.md`
- `docs/observability-sre-metrics.md`
- `docs/agent-reports/2026-06-08-rollout-sre-postgres-stability.md`

## Infrastructure GitOps Changes

Infrastructure PRs merged during this recovery/stabilization pass:

- `infrastructure-home#101`: rolled out the browser return-login image digest and fixed the
  production PromQL example label.
- `infrastructure-home#102`: required strict API pod spreading with `DoNotSchedule`.
- `infrastructure-home#103`: fixed the strict-spread rollout deadlock by setting
  `maxUnavailable: 1` and `maxSurge: 0`.

## Verified Browser Access Model

Use the mini-PC LAN edge route from a normal browser client.

Do not use Kubernetes pod IPs, ClusterIPs, or LXD/Kubernetes load balancer addresses from a normal
LAN browser unless that client has explicit routing into that network.

The current browser preview uses a self-signed certificate, so browser warnings are expected.

## Grafana And Argo Verification

Verified from the mini-PC against the LAN edge route:

- Password Vault `/healthz`: HTTP 200.
- Password Vault `/readyz`: HTTP 200.
- Password Vault browser index: HTTP 200.
- Grafana `/api/health`: HTTP 200.
- Argo CD `/healthz`: HTTP 200.

Verified in Grafana:

- datasource `VictoriaMetrics` exists and is the default Prometheus-compatible datasource;
- dashboard `Password Vault Overview` exists with six panels;
- dashboard queries return live data.

Live PromQL checks returned:

- `sum(up{job="password-vault-api"}) or vector(0)` = `3`;
- 5xx ratio = `0`;
- request-rate data for `/healthz` and `/readyz`;
- p95 request-duration data for `/healthz` and `/readyz`;
- pending requests = `0`;
- unmatched 404 rate = `0`.

## PostgreSQL HA Assessment

Clustered PostgreSQL is required before real password-vault secrets are accepted.

The current preview database is useful for development and browser/API preview only. It is not
acceptable for real password-manager data because it is one PostgreSQL pod with node-local storage,
no database-level failover target, no verified off-node backup, and no restore drill.

No real conflict with HiringTrace was found. The safe model is:

- a shared PostgreSQL operator can be platform infrastructure;
- the Password Vault database must be a separate product-owned PostgreSQL cluster;
- namespace, credentials, services, PVCs, backup object-store prefix, restore drills, and migrations
  must remain product-specific;
- Password Vault migrations must never target another product database.

The current unresolved platform gap is CloudNativePG lifecycle. CRDs exist, but active
CloudNativePG `Cluster`, `Backup`, and `ScheduledBackup` resources do not. A controller/operator
deployment was not found in the cluster scan. CRDs without a running controller do not provide HA,
failover, backup orchestration, or reconciliation.

Recommended direction:

- install or restore the CloudNativePG operator through Argo CD as platform infrastructure;
- create a product-specific three-instance Password Vault cluster;
- prefer synchronous quorum replication for real secrets, because acknowledged saved secrets should
  survive primary failure;
- add WAL archiving, scheduled base backups, restore drills, failover drills, and alerts before real
  user data.

## Migration Assessment

Stable PostgreSQL versions do not remove the need for application schema migrations.

PostgreSQL stability covers the database engine. Password Vault still owns account, MFA, session,
vault, item revision, audit, index, constraint, and compatibility changes. Without reviewed
migrations, schema changes become manual drift that cannot be reproduced from Git, CI, or restore
procedures.

Current production values correctly keep `runMigrationsOnStartup: false`. That avoids multiple API
pods racing to mutate schema during normal startup. The missing piece is a controlled migration
executor, such as a GitOps-managed Kubernetes `Job` or reviewed operator step.

Migration policy:

- use forward-only immutable migration files after merge;
- do not edit already-applied migrations;
- use expand/contract for live changes;
- keep destructive or contract steps in later releases;
- require backup/restore evidence before high-risk schema changes;
- run migrations through one controlled job, not every API pod;
- test the full schema from an empty database in CI.

The target is not "no migrations." The target is rare, reviewed, backward-compatible migrations
that support durable product changes.

## Observability Assessment

The current live dashboard is a useful L1 Golden Signals dashboard. It is not yet a complete SRE
system.

Current technical coverage:

- latency: HTTP request duration histograms;
- traffic: HTTP request rate;
- errors: 5xx ratio and unmatched 404 rate;
- saturation: pending HTTP requests only.

Missing technical observability:

- DB pool usage and wait latency;
- DB query latency and DB error classes;
- PostgreSQL primary/replica state, replication lag, WAL/archive health, backup age, disk pressure,
  connection count, and restore drill age;
- release/build info metric;
- alert delivery and Password Vault-specific alert rules;
- external synthetic journeys from the same edge path a browser uses;
- internal metrics access restriction or a separate internal-only metrics listener.

Recommended product/security metrics:

- registration start/finish and protected activation;
- login outcomes and MFA outcomes;
- session creation, expiry, and invalidation;
- rate-limit hits, CSRF failures, origin/fetch-metadata rejections;
- recovery-code attempts and successes;
- vault item writes, reads, deletes, sync pulls, stale revision rejections, and sync conflicts after
  vault CRUD exists.

These metrics must stay aggregate and low-cardinality. Do not label metrics with login handles,
account IDs, device IDs, item IDs, raw paths, OTP codes, encrypted payloads, or secrets.

## Load Testing Assessment

The repository already has an MVP k6 load-test harness using the official `grafana/k6:2.0.0` Docker
image. The current coverage is smoke-level:

- health/readiness/metrics;
- `register/start`;
- `login/start`;
- mixed low-rate smoke.

This is useful, but it is not yet representative password-manager load. Real load tests should be
expanded after vault unlock, CRUD, and sync exist. The next load-test milestone should add synthetic
no-secret journeys for register, MFA, return login, unlock metadata, item write/read/update/delete,
sync pull, conflict handling, and rate-limit behavior.

## Claude Code Usage

Purpose: independent architecture and SRE review.

Prompt/task given: review the current Password Vault product and infrastructure state for blocking
risks, PostgreSQL HA, migrations, Golden Signals and product metrics, waste/defer candidates, next
tasks, and disagreements with current decisions. Report only; no file edits.

Summary of output:

- Confirmed vault item CRUD, unlock, and sync are genuinely unimplemented.
- Confirmed the most serious blockers before real data are database HA/backup/restore, controlled
  migrations, secret custody, NetworkPolicy, and alerting.
- Confirmed there is no meaningful database conflict with HiringTrace if resources remain
  product-specific.
- Flagged CloudNativePG CRDs without an observed controller as a dangerous half-installed state.
- Recommended a GitOps migration job because production startup migrations are disabled and no
  migration Job/hook exists.
- Recommended product/security metrics beyond basic HTTP Golden Signals.
- Suggested deferring OTEL traces, WebAuthn, browser extension, organizations, KeePass import, and
  unrelated HiringTrace database migration until the core MVP is stable.

Accepted suggestions:

- Treat controlled migration execution as a blocking task before real data.
- Treat CloudNativePG controller lifecycle as a platform prerequisite, not just "CRDs exist."
- Treat NetworkPolicy/internal metrics restriction and real alert delivery as blockers.
- Keep product metrics low-cardinality and synchronized with implemented product journeys.
- Do not expand dashboards or traces before core signals and alerts are useful.

Rejected or deferred suggestions:

- Claude suggested reconsidering strict API spreading in favor of softer spreading plus surge for
  more rollout capacity. Deferred for now. The current product requirement values steady-state
  one-pod-per-worker placement and one-worker-loss tolerance; `maxUnavailable: 1`, `maxSurge: 0`
  preserves service availability during rollout while avoiding the strict-spread deadlock.
- Public TLS/cert-manager and OTEL traces are deferred until the browser MVP and data-safety gates
  are stronger.

## Commands And Checks Run

Representative commands, with private paths redacted:

```bash
KUBECONFIG=<redacted-path> kubectl -n argocd get application prod-root password-vault -o wide
KUBECONFIG=<redacted-path> kubectl -n password-vault get deploy,rs,pod,svc -o wide
KUBECONFIG=<redacted-path> kubectl -n password-vault get endpoints,endpointslice -o wide
KUBECONFIG=<redacted-path> kubectl get crd
KUBECONFIG=<redacted-path> kubectl get clusters.postgresql.cnpg.io -A
KUBECONFIG=<redacted-path> kubectl get scheduledbackups.postgresql.cnpg.io,backups.postgresql.cnpg.io -A
curl -k https://<mini-pc-lan-ip>:11443/healthz
curl -k https://<mini-pc-lan-ip>:11443/readyz
curl -k https://<mini-pc-lan-ip>:11443/
curl -k https://<mini-pc-lan-ip>:3000/api/health
curl -k https://<mini-pc-lan-ip>:9443/healthz
git diff --check
kubectl kustomize kubernetes/gitops/prod/apps/password-vault
gh pr checks 103 --watch --interval 10
claude -p --effort high ...
```

Grafana MCP was used to list datasources, find the dashboard, inspect panel queries, and execute the
dashboard PromQL queries against VictoriaMetrics.

## Sources Consulted

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- CloudNativePG documentation, current architecture:
  <https://cloudnative-pg.io/documentation/current/architecture/>
- CloudNativePG documentation, backup and recovery:
  <https://cloudnative-pg.io/docs/1.25/backup/>
- CloudNativePG documentation, recovery:
  <https://cloudnative-pg.io/docs/1.25/recovery/>
- Grafana k6 2.0 release note:
  <https://grafana.com/blog/k6-2-0-release/>
- Docker Hub `grafana/k6` image documentation:
  <https://hub.docker.com/r/grafana/k6>

## Validation

Tested:

- edge HTTP(S) responses from the mini-PC;
- Argo CD application health;
- Kubernetes deployment readiness and pod placement;
- Grafana dashboard existence and PromQL query results;
- CloudNativePG resource absence/presence by read-only Kubernetes API queries;
- infra PR #103 CI validation before merge;
- product documentation diff review.

Not tested:

- browser access from the MacBook itself;
- human TOTP login flow in a real browser after the rollout;
- Playwright visual regression;
- alert notification delivery;
- CloudNativePG failover;
- PostgreSQL backup/restore;
- vault unlock, CRUD, or sync, because those flows are not implemented yet;
- representative load against vault flows, because those flows are not implemented yet.

## Next Stabilization Tasks

1. Add a controlled GitOps migration job/runbook and verify current schema can be reproduced from
   repository migrations.
2. Install or restore CloudNativePG controller lifecycle through Argo CD.
3. Replace the preview single PostgreSQL StatefulSet with a product-specific three-instance
   CloudNativePG cluster.
4. Add WAL archiving, scheduled backups, restore drill, failover drill, and related alerts.
5. Move runtime secret custody to a reviewed encrypted or external secret workflow.
6. Add `NetworkPolicy` default deny, API/PostgreSQL restrictions, and internal-only metrics access.
7. Add Password Vault-specific alert rules and test real alert delivery.
8. Add product/security metrics for auth, MFA, sessions, rate limits, CSRF, recovery, and later
   vault operations.
9. Complete and smoke-test the browser return-login plus TOTP flow from a real browser client.
10. Implement browser vault unlock, encrypted item CRUD, sync conflict handling, and matching k6
    synthetic load scenarios.

