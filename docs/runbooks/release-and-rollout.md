# Release And Rollout Runbook

Status: MVP draft.

## Scope

This runbook covers product image release, GitOps handoff, and safe rollout expectations. It does not
authorize cluster mutation by itself.

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

If the cluster cannot schedule at least one surge pod, a rollout may stall instead of taking the app
down. Treat that as safer than making unavailable pods serve traffic.

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

Browser-access check for the current home platform:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Do not use Kubernetes/LXD `LoadBalancer` addresses as the default browser URLs for LAN clients.

## Rollback

Rollback by reverting the infrastructure digest value to the previous known-good image digest and
syncing through Argo CD after human approval.

If a migration was applied, only rollback the app if the migration was backward-compatible with the
previous app version. Otherwise follow the migration-specific rollback plan.
