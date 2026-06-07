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
   provenance/SBOM attestations.
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

The current public preview used startup migrations as a bootstrap shortcut while no real user data
was accepted. Treat that as an exception, not the target operating model. Before real-user use,
production values must keep `PV_RUN_MIGRATIONS_ON_STARTUP=false` and schema changes must run through
an explicit migration job or another reviewed operator step.

Stable PostgreSQL versions do not remove application schema migrations. PostgreSQL stability means
the engine behavior is supported and predictable; it does not create password-vault tables,
constraints, indexes, auth fields, MFA state, or encrypted revision metadata for us. The goal is not
"no migrations." The goal is few, deliberate, backward-compatible migrations with clear rollout and
rollback behavior.

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

## Rollback

Rollback by reverting the infrastructure digest value to the previous known-good image digest and
syncing through Argo CD after human approval.

If a migration was applied, only rollback the app if the migration was backward-compatible with the
previous app version. Otherwise follow the migration-specific rollback plan.
