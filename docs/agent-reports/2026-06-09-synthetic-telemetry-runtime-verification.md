# Session Report: Synthetic Telemetry Runtime Verification

## Goal

Deploy the Password Vault API build that separates synthetic product telemetry from user traffic,
then verify the split in the live Kubernetes/GitOps environment.

## Active Context

- Product repository: `password-vault`
- Infrastructure repository: `infrastructure-home`
- Cluster access: production kubeconfig path used explicitly
- Out of scope: Terraform, LXD, database schema changes, GitHub repository settings

## Work Completed

- Reopened product issue #111 after GitHub auto-closed it before runtime verification.
- Promoted image digest
  `sha256:728ffd8164f9dc0e0a7ebe06f4f48ed4544a985e8d114e49d3e022dba99ede38`
  through infrastructure GitOps PR #167.
- Waited for Argo CD and the API Deployment to become `Synced` and `Healthy`.
- Ran one live synthetic journey Job:
  `password-vault-synthetic-journey-trafficclass-011524`.
- Verified product counters in VictoriaMetrics with `traffic_class="synthetic"`.

## Runtime Evidence

- Password Vault edge `GET /` returned HTTP 200 through the mini-PC LAN path.
- Password Vault edge `/readyz` returned HTTP 200 through the mini-PC LAN path.
- Grafana `/api/health` returned HTTP 200 through the mini-PC LAN path.
- Argo CD `/healthz` returned HTTP 200 through the mini-PC LAN path.
- Argo CD reported `prod-root` and `password-vault` as `Synced` and `Healthy`.
- API Deployment reported image digest
  `sha256:728ffd8164f9dc0e0a7ebe06f4f48ed4544a985e8d114e49d3e022dba99ede38`
  and `3/3` ready replicas after rollout.
- VictoriaMetrics returned `sum(up{job="password-vault-api"}) = 3`.
- VictoriaMetrics returned synthetic product counters over a 30-minute window:
  - registration events: `2`
  - login successes: `2`
  - encrypted item changes: `1`
  - TOTP enrollment events: `4`
  - TOTP login events: `3`
  - recovery-code login verification: `1`

## Validation Commands

```bash
git diff --check
gh pr checks 167 --watch --interval 10
KUBECONFIG=<redacted-path> kubectl -n argocd get application prod-root password-vault
KUBECONFIG=<redacted-path> kubectl -n password-vault wait --for=condition=complete job/password-vault-synthetic-journey-trafficclass-011524
```

Grafana/VictoriaMetrics queries:

```promql
sum(up{job="password-vault-api"})
sum(increase(password_vault_registration_events_total{job="password-vault-api",traffic_class="synthetic"}[30m]))
sum by (traffic_class,outcome) (increase(password_vault_login_attempts_total{job="password-vault-api",traffic_class="synthetic"}[30m]))
sum by (traffic_class,event,outcome) (increase(password_vault_vault_item_changes_total{job="password-vault-api",traffic_class="synthetic"}[30m]))
sum by (traffic_class,event,outcome) (increase(password_vault_mfa_events_total{job="password-vault-api",traffic_class="synthetic"}[30m]))
```

## Observations

- The public/API port intentionally returns HTTP 404 for `/metrics`; live metrics are exposed through
  the internal metrics listener and ClusterIP service.
- Short query windows can still include legacy no-`traffic_class` time series from pods that ran
  before the split. Dashboards must filter on `traffic_class` explicitly when comparing synthetic
  and user traffic.
- The correct browser paths from a client on the home network are the mini-PC LAN edge URLs, not the
  LXD/Kubernetes LoadBalancer IP addresses.

## Remaining Real-Secret Gates

- Configure and verify CloudNativePG backup, WAL archiving, PITR, and restore drill.
- Prove alert delivery to a real receiver.
- Replace or trust the self-signed edge TLS model before real secret use.
- Harden browser/API HTTP ingress NetworkPolicy before real secret use.
- Clean up legacy preview PVC/secrets only after backup and rollback evidence exists.

