# Password Vault Helm Chart

Status: MVP chart.

The chart deploys the API service only. PostgreSQL, backup, and production secret creation remain
infrastructure responsibilities.

The chart targets Kubernetes `>=1.27`, because it renders topology spread node inclusion policies and
`matchLabelKeys` for Deployment revision-aware spreading.

Schema migrations are also an infrastructure/operator responsibility. Application pods do not run
migrations by default.

The chart can also emit a disabled-by-default synthetic cleanup `CronJob` for bounded live synthetic
test data. It is operational hygiene only; it does not replace scheduled end-to-end synthetic
monitoring.

The chart can emit a separate disabled-by-default full synthetic journey `CronJob`. That job runs
the same dependency-free browser/API journey used in CI against a configured base URL.

## Runtime Secrets

The chart expects Kubernetes Secrets created outside this public repository:

- `database.urlSecret.name` / `database.urlSecret.key`: PostgreSQL connection URL.
- `syntheticMetadata.keySecret.name` / `syntheticMetadata.keySecret.key`: 32-byte base64url
  `PV_SYNTHETIC_METADATA_KEY_B64` value.
- `totpSeed.keySecret.name` / `totpSeed.keySecret.key`: 32-byte base64url
  `PV_TOTP_SEED_KEY_B64` value used for application-level AEAD of server-owned TOTP seeds.

Do not commit real secret values.

## Service Publishing

The chart defaults to an internal `ClusterIP` API service. Production-like GitOps environments that
publish the API through a fixed edge proxy may set:

```yaml
service:
  type: LoadBalancer
  loadBalancerIP: <redacted-ip>
```

Only set `loadBalancerIP` when the cluster load-balancer implementation supports a reserved address
and the edge proxy upstream is managed from the same reviewed infrastructure intent. Leaving the
field empty keeps the default portable chart behavior.

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

## Synthetic Cleanup

The API image includes a maintenance command:

```bash
password-vault-api cleanup-synthetic --dry-run
password-vault-api cleanup-synthetic --confirm
```

The chart can schedule it with `syntheticCleanup.cronJob.enabled=true`. The default mode is dry-run:
`syntheticCleanup.cronJob.confirm=false` renders `--dry-run` and deletes nothing.

The cleanup job requires only `PV_DATABASE_URL`. It does not require TOTP seed or synthetic metadata
keys. It passes the cleanup bounds through environment variables:

- `PV_SYNTHETIC_CLEANUP_PREFIX`
- `PV_SYNTHETIC_CLEANUP_DOMAIN`
- `PV_SYNTHETIC_CLEANUP_MIN_AGE_HOURS`
- `PV_SYNTHETIC_CLEANUP_MAX_DELETE`

The application enforces a reserved `.invalid` domain, a minimum age floor, and a bounded maximum
delete count. Production-like values should still run dry-run first and inspect aggregate logs before
setting `confirm=true`.

Kubernetes CronJob defaults:

- `concurrencyPolicy: Forbid` prevents overlapping cleanup jobs;
- `startingDeadlineSeconds` limits stale missed starts;
- job history limits avoid unbounded completed Job objects;
- `automountServiceAccountToken: false` keeps the pod from receiving a Kubernetes API token.

When `networkPolicy.enabled=true`, the chart also emits an egress-only `NetworkPolicy` for the
cleanup pod. It permits only PostgreSQL and DNS egress from `component=synthetic-cleanup`. The
database-side ingress policy still belongs to the infrastructure repository and must explicitly
allow that component.

Do not point cleanup at real user domains. Do not treat cleanup logs as synthetic monitoring proof:
scheduled synthetic pass/fail metrics are a separate acceptance gate.

## Synthetic Journey

The chart can schedule the full protected-user browser/API synthetic journey with
`syntheticJourney.cronJob.enabled=true`.

The job runs the pinned Node image from `syntheticJourney.cronJob.nodeImage` and executes the
vendored `browser-api-journey.mjs` script from a read-only ConfigMap. The journey exercises:

- health and readiness;
- registration start/finish;
- browser-side key derivation;
- TOTP enrollment and confirmation;
- logout and return login;
- login-time TOTP verification;
- vault unlock;
- encrypted item create;
- sync/read/decrypt;
- recovery-code login;
- forced TOTP re-enrollment.

Required production-like value:

```yaml
syntheticJourney:
  cronJob:
    enabled: true
    baseUrl: https://<mini-pc-lan-ip>:11443
    allowNonLocalBaseUrl: true
    tlsInsecure: true
    checkMetrics: false
```

`baseUrl` is required when the CronJob is enabled. Prefer the same LAN/edge URL that a browser uses
so the synthetic detects edge routing failures, not only in-cluster service health. Keep real LAN
addresses out of public product documentation and use placeholders unless disclosure is explicitly
approved.

The script refuses non-local base URLs unless `allowNonLocalBaseUrl=true`, rejects non-`.invalid`
synthetic email domains, prints no account secret keys, TOTP seeds, OTP codes, recovery codes,
cookies, item IDs, vault IDs, or plaintext item passwords, and uses reserved synthetic accounts.

When `networkPolicy.enabled=true`, the chart can also emit an egress-only policy for
`component=synthetic-journey`. By default it allows the synthetic pod to reach API pods on the chart
service port plus DNS. For edge-route checks, production values should add the reviewed edge
destination as an explicit `syntheticJourney.networkPolicy.externalEgress` CIDR and port list. The
schema accepts only narrow IPv4 CIDRs; use `/32` for a single edge host. Broad egress such as
`0.0.0.0/0` is intentionally rejected.

The first scheduled-journey implementation exposes pass/fail through Kubernetes CronJob/Job status
and kube-state-metrics. It is useful operational evidence, but it is not yet a custom
low-cardinality product metric such as `password_vault_synthetic_journey_last_success_timestamp`.
Add that metric only with a reviewed persistence/export path that does not expose secrets.

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

Use `networkPolicy.database.podSelectorOverride` when the production values must replace the default
database selector entirely. This is required when switching from the chart's default preview
PostgreSQL `StatefulSet` labels to CloudNativePG pod labels, because Helm merges nested values maps
instead of replacing `matchLabels` key-by-key.

Do not set `podSelectorOverride.matchLabels` to an empty map. An empty pod selector can match every
pod in the namespace and broaden API database egress instead of restricting it.
