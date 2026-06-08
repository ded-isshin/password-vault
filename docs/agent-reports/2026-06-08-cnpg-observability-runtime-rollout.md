# Agent Report: CNPG Observability Runtime Rollout

Status: evidence report. Date: 2026-06-08.

## Goal

Verify browser-facing access for Grafana, Argo CD, and Password Vault; deploy Password Vault
CloudNativePG observability through GitOps; update the current PostgreSQL, SRE, migration, and
agent-workflow conclusions.

## Active Context

- Product repository: `password-vault`.
- Infrastructure repository: `infrastructure-home` through a dedicated worktree.
- Cluster access: read checks plus GitOps-driven deployment for the approved Password Vault
  observability change.
- Public safety: private LAN details and kubeconfig paths are represented with placeholders.

## Browser Access

Verified from the mini-PC:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Do not use Kubernetes/LXD addresses such as Pod IPs, ClusterIPs, or `LoadBalancer` addresses in the
cluster network as the default MacBook browser URLs. Those are not necessarily routed from the
MacBook LAN path.

## GitOps Rollout

Infrastructure PR:

- `ded-isshin/infrastructure-home#127`
- merged revision: `2703c58`

Changed infrastructure files:

- `kubernetes/gitops/prod/platform/observability/manifests/dashboards/password-vault-overview.json`
- `kubernetes/gitops/prod/apps/password-vault/vmrule.yaml`
- `kubernetes/gitops/prod/apps/password-vault/README.md`

Argo CD verification:

- `prod-root` synced and healthy at revision `2703c58`.
- `password-vault-alerts` VMRule contains the new `PasswordVaultCnpg*` rules.
- Grafana dashboard ConfigMap contains panels 16-21 for CNPG runtime state.

## Runtime Evidence

Live Grafana/VictoriaMetrics checks after rollout:

- `sum(up{job="password-vault-cnpg"}) or vector(0)` returned `3`.
- `max(cnpg_pg_replication_streaming_replicas{job="password-vault-cnpg"}) or vector(0)` returned
  `2`.
- `max by (pod) (cnpg_pg_replication_lag{job="password-vault-cnpg"})` returned `0` for all three
  CNPG pods.
- `max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"}) > bool 0 or
  vector(0)` returned `0`.
- `ALERTS{alertname=~"PasswordVaultCnpg.*",alertstate="firing"}` returned no firing alerts.
- Grafana dashboard UID `password-vault-overview` exists and contains the deployed CNPG query
  expressions.

Grafana image rendering is not installed in the current environment. Dashboard rendering evidence is
therefore based on Grafana API and datasource queries, not a rendered PNG screenshot.

## PostgreSQL HA

Clustered PostgreSQL is still required before real password-vault secrets are accepted. The current
pre-cutover CloudNativePG cluster proves that product-specific CNPG resources can run in the cluster,
but the API still needs a controlled cutover from the preview single PostgreSQL StatefulSet to the
CNPG read-write service.

There is no observed conflict with another product database. The safe boundary remains:

- product-specific CNPG `Cluster`;
- product-specific services, secrets, PVCs, users, and database names;
- product-specific backup/WAL prefix;
- no cross-product migration target.

Remaining database gates:

1. Configure backup/WAL archiving.
2. Prove restore and point-in-time recovery.
3. Run a failover drill.
4. Validate the product schema/migration job against CNPG.
5. Switch API connection to the CNPG read-write service.
6. Keep the preview PostgreSQL StatefulSet only for a short reviewed rollback window, then remove it.

## SRE Observability

Official Google SRE guidance supports the current split:

- dashboard: richer technical, product, and durability context;
- alerts: simple, actionable, low-noise signals;
- pages later: user-visible symptoms and fast SLO burn;
- tickets: pre-cutover DB warnings, missing backup, and non-urgent observability gaps.

Password Vault needs both technical and product signals:

- technical: latency, traffic, errors, saturation, DB pool pressure, CNPG lag, WAL/archive state,
  backup freshness, rollout revision;
- product/security: registration completion, TOTP/MFA outcomes, returning access success,
  encrypted item write/sync success, recovery-code usage, rate-limit and security rejection trends.

## Migration Analysis

Stable PostgreSQL versions do not remove application schema migrations. PostgreSQL engine stability
and product schema evolution solve different problems.

The policy should be:

- keep PostgreSQL on current stable supported versions;
- avoid speculative schema churn;
- keep migration files immutable after real user data exists;
- run production-like schema changes through controlled GitOps migration jobs or reviewed operator
  steps;
- keep normal API pod startup free of production migrations;
- use expand/contract changes for populated tables;
- require backup/WAL/restore evidence before destructive or high-lock migrations.

The target is few deliberate migrations, not zero migrations.

## Claude Code Usage

Purpose: independent platform/SRE review of the CNPG dashboard and alert diff.

Prompt/task given: review the current `infrastructure-home` worktree diff for blocking correctness,
GitOps, public-safety, PromQL, Argo/VictoriaMetrics, and operational-noise issues.

Summary of output:

- Claude flagged the initial replication-lag query as a likely blocking risk because the panel used a
  `pg_stat_replication` metric and `or vector(0)` could mask absence.
- Live VictoriaMetrics verification showed both the `pg_stat_replication` metric and
  `cnpg_pg_replication_lag` exist in this cluster.
- The accepted improvement was to use `cnpg_pg_replication_lag` because it is the cleaner CNPG
  cluster-level pod signal for dashboard and alerting.
- Claude also noted non-blocking polish: backup availability is expected red until backups exist,
  WAL failures should be shown as an interval increase, and target/replica alerts should handle full
  series absence.

Accepted suggestions:

- Use `cnpg_pg_replication_lag` for the lag panel and alert.
- Use `increase(cnpg_pg_stat_archiver_failed_count[...])` for the WAL failure dashboard panel.
- Make target/streaming-replica alerts return a warning series when all CNPG scrape series disappear.

Rejected or corrected suggestions:

- The claim that `cnpg_pg_stat_replication_replay_lag_seconds` does not exist was corrected by live
  datasource evidence. The query did exist, but it was still replaced with the better CNPG signal.

## Waste Reduction

What improved in this slice:

- The work was narrowed to one GitOps PR with three files.
- Claude was used once as a blocking reviewer, and its finding was verified instead of blindly
  accepted.
- Runtime claims were checked through Kubernetes, Grafana API, and live PromQL.
- Current truth was updated in the canonical observability doc; this dated report remains evidence.

Rules to keep:

- one writer per file scope;
- no parallel agents editing the same docs;
- no new report if a canonical doc update is enough;
- no dashboard or alert claim without a live query or render check;
- use issue comments for task trail, but keep durable current truth in docs.

## Validation

Validated locally before merge:

```bash
yamllint kubernetes/gitops/prod/apps/password-vault
jq empty kubernetes/gitops/prod/platform/observability/manifests/dashboards/password-vault-overview.json
find kubernetes/gitops -type f -name '*.json' -print0 | xargs -0 -n1 jq empty
git diff --check
kubectl kustomize kubernetes/gitops/prod/apps/password-vault
KUBECONFIG=<redacted-path> kubectl apply --dry-run=server -f /tmp/password-vault-app.yaml
```

Validated after merge:

```bash
KUBECONFIG=<redacted-path> kubectl -n argocd get app prod-root
KUBECONFIG=<redacted-path> kubectl -n observability get vmrule password-vault-alerts
KUBECONFIG=<redacted-path> kubectl -n observability get configmap grafana-dashboard-password-vault-overview
```

Grafana/VictoriaMetrics queries were validated through the Grafana MCP datasource.

## Sources

- Google SRE Book, "Monitoring Distributed Systems":
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, "Alerting on SLOs":
  <https://sre.google/workbook/alerting-on-slos/>
- Google SRE Workbook, "Implementing SLOs":
  <https://sre.google/workbook/implementing-slos/>
- CloudNativePG 1.29 Monitoring:
  <https://cloudnative-pg.io/docs/1.29/monitoring/>
- CloudNativePG 1.29 Replication:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG 1.29 Backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG 1.29 Recovery:
  <https://cloudnative-pg.io/docs/1.29/recovery/>
- PostgreSQL Versioning Policy:
  <https://www.postgresql.org/support/versioning/>
- PostgreSQL `ALTER TABLE`:
  <https://www.postgresql.org/docs/current/sql-altertable.html>
- PostgreSQL `CREATE INDEX`:
  <https://www.postgresql.org/docs/current/sql-createindex.html>

## Next Steps

1. Configure CNPG backup/WAL archiving to the approved object store.
2. Add restore/PITR and failover drill runbooks with actual evidence.
3. Validate schema migration job against the CNPG cluster.
4. Cut API database connection over to CNPG.
5. Add external or edge-equivalent synthetic journey metrics.
6. Add Alertmanager route and controlled notification smoke test.
7. Decide whether Grafana image rendering is worth installing for automated screenshot evidence.
