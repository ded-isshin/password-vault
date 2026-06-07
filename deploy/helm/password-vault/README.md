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

Schema migrations are not run by app pods by default. Production migrations must use an
expand/contract plan and a controlled migration job/runbook.

## Observability

When `observability.vmServiceScrape.enabled=true`, the chart emits a VictoriaMetrics
`VMServiceScrape` with stable job label `password-vault-api`.

The API exposes `/metrics` on the same service port as the public API. If `ingress.enabled=true`,
operators must block public access to `/metrics` at the ingress layer or provide an internal-only
metrics path before exposing this chart to the internet.
