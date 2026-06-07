# Session Report: Current Stabilization And SRE Review

## Goal

Answer the current stabilization questions for the deployed Password Vault preview: browser access
for Grafana and Argo CD, PostgreSQL HA posture, minimum stable MVP backlog, SRE/Golden Signals
observability, migration policy, and orchestration quality.

## Active Context

- `password-vault`: product documentation and MVP stabilization plan.
- `infrastructure-home`: read-only live checks plus a small GitOps values/PDB fix.

Repositories explicitly out of scope:

- `hiringtrace-site`
- `hiringtrace-site-archive`
- unrelated product repositories

## Verified Current State

- Password Vault, Grafana, and Argo CD are reachable through the mini-PC LAN edge ports from the
  mini-PC.
- Kubernetes `LoadBalancer` addresses are on the cluster/LXD-side network and should not be used as
  the normal MacBook browser target unless the client has routing into that network.
- Argo CD reports `password-vault` and `observability-vm-stack` as `Synced` and `Healthy`.
- Grafana contains dashboard `Password Vault Overview`.
- VictoriaMetrics datasource queries for password-vault API target health, request rate, 5xx ratio,
  and p95 latency return data.
- Password Vault API has three ready replicas.
- The preview PostgreSQL database is a single `StatefulSet` replica using node-local `local-path`
  storage. This is bootstrap/demo infrastructure, not HA.
- CloudNativePG CRDs exist in the cluster, but no active password-vault CloudNativePG cluster exists.

## Main Decisions

- Use the mini-PC LAN edge route for browsers, not the cluster/LXD `LoadBalancer` address.
- Treat the current single PostgreSQL StatefulSet as non-real-data preview infrastructure only.
- Do not share another product database. Sharing the CloudNativePG operator is acceptable; sharing a
  product database is not.
- Keep real-user use blocked until product-specific HA PostgreSQL, backup, restore, and failover
  gates pass.
- Disable startup migrations in production values and move toward an explicit migration job/runbook.
- Expand observability from basic HTTP panels to SLOs, auth/MFA, vault sync, database durability,
  backup/restore, and security aggregate metrics.

## Files Changed

Product repository:

- `docs/observability-sre-metrics.md`
- `docs/runbooks/release-and-rollout.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/mvp-implementation-plan.md`
- `docs/agent-reports/2026-06-07-current-stabilization-sre-review.md`

Infrastructure repository:

- `kubernetes/gitops/prod/apps/password-vault/values-prod.yaml`
- `kubernetes/gitops/prod/apps/password-vault/postgres-pdb.yaml`

## Claude Code Usage

Purpose: independent architecture/platform review.

Prompt/task given: review current Password Vault MVP stabilization across product and
infrastructure docs, browser access, PostgreSQL HA, migration policy, observability, and process
quality. Report only; no edits or commands.

Summary of output:

- Blocking: single-replica PostgreSQL PDB with `maxUnavailable: 0` blocks voluntary node drain
  without improving real node-failure resilience.
- Blocking: startup migrations in production values can race with rolling updates and should be
  replaced by an explicit migration job.
- Blocking: login finish and login-time TOTP verification remain the next functional MVP gate.
- Blocking: no alert rules are deployed; a synced observability stack can still be silent.
- Moderate: edge proxy configuration and `/metrics` exposure need to be codified and audited.
- Note: two API pods on one worker is expected with `ScheduleAnyway`, not necessarily a scheduler
  failure.

Accepted suggestions:

- Disable startup migrations in production values.
- Change the temporary single-replica PostgreSQL PDB to allow one voluntary unavailable pod so node
  maintenance is not blocked.
- Treat Argo CD `Synced Healthy` as deployment-state evidence, not proof of operational correctness.
- Verify dashboard label names from live datasource/dashboard data before writing alert rules.

Deferred suggestions:

- Add Argo CD PreSync migration job.
- Add VMRule alert rules and non-blackhole Alertmanager routing.
- Add edge proxy configuration as tracked infrastructure and explicitly block `/metrics` at the edge.
- Add `password_vault_build_info`.
- Implement login finish and login-time TOTP verification.
- Move to product-specific CloudNativePG cluster with backup, restore, and failover drills.

Rejected suggestions:

- None. Some items are deferred because they need a separate implementation slice and validation.

## Migration Analysis

Stable PostgreSQL versions do not remove application schema migrations. They reduce engine drift;
they do not create or evolve application-owned tables, constraints, indexes, auth fields, MFA state,
encrypted revision metadata, or compatibility windows.

The stable target is:

- supported PostgreSQL major/minor versions;
- conservative application schema changes;
- expand/contract migration policy;
- explicit migration jobs for real-user environments;
- no automatic migration from normal app pod startup;
- restore-aware rollback planning.

## Observability Analysis

The first dashboard proves data is flowing, but the product is not yet fully observable.

Minimum next observability gates:

- target-down alert;
- fast 5xx burn-rate alert;
- external synthetic probe through the same edge route a browser uses;
- auth/MFA aggregate counters;
- vault write/sync counters;
- database pool, query, replica lag, backup age, and restore-drill metrics;
- dashboard and alerts that use the live `exported_endpoint` label when querying VictoriaMetrics.

## Open Risks

- MacBook connectivity still needs client-side verification from the MacBook itself.
- Grafana anonymous viewer access may become sensitive once security dashboards are added.
- Edge proxy configuration is an operational surface and should be tracked in infrastructure code.
- The current database cannot tolerate a worker loss as a real password-vault data store.
- Login finish, login-time TOTP verification, vault CRUD, and sync remain unimplemented.

## Validation

Tested:

- Edge health checks from the mini-PC returned HTTP 200/OK for Password Vault, Grafana, and Argo CD.
- Argo CD application status showed `Synced` and `Healthy`.
- Grafana dashboard search found `Password Vault Overview`.
- VictoriaMetrics queries returned data for API target count, request rate, 5xx ratio, and p95
  latency.
- Product docs diff passed `git diff --check`.
- Public-safety grep found no private LAN/LXD/public IPs in the changed product docs.

Not tested:

- Browser access from the MacBook.
- Alert notification delivery.
- CloudNativePG failover.
- Backup/restore.
- Login finish/TOTP login flow.
- Vault CRUD/sync.

## Sources Consulted

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- PostgreSQL Versioning Policy:
  <https://www.postgresql.org/support/versioning/>
- CloudNativePG replication documentation:
  <https://cloudnative-pg.io/docs/1.29/replication/>
