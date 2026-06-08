# Password Vault Helm Chart

Status: MVP chart.

The chart deploys the API service only. PostgreSQL, backup, and production secret creation remain
infrastructure responsibilities.

The chart targets Kubernetes `>=1.27`, because it renders topology spread node inclusion policies and
`matchLabelKeys` for Deployment revision-aware spreading.

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
- topology spread policies set `nodeAffinityPolicy: Honor` and `nodeTaintsPolicy: Honor` so
  tainted control-plane nodes and node affinity exclusions do not distort skew calculations.
- topology spread uses `matchLabelKeys: [pod-template-hash]` so each Deployment revision is spread
  independently during rolling updates.
- writable `/tmp` `emptyDir` while keeping the container root filesystem read-only.

The chart default keeps `whenUnsatisfiable: ScheduleAnyway` as a portable soft-spread default.
Production values can enforce one-new-ReplicaSet-pod-per-worker spreading by pairing
`whenUnsatisfiable: DoNotSchedule` with `matchLabelKeys: [pod-template-hash]` and
`nodeTaintsPolicy: Honor`.

Schema migrations are not run by app pods by default.

## Observability

When `observability.vmServiceScrape.enabled=true`, the chart emits a VictoriaMetrics
`VMServiceScrape` with stable job label `password-vault-api`.

The application exposes `/metrics` on a separate metrics listener configured by
`config.metricsBindAddr` and published through the internal-only `password-vault-api-metrics`
ClusterIP Service. The public API service should not expose `/metrics`; smoke checks should expect
HTTP 404 on the API port and HTTP 200 on the metrics port.

## Network Policy

When `networkPolicy.enabled=true`, the chart isolates API pod ingress and egress.

Ingress policy:

- HTTP ingress to the API port remains source-open by default. This is intentional for the current
  deployment shape because the edge NGINX host reaches the API through the Kubernetes
  `LoadBalancer` path rather than an in-cluster ingress controller with stable pod/namespace
  selectors.
- Metrics ingress is restricted to the configured observability namespace and `vmagent` pod
  selector. Metrics should stay on the internal ClusterIP metrics service and should not be exposed
  through the browser/API edge route.

Egress policy:

- API pods can connect to the configured PostgreSQL pod selector on TCP/5432.
- API pods can resolve DNS through kube-dns and, when enabled, NodeLocalDNS.
- There is no catch-all egress rule.

This is a hardening step, not a complete edge redesign. Moving API ingress from source-open
LoadBalancer traffic to selector-based in-cluster ingress requires a separate reviewed change to the
edge routing model.

The database selector is part of the deployment contract. The current preview PostgreSQL
`StatefulSet` and any future CloudNativePG `Cluster` must expose pod labels that match
`networkPolicy.database.podSelector`, or API-to-database traffic will be denied when the policy is
enabled.
