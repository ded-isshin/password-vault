# Password Vault Helm Chart

Status: MVP chart.

The chart deploys the API service only. PostgreSQL, backup, and production secret creation remain
infrastructure responsibilities.

Schema migrations are also an infrastructure/operator responsibility. Application pods do not run
migrations by default.

## Runtime Secrets

The chart expects Kubernetes Secrets created outside this public repository:

- `database.urlSecret.name` / `database.urlSecret.key`: PostgreSQL connection URL.
- `syntheticMetadata.keySecret.name` / `syntheticMetadata.keySecret.key`: 32-byte base64url
  `PV_SYNTHETIC_METADATA_KEY_B64` value.
- `totpSeed.keySecret.name` / `totpSeed.keySecret.key`: 32-byte base64url
  `PV_TOTP_SEED_KEY_B64` value used for application-level AEAD of server-owned TOTP seeds.

Do not commit real secret values.

## Migration Policy

Application pods do not run migrations by default. `config.runMigrationsOnStartup` should stay
`false` for production-like environments.

The chart can emit a controlled Argo CD migration hook when both `migrations.job.enabled=true` and
`migrations.job.argocdHook.enabled=true`. A non-hook Kubernetes Job is intentionally rejected by the
chart because repeated GitOps/Helm applies can fail when they try to patch an existing completed
Job's immutable pod template. Use a separate reviewed operator step if a non-Argo migration path is
needed.

The job runs the same image with:

```bash
password-vault-api migrate
```

The command requires `PV_DATABASE_URL`, applies bundled SQLx migrations, and exits. It does not start
the HTTP server and does not require the TOTP or synthetic metadata keys.

The Argo `PreSync` hook fails closed: if migration execution fails, Argo CD stops the sync before
rolling the API Deployment.

The migration hook uses `metadata.generateName` by default and deletes successful Jobs with
`HookSucceeded`. Fixed-name Kubernetes Jobs are immutable after creation and can block later Argo CD
syncs when the image digest or pod template changes. Failed hook Jobs are left for inspection.

`ttlSecondsAfterFinished` is intentionally unset by default. Enable it only if the operating model
accepts Kubernetes deleting completed migration jobs before the next Argo sync.

Production schema-changing releases must still follow an expand/contract plan, with backup/restore
evidence before destructive or contract migrations.

## Rollout Policy

Defaults are set for live rolling updates:

- 3 replicas.
- `RollingUpdate` with `maxUnavailable: 0` and `maxSurge: 1`.
- readiness probe on `/readyz`.
- graceful SIGTERM handled by the Rust service.
- short pre-stop drain before container termination.
- `PodDisruptionBudget` with `maxUnavailable: 1`.
- topology spread constraints across Kubernetes nodes.
- writable `/tmp` `emptyDir` while keeping the container root filesystem read-only.

Schema migrations are not run by app pods by default.

## Observability

When `observability.vmServiceScrape.enabled=true`, the chart emits a VictoriaMetrics
`VMServiceScrape` with stable job label `password-vault-api`.

The API exposes `/metrics` on the same service port as the public API. If `ingress.enabled=true`,
operators must block public access to `/metrics` at the ingress layer or provide an internal-only
metrics path before exposing this chart to the internet.
