# Decision Brief: MVP Stabilization, Routing, Observability, And Database Gates

Status: draft.

## Why this exists

The first deployed preview proved that the API, GitOps handoff, metrics scrape, and Grafana dashboard
can work. It also exposed the next real gates: browser reachability, HTTPS, database HA/backup,
controlled migrations, richer observability, and stricter agent workflow.

This brief separates what is verified from what remains required before Password Vault can handle
real user secrets.

## Current verified state

- The Kubernetes service, Argo CD application, and Grafana dashboard are live in the home cluster.
- Grafana has a `Password Vault Overview` dashboard using the `VictoriaMetrics` datasource.
- The dashboard currently covers basic HTTP Golden Signals: scrape target health, request rate,
  p95 latency, 5xx ratio, pending requests, and unmatched 404 rate.
- The Password Vault API is exposed through an internal/LAN Kubernetes `LoadBalancer`.
- Grafana and Argo CD already have edge NGINX publication paths from the mini-PC LAN address.
- Password Vault does not yet have a browser-friendly edge HTTPS route.
- The current database is a single-replica PostgreSQL `StatefulSet` with a single PVC.
- Product PR #49 adds TOTP enrollment/confirmation and passed CI, but is not part of this brief.

## Browser access model

The cluster `LoadBalancer` addresses are not guaranteed to be reachable from a MacBook. They live on
the cluster/LXD-side network. A browser on another device should use the edge host instead.

Current expected browser paths:

- Grafana: `https://<mini-pc-lan-ip>:3000`
- Argo CD: `https://<mini-pc-lan-ip>:9443`

Password Vault still needs one of these access paths:

1. Preferred preview path: add an edge HTTPS reverse proxy on a dedicated non-standard port, for
   example `https://<mini-pc-lan-ip>:<password-vault-edge-port>`.
2. Temporary operator path: use SSH local forwarding to the internal service.
3. Later public path: proper DNS, TLS, ingress/edge hardening, and explicit public-safety review.

Do not publish Password Vault through a broad reverse proxy until `/metrics` is either on a separate
internal listener or blocked by the edge/ingress layer.

Self-signed TLS is acceptable for operator-only Grafana/Argo preview paths, but it is not acceptable
for a password-manager user experience. A CA-valid certificate, or a deliberately trusted internal
CA for a private preview, is a precondition before real users or real secrets.

## PostgreSQL direction

The current single-replica PostgreSQL is acceptable only for preview and disposable MVP testing.
It is not acceptable for real user secrets.

There is no architectural conflict with another product database as long as each product has:

- its own namespace;
- its own database cluster or instance;
- its own secrets;
- its own backup scope;
- no shared PostgreSQL superuser credentials in app runtime.

The stable direction is CloudNativePG:

- install and manage the CloudNativePG operator through the infrastructure GitOps platform layer;
- three PostgreSQL instances;
- one primary plus two hot standbys;
- pods spread across worker nodes;
- application connects to the operator-managed read-write service;
- WAL archiving and physical base backups to object storage;
- restore drill before real user secrets;
- no public PostgreSQL exposure.

For password-manager data, synchronous replication should be the default production-like target:

```yaml
spec:
  instances: 3
  postgresql:
    synchronous:
      method: any
      number: 1
      dataDurability: required
```

This prioritizes acknowledged-write durability over write availability during degraded states. If
failure testing shows this is too disruptive, `dataDurability: preferred` can be considered only with
explicit risk acceptance.

PostgreSQL 18 is the preferred target for a new CloudNativePG-managed cluster if the selected
operator and operand image catalog support it. Do not upgrade the current single-instance runtime by
blindly changing the image tag.

Replica count does not replace backup. Three instances on node-local storage can survive a worker
failure, but they do not protect against physical host loss, operator mistakes, bad migrations, or
logical corruption. Off-node encrypted backups and restore drills are a higher priority than claiming
HA from replicas alone.

## Migration policy

Database migrations are not PostgreSQL upgrades. They are application schema versioning.

Password Vault needs migrations because the database schema is part of the security and sync
contract: accounts, sessions, MFA factors, recovery codes, audit events, vaults, immutable item
revisions, constraints, and indexes must exist consistently in dev, CI, and production.

Stable policy:

- keep migrations small and reviewable;
- use stable PostgreSQL features only;
- prefer additive/backward-compatible changes after real data exists;
- use expand/contract rollout for breaking changes;
- never rewrite already-applied migration files;
- run schema validation in CI against PostgreSQL;
- disable app startup migrations in production-like deployments;
- run migrations through a controlled GitOps migration Job or equivalent release step;
- require backup/restore readiness before risky or destructive migrations.

`runMigrationsOnStartup` remains a preview/bootstrap shortcut only.

SQLx migrations use a database-side migration ledger and locking, so the main risk is not concurrent
startup corruption. The stable-production risk is rollout coupling: a bad migration can block or
crash all new API pods during rollout. A GitOps migration Job should run first, then API pods should
start with startup migrations disabled.

## Technical observability

Use Google SRE's Golden Signals as the default dashboard frame:

- Latency: p50/p95/p99 by stable endpoint group and outcome.
- Traffic: request rate by stable endpoint group and status class.
- Errors: 5xx ratio, failed dependency operations, and policy-defined bad events.
- Saturation: pending requests, CPU/memory, database pool utilization, disk/PVC usage, and
  replication or backup lag.

Recommended next low-cardinality metrics:

- `pv_build_info{version,git_sha}`
- API ready replicas and version skew by pod.
- Database pool in-use/idle connections.
- Database acquire wait histogram.
- Database query duration/error counters by fixed operation name.
- Auth flow attempts by `flow` and `outcome`.
- CSRF rejection counts by fixed reason.
- Rate-limit hits by fixed bucket.
- TOTP verification outcomes by phase and outcome.
- Vault item create/update/delete/sync counters after vault CRUD exists.
- Sync conflict/retry counters after sync exists.

The current stack is VictoriaMetrics with `vmalert` and VMAlertmanager, not vanilla Prometheus.
Dashboard queries can use PromQL, but alert resources should be expressed as VictoriaMetrics
`VMRule` objects in the infrastructure repository.

## Product and business observability

Product metrics must be aggregate and privacy-preserving.

Useful metrics:

- registration funnel: start, finish, TOTP confirm;
- MFA enrollment completion ratio;
- active sessions aggregate;
- device registration count aggregate;
- recovery codes issued and used aggregate;
- vault create/update/delete rate;
- sync success/conflict/retry rate;
- browser unlock success/failure rate after browser crypto exists;
- time from registration start to vault-ready state.

Do not use account, email, device, vault, item, session, challenge, client IP, user-agent, secret, or
raw path values as metric labels.

## Initial SLO candidates

These are internal starter SLOs, not public promises.

- Critical API availability: 99.5% over 28 days, then tighten after baseline data.
- `/v1/session` and `/v1/csrf` p95 latency: less than 150 ms.
- Auth/write endpoint p95 latency: less than 400 ms.
- 5xx ratio: less than 0.1% over rolling 28 days for well-formed non-test traffic.
- At least two API pods ready: 99.9% after HA database is in place.
- Restore drill freshness: at least one successful restore drill in the last 30 days before real
  data is accepted.

For a password manager, `401`, `403`, and `429` are often correct security outcomes. They should be
tracked for anomaly detection, but they should not automatically burn the availability error budget.

## Alerts

Start with simple, actionable alerts:

- API scrape target down.
- Ready API replicas below two.
- Elevated 5xx error budget burn.
- p99 latency above SLO for sustained windows.
- Database pool saturation.
- Database acquire wait high.
- Database errors elevated.
- PostgreSQL primary unavailable.
- Replication lag or synchronous standby unavailable.
- WAL archive or base backup freshness missing.
- Disk/PVC fill forecast.
- TOTP failure spike.
- CSRF rejection spike.
- Rate-limit spike.
- No application traffic or no metrics when traffic is expected.
- Version skew after rollout.

Alerting is not complete until VMAlertmanager has a real notification receiver. A blackhole or
placeholder receiver is useful for testing configuration shape only; it is not operational alerting.

## Stabilization backlog

1. Add Password Vault edge HTTPS route with `/metrics` protected.
2. Merge and deploy TOTP enrollment foundation after runtime TOTP seed key is provisioned.
3. Implement login finish with TOTP/recovery verification and rate limiting.
4. Implement browser-side crypto/unlock and real app shell.
5. Implement vault item CRUD and sync.
6. Replace single PostgreSQL `StatefulSet` with CloudNativePG.
7. Add off-node encrypted WAL archiving, scheduled base backups, and restore drill.
8. Replace startup migrations with a controlled migration Job.
9. Add namespace NetworkPolicy/RBAC hardening.
10. Extend metrics, dashboards, and VMRule alerts to cover technical and product signals.
11. Extend k6 scenarios for registration/MFA funnel, soak, spike, and rollout-under-load.
12. Run a public-safety review before any broader public exposure.

The CloudNativePG CRDs may exist in a cluster from previous bootstrap, but that is not the same as a
GitOps-managed operator and product database cluster. Treat the operator installation and ownership
model as an explicit platform task.

## Anti-waste gate

Before large implementation slices, run a short throwaway risk scan:

- what already exists;
- what can be deleted or ignored;
- what blocks MVP stability;
- what artifact will survive the task;
- what evidence will prove completion.

Do not start a large agent swarm or broad refactor unless the work clearly maps to an MVP gate,
issue, ADR, validated deployment, or user-approved experiment.

## Sources

- Google SRE, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- CloudNativePG documentation:
  <https://cloudnative-pg.io/docs/1.29/>
- PostgreSQL versioning policy:
  <https://www.postgresql.org/support/versioning/>
- SQLx migration macro:
  <https://docs.rs/sqlx/latest/sqlx/macro.migrate.html>
