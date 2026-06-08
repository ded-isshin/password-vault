# Release And Rollout Runbook

Status: MVP draft.

## Scope

This runbook covers product image release, GitOps handoff, and safe rollout expectations. It does not
authorize cluster mutation by itself.

Current status: the browser/API MVP is deployed through GitOps, while the database is still a
preview single PostgreSQL `StatefulSet`. CloudNativePG operator foundation exists in the cluster,
but no Password Vault CloudNativePG `Cluster`, `Backup`, or `ScheduledBackup` exists yet.

## Release Artifact Flow

1. Product PR updates code, tests, Dockerfile, chart, docs, or load scripts.
2. GitHub Actions run Rust, PostgreSQL, public-safety, container smoke, Helm, and docs checks.
3. Merge to `main` triggers the container publish job.
4. The publish job builds the image on a GitHub-hosted runner, pushes to GHCR, and attaches
   provenance/SBOM attestations. The image build passes `BUILD_REVISION=<github-sha>` so
   `password_vault_build_info` reports the source revision in Grafana.
5. Infrastructure PR updates production values to the image digest.
6. Human reviews and approves the infrastructure PR.
7. Argo CD sync rolls out the new digest.

## Zero-Downtime Expectations

The chart defaults are designed for live updates:

- 3 API replicas.
- `RollingUpdate` with `maxUnavailable: 0` and `maxSurge: 1`.
- readiness probe on `/readyz`.
- startup/liveness probes on `/healthz`.
- PodDisruptionBudget with `maxUnavailable: 1`.
- graceful SIGTERM in the Rust service.
- short pre-stop drain before termination to give endpoint updates time to propagate.
- topology spread constraints with `nodeAffinityPolicy: Honor` and `nodeTaintsPolicy: Honor`.
- `matchLabelKeys: [pod-template-hash]` so the scheduler calculates spread per Deployment revision
  during rolling updates instead of letting old ReplicaSet pods distort the placement of new pods.

The chart default keeps `whenUnsatisfiable: ScheduleAnyway`, which is best-effort spreading. The
production hard-spread guarantee depends on the full pairing of `whenUnsatisfiable: DoNotSchedule`,
`matchLabelKeys: [pod-template-hash]`, and `nodeTaintsPolicy: Honor`.

On the current three-worker cluster, hard topology spreading with `DoNotSchedule` is compatible with
`maxSurge: 1` only when tainted control-plane nodes are excluded from skew calculations. Without
`nodeTaintsPolicy: Honor`, the scheduler can count tainted control-plane nodes as empty topology
domains and leave the surge pod pending. Without `matchLabelKeys: [pod-template-hash]`, old
ReplicaSet pods can distort new ReplicaSet placement and leave the final steady state uneven even
when the rollout succeeds. If the cluster still cannot schedule at least one surge pod, a rollout
may stall instead of taking the app down. Treat that as safer than making unavailable pods serve
traffic.

## Migration Policy

Production app pods should not run migrations automatically by default.

Production values must keep `PV_RUN_MIGRATIONS_ON_STARTUP=false`. Schema changes should run through
the chart-controlled migration `Job` or another reviewed operator step.

The product image supports:

```bash
password-vault-api migrate
```

The command requires `PV_DATABASE_URL`, applies bundled SQLx migrations, and exits. It does not
start the HTTP server and does not require the TOTP or synthetic metadata keys.

For Argo CD, enable the chart migration job with:

```yaml
migrations:
  job:
    enabled: true
    argocdHook:
      enabled: true
```

The chart intentionally rejects `migrations.job.enabled=true` without
`migrations.job.argocdHook.enabled=true`, because a normal fixed-name Kubernetes Job can fail on
later GitOps/Helm applies when its immutable pod template changes. Use a separate reviewed operator
step for non-Argo migration execution.

The Argo `PreSync` hook runs before the API Deployment rollout. If the migration job fails, Argo
stops the sync and does not proceed with the deployment.

The chart uses `metadata.generateName` for Argo migration hooks and deletes successful hook Jobs with
`HookSucceeded`. This avoids fixed-name Job immutability failures when a later release changes the
pod template or image digest. Failed hook Jobs are intentionally left for inspection. Leave
`ttlSecondsAfterFinished` unset for Argo-managed migration hooks unless there is a separate
evidence-retention decision.

Stable PostgreSQL versions do not remove application schema migrations. PostgreSQL stability means
the engine behavior is supported and predictable; it does not create password-vault tables,
constraints, indexes, auth fields, MFA state, or encrypted revision metadata for us. The goal is not
"no migrations." The goal is few, deliberate, backward-compatible migrations with clear rollout and
rollback behavior.

PostgreSQL schema changes are still operational changes. Some `ALTER TABLE` forms take strong locks,
scan tables, rebuild indexes, rewrite table storage, or temporarily require extra disk. Review each
real-user migration for expected lock behavior, runtime, and rollback compatibility before enabling
the GitOps migration job for that release.

Use expand/contract migrations:

1. Expand: add backward-compatible columns/tables/indexes.
2. Deploy app version compatible with old and new schema.
3. Backfill with a controlled job if needed.
4. Verify traffic and metrics.
5. Contract: remove old columns/paths in a later release only after verification.

Do not drop/rename columns in the same release that first requires the new shape.

Before a schema-changing production release, record:

- migration files to apply;
- whether the change is expand, backfill, or contract;
- expected lock behavior and rough runtime;
- latest backup/WAL status;
- rollback compatibility with the previous app image;
- validation query or application smoke that proves the new schema works;
- operator who reviewed the migration output.

## Smoke Verification

After deployment and traffic:

```bash
curl -fsS https://<redacted-domain>/healthz
curl -fsS https://<redacted-domain>/readyz
```

Verify metrics through Grafana/VictoriaMetrics:

```promql
up{job="password-vault-api"}
sum(rate(axum_http_requests_total{job="password-vault-api"}[5m]))
```

The API service should not expose `/metrics` on the browser/API port. Metrics are scraped through
the internal metrics service and port.

Browser-access check for the current home platform:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Use `https` and expect a local/self-signed certificate warning unless the edge certificate model has
been changed. Do not use Kubernetes/LXD `LoadBalancer` addresses as the default browser URLs for LAN
clients. Those addresses are internal service-routing details unless the client machine has explicit
routing into that network.

Read-only edge checks from the mini-PC:

```bash
curl -kfsS https://<mini-pc-lan-ip>:11443/healthz >/dev/null
curl -kfsS https://<mini-pc-lan-ip>:3000/api/health >/dev/null
curl -kfsS https://<mini-pc-lan-ip>:9443/healthz >/dev/null
```

`-k` is a LAN/self-signed certificate convenience for the current home edge only. Remove it when a
real trusted certificate model exists.

Do not commit concrete home-network IPs, hostnames, domains, cookies, tokens, or screenshots that
show private runtime details.

## GitOps And Data Platform Verification

Use read-only checks first. Do not use direct `kubectl apply`, `kubectl patch`, Helm installs, or
Terraform commands for normal rollout verification.

Argo CD application state:

```bash
KUBECONFIG=<redacted-path> kubectl -n argocd get app \
  prod-root data-cloudnative-pg password-vault -o wide
```

Expected state after the CloudNativePG operator foundation:

- `prod-root` is `Synced` and `Healthy`;
- `data-cloudnative-pg` is `Synced` and `Healthy`;
- `password-vault` is `Synced` and `Healthy`.

CloudNativePG operator foundation:

```bash
KUBECONFIG=<redacted-path> kubectl -n cnpg-system get deploy,pods,svc -o wide
KUBECONFIG=<redacted-path> kubectl -n observability get vmpodscrape data-cloudnative-pg-operator -o wide
```

Expected state:

- `deployment/cloudnative-pg` is `1/1`;
- the operator image matches the GitOps-pinned chart/app version;
- `VMPodScrape/data-cloudnative-pg-operator` is `operational`.

CloudNativePG must not be assumed to manage the preview database until a `Cluster` resource exists.
Verify this boundary explicitly:

```bash
KUBECONFIG=<redacted-path> kubectl get \
  clusters.postgresql.cnpg.io,backups.postgresql.cnpg.io,scheduledbackups.postgresql.cnpg.io,poolers.postgresql.cnpg.io \
  -A
```

Expected current preview state:

- no CloudNativePG `Cluster`, `Backup`, `ScheduledBackup`, or `Pooler` resources;
- `password-vault-postgres` remains a preview single `StatefulSet`;
- other products' PostgreSQL `StatefulSet`s remain separate and must not be reused by Password Vault.

VictoriaMetrics checks:

```promql
sum(up{job="password-vault-api"})
sum by (job, pod) (up{job="observability/data-cloudnative-pg-operator"})
```

Expected current scrape state:

- Password Vault API target count matches the expected API replica count;
- CloudNativePG operator scrape has `up=1`;
- CNPG controller/workqueue metrics are present before any CNPG database cluster is created.

## Backup, Restore, And Real-Data Gate

The current preview database is not production-like. Do not accept real user secrets until the
following evidence exists:

- backup object-store target is selected;
- backup credentials are provisioned outside Git as Kubernetes/runtime secrets;
- CloudNativePG Barman Cloud Plugin or another reviewed CloudNativePG-supported backup method is
  selected and documented;
- Password Vault CloudNativePG `Cluster` exists with three instances and explicit node spread;
- synchronous replication policy is reviewed for the current cluster;
- continuous WAL archiving is enabled;
- scheduled physical base backups are enabled;
- at least one scheduled backup has completed successfully;
- restore into a non-live namespace or separate `Cluster` object has been completed;
- the restored database runs the application schema;
- a controlled application smoke test succeeds against the restored database;
- observed RTO/RPO and manual steps are recorded.

Prepared sequence for the future CNPG cutover:

1. Keep the existing preview `StatefulSet` and PVC intact.
2. Add a GitOps PR for the new CloudNativePG `Cluster`; do not change the API database connection
   yet.
3. Prove the new cluster reaches healthy primary/replica state.
4. Add backup/WAL configuration and a scheduled backup.
5. Run a restore drill into a scratch target.
6. Choose the data preservation mode:
   - if preview data is disposable, initialize the new cluster from migrations only;
   - if preview data must be preserved through a full schema dump, restore the dump into an empty
     cluster and then run `password-vault-api migrate`;
   - if preview data must be preserved through data-only import, run `password-vault-api migrate`
     first and then import data only.
7. Before the final export or final incremental sync, freeze writes or put the old preview path into
   a reviewed read-only/maintenance window. Do not let accepted writes continue on the old database
   between final data copy and API cutover.
8. Run a reconciliation check that proves the new target contains the expected migrated state without
   logging secrets or user identifiers.
9. Change the API database Secret/Service reference through GitOps.
10. Roll API pods with `maxUnavailable: 0`.
11. Run browser/API synthetic verification, metrics checks, backup checks, and Argo checks.
12. Keep the old preview `StatefulSet` quarantined for a rollback window. Do not delete the old PVC
    until cutover and restore evidence are recorded.

Do not treat PostgreSQL pod readiness as backup proof. HA, backup, PITR, and restore drills are
different controls.

## Rollback

Rollback by reverting the infrastructure digest value to the previous known-good image digest and
syncing through Argo CD after human approval.

If a migration was applied, only rollback the app if the migration was backward-compatible with the
previous app version. Otherwise follow the migration-specific rollback plan.

Rollback cases:

- Image-only regression: revert the infrastructure image digest and let Argo roll the Deployment
  back.
- GitOps manifest regression: revert the infrastructure PR that introduced the manifest change and
  verify Argo returns to `Synced/Healthy`.
- Migration regression before cutover: stop the rollout, inspect the migration hook job, and keep
  serving the previous API/database path if compatibility allows.
- Database cutover regression: revert the API database reference to the previous known-good database
  only if the previous database still contains the required data and no incompatible writes were
  accepted after cutover.
- Data loss/corruption suspicion: stop accepting writes before attempting recovery, preserve logs and
  failed jobs, and recover into a separate target before replacing live traffic.

## Sources

- CloudNativePG installation and upgrade documentation:
  https://cloudnative-pg.io/docs/1.29/installation_upgrade/
- CloudNativePG backup documentation:
  https://cloudnative-pg.io/docs/1.29/backup/
- CloudNativePG recovery documentation:
  https://cloudnative-pg.io/docs/1.29/recovery/
- Barman Cloud Plugin documentation:
  https://cloudnative-pg.io/plugin-barman-cloud/docs/intro/
