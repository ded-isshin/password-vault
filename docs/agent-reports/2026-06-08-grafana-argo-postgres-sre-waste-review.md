# Agent Report: Grafana, Argo CD, PostgreSQL, SRE, And Waste Review

Status: evidence report. Date: 2026-06-08.

## Goal

Answer the current stabilization questions for the deployed Password Vault MVP preview:

- browser access to Grafana, Argo CD, and Password Vault;
- PostgreSQL HA requirements and possible conflicts with another product database;
- minimum stability and feature work for a dependable MVP;
- Google SRE Golden Signals, SLO, technical, product, and security metrics;
- whether schema migrations are still needed with stable PostgreSQL versions;
- how to reduce hallucinated or wasteful agent work.

## Active Context

- Product repository: `password-vault`.
- Infrastructure worktree: read-only checks for the Password Vault GitOps app and observability
  dashboard.
- Public repository safety: no secrets, kubeconfigs, live private logs, private IPs, or hostnames
  are recorded here. LAN addresses used in live checks are represented with placeholders.

## Verified

- Grafana responds through the mini-PC LAN-facing HTTPS edge route:
  `https://<mini-pc-lan-ip>:3000/api/health`.
- Argo CD responds through the mini-PC LAN-facing HTTPS/TCP edge route:
  `https://<mini-pc-lan-ip>:9443/healthz`.
- Password Vault responds through the mini-PC LAN-facing HTTPS edge route:
  `https://<mini-pc-lan-ip>:11443/` and `https://<mini-pc-lan-ip>:11443/v1/session`.
- The edge host is listening on all interfaces for the Grafana, Argo CD, and Password Vault preview
  ports.
- Kubernetes `LoadBalancer` addresses in the LXD/Kubernetes network are not the normal MacBook
  browser URLs unless the MacBook has routing into that internal network.
- Argo CD reports the Password Vault app as `Synced`, `Healthy`, and latest operation `Succeeded`.
- The API Deployment has three ready replicas pinned to an immutable GHCR image digest.
- The API pods are spread across three worker nodes.
- A historical fixed-name migration `Job` remains as pruning debt, but it no longer blocks the
  current Argo CD operation.
- Grafana contains the provisioned `Password Vault Overview` dashboard with 12 panels.
- VictoriaMetrics query `sum(up{job="password-vault-api"}) or vector(0)` returned `3`.
- The build info metric is present, but `revision="unknown"` is currently reported.
- Fresh 5-minute product journey rates can be zero when no synthetic or manual traffic exercises
  registration, MFA, vault writes, or sync.
- The live product database is still one `postgres:17-bookworm` StatefulSet replica with one
  `local-path` PVC.
- CloudNativePG CRDs exist, but no product `Cluster`, `Backup`, or `ScheduledBackup` resource was
  found in the `password-vault` namespace.
- No CloudNativePG operator/controller deployment was found in the live cluster scan.
- No `NetworkPolicy` resource exists in the `password-vault` namespace.
- Product CI previously used PostgreSQL 18 while the preview deployment used PostgreSQL 17. This
  report updates the workflows and docs to use `postgres:17-bookworm` for database-backed CI and
  load-smoke checks.

## Browser Access

Use the mini-PC LAN-facing edge endpoint from a MacBook:

```text
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
Password Vault: https://<mini-pc-lan-ip>:11443/
```

Do not use Kubernetes/LXD `LoadBalancer`, Pod, or ClusterIP addresses as the default MacBook browser
URL. Those addresses are cluster-internal from the perspective of a normal LAN client.

If the MacBook still cannot connect to the LAN-facing edge address, the likely causes are:

- the MacBook is not on the same LAN/VPN path as the mini-PC;
- the browser is using `http://` for ports that expect TLS;
- the mini-PC LAN address changed;
- a local network or client-isolation rule blocks MacBook-to-mini-PC traffic;
- the browser rejects the self-signed certificate before the user accepts the warning.

## PostgreSQL HA

There is no direct conflict with another product database if product isolation stays strict:

- do not reuse another product namespace, PostgreSQL service, secret, database, user, PVC, or backup
  prefix;
- do not run Password Vault migrations against another product database;
- sharing a CloudNativePG operator as platform infrastructure is acceptable;
- sharing another product's PostgreSQL instance is not acceptable.

Clustered PostgreSQL is required before accepting real password-vault secrets. The current single
StatefulSet is acceptable only as a preview/demo database. The recommended production-like direction
remains:

- CloudNativePG operator as a GitOps-managed platform component;
- product-owned `Cluster` in the `password-vault` namespace;
- three PostgreSQL instances spread across workers;
- PostgreSQL 17 for now, matching the current deployed major version;
- quorum synchronous replication with `method: any`, `number: 1`, and `dataDurability: required`;
- WAL archiving, scheduled physical base backups, PITR, restore drills, and failover drills before
  real user data.

The tradeoff is explicit: `dataDurability: required` protects acknowledged writes but can pause
writes when the required standby set is unavailable.

## SRE And Metrics

The Google SRE Golden Signals apply, but they need product-specific interpretation:

| Signal | Technical metric | Product interpretation |
| --- | --- | --- |
| Latency | HTTP p95/p99, auth latency, DB query/wait latency, auth hash duration | Users must be able to log in, pass MFA, unlock, save, and sync without unacceptable delay. |
| Traffic | RPS, registration/login/MFA/vault/sync operation rates | Demand is not one number; first-run, returning access, and vault sync are separate journeys. |
| Errors | 5xx, policy errors, auth failure class, vault conflicts, DB errors | 4xx can be expected or abusive; page on user-visible failure, saturation, or security thresholds. |
| Saturation | pending requests, DB pool pressure, auth hash active work, pod CPU/memory, DB disk/replica lag | Password-manager saturation includes expensive auth work and write durability, not only HTTP queue depth. |

The first business/product SLI should be protected activation:

```text
registration complete -> TOTP confirmed -> first encrypted item saved
```

The next required synthetic journey is:

```text
register -> TOTP -> login -> TOTP -> unlock -> create item -> sync -> read/decrypt
```

It should publish one low-cardinality success/failure metric and run both in CI and from a client
path equivalent to the LAN/browser route.

## Migration Analysis

Stable PostgreSQL versions do not remove application schema migrations.

PostgreSQL version stability controls the database engine. Password Vault still owns application
tables, constraints, indexes, auth/MFA/session fields, key wraps, and encrypted sync metadata. Those
objects must be created and evolved deliberately.

The right target is not "no migrations." The target is:

- one clean baseline while there are no real users, if the bootstrap migration history becomes noisy;
- immutable migration files after real user data exists;
- controlled Argo CD `PreSync` migration jobs for release-time schema changes;
- no startup migrations for real-user environments;
- expand/contract migrations for populated tables;
- backup/WAL status and restore plan before risky schema changes.

## Minimum Stabilization Queue

1. Full synthetic browser/API journey for register, TOTP, login, unlock, encrypted item create,
   sync, and read/decrypt.
2. NetworkPolicy or separate internal metrics listener so `/metrics` and PostgreSQL are not exposed
   more broadly than required.
3. L2 alerting: target down, fast 5xx burn-rate, all replicas not ready, and missing build metric.
4. Fix `password_vault_build_info` so the runtime reports a useful source revision instead of
   `unknown`.
5. Product-owned CloudNativePG cluster plus WAL archiving, scheduled backups, restore drill, and
   failover drill.
6. Database pool/query/error metrics and auth hash saturation metrics.
7. Security aggregate metrics for CSRF failures, rate-limit hits, recovery-code attempts, and TOTP
   failures.
8. Documentation cleanup: canonical docs hold current truth; dated agent reports remain evidence
   logs only.

## Waste Reduction

Use these rules before starting new agent work:

- define the work order: goal, active repo, write scope, forbidden scope, output artifact, timeout,
  and whether the result is blocking or advisory;
- do not run multiple agents that write the same files;
- agents may write only inside disjoint scopes or should be report-only;
- every claim needs evidence: command, file, source, or verified live query;
- current truth belongs in canonical docs, ADRs, runbooks, and issues, not in many parallel reports;
- old reports are historical evidence and must not be treated as current state without verification;
- create a new task only if it proves an MVP gate, removes a blocker, reduces data-loss/security
  risk, or adds a repeatable regression check.

## Claude Code Usage

Purpose: independent architecture/SRE/security review.

Prompt/task given: review browser connectivity, PostgreSQL HA/conflicts, next stability tasks,
Golden Signals/SLO metrics, schema migrations, and waste-control for Password Vault.

Summary of output:

- Confirmed the existing docs are mostly correct but the deployed state is still preview-only.
- Flagged the biggest blockers before real secrets: single PostgreSQL StatefulSet, no active CNPG
  operator/cluster, no backup/WAL/restore drill, no NetworkPolicy, and no L2 SLO alerting.
- Confirmed MacBook should use the mini-PC edge ports, not LXD/Kubernetes addresses.
- Confirmed there is no database conflict with another product if product isolation stays strict.
- Recommended aligning CI PostgreSQL version with deployed preview PostgreSQL.
- Recommended one canonical truth source per topic and fewer parallel agent reports.

Accepted suggestions:

- Update stale current-state docs.
- Align CI database-backed workflows with the deployed PostgreSQL 17 major version.
- Treat CNPG CRDs without an operator/cluster as not operational.
- Keep migrations, but reduce churn with a clean baseline before real users if needed.
- Treat product dashboard L1 as useful but not SRE-ready until L2 alerts and L3 synthetics exist.

Rejected or deferred suggestions:

- Renaming the infrastructure `prod` GitOps path is deferred because it is a broader platform
  repository structure decision. The current mitigation is to keep explicit "preview/no real
  secrets" labeling in product and infra docs.
- Removing CNPG CRDs is deferred; the preferred direction is to install and manage the operator
  through GitOps instead.

## Files Changed

- `.github/workflows/container.yml`
- `.github/workflows/load.yml`
- `.github/workflows/rust.yml`
- `README.md`
- `docs/architecture.md`
- `docs/development.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/mvp-implementation-plan.md`
- `docs/observability-sre-metrics.md`
- `docs/agent-reports/2026-06-08-grafana-argo-postgres-sre-waste-review.md`
- Infrastructure worktree:
  `kubernetes/gitops/prod/apps/password-vault/README.md`

## Commands And Checks Run

Representative commands, with sensitive paths and LAN details redacted:

```bash
curl -k https://<mini-pc-lan-ip>:3000/api/health
curl -k https://<mini-pc-lan-ip>:9443/healthz
curl -k https://<mini-pc-lan-ip>:11443/
curl -k https://<mini-pc-lan-ip>:11443/v1/session
KUBECONFIG=<redacted-path> kubectl -n argocd get application password-vault
KUBECONFIG=<redacted-path> kubectl -n password-vault get deploy,pods,svc,pvc,jobs -o wide
KUBECONFIG=<redacted-path> kubectl get crd
KUBECONFIG=<redacted-path> kubectl get deploy,statefulset,daemonset -A
KUBECONFIG=<redacted-path> kubectl -n password-vault get networkpolicy
ss -ltn
```

Grafana MCP checks:

- dashboard search for `Password Vault`;
- dashboard summary for `password-vault-overview`;
- Prometheus queries for `up`, request rate, p95 latency, build info, registration, MFA, vault item,
  and sync metrics.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- PostgreSQL Versioning Policy:
  <https://www.postgresql.org/support/versioning/>
- CloudNativePG 1.29 Replication:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG 1.29 Backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG 1.29 Recovery:
  <https://cloudnative-pg.io/docs/1.29/recovery/>

## Not Tested

- Direct browser access from the MacBook itself.
- A full synthetic browser/API journey.
- NetworkPolicy behavior, because it is not enabled yet.
- CloudNativePG failover or restore, because no product CNPG cluster exists yet.
- Alert delivery, because product alert rules are not deployed yet.

## Open Questions

- Which S3-compatible backup target should be used for CloudNativePG WAL/base backups?
- Should the current preview data be migrated to CNPG or discarded as demo data?
- Should the platform path currently named `prod` be renamed or guarded more strongly for preview
  apps that are not approved for real data?
