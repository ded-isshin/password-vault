# Session Report: Access, CI, NetworkPolicy, And SRE Refresh

Status: completed for this stabilization slice. Scope: Password Vault MVP preview, infrastructure
CI, browser access, PostgreSQL isolation, and SRE planning.

## Goal

Verify Grafana, Argo CD, and Password Vault browser access; clarify PostgreSQL HA and migration
policy; reduce CI/download flakiness; deploy a first safe NetworkPolicy; refresh SRE/Golden Signals
evidence; and identify the next stability work that matters before real user secrets.

## Active Context

- Product repo: `password-vault`.
- Infrastructure repo: `infrastructure-home`.
- Runtime context: production Kubernetes GitOps preview.

Public safety: browser and cluster addresses are documented here as placeholders. Do not replace
them with private home-network addresses in public docs unless explicitly approved.

## Work Completed

- Verified that browser access should use the mini-PC LAN edge ports, not Kubernetes/LXD
  `LoadBalancer` addresses.
- Confirmed Grafana, Argo CD, and Password Vault edge routes respond successfully from the mini-PC.
- Verified the Grafana datasource and dashboard:
  - datasource: `VictoriaMetrics`;
  - dashboard: `Password Vault Overview`;
  - `up{job="password-vault-api"}` returned `3`;
  - build info returned the deployed product commit SHA.
- Merged infrastructure PR #113 to harden CI downloads:
  - replaced raw TFLint install flow with containerized TFLint execution;
  - replaced Gitleaks release-asset download with containerized Gitleaks execution;
  - added Terraform provider cache;
  - retained retries for Ansible dependency installation and Terraform init.
- Merged infrastructure PR #114 to add a first Password Vault PostgreSQL NetworkPolicy.
- Verified Argo CD applied the NetworkPolicy from infrastructure main revision `d1ae20f`.
- Verified the live browser/API synthetic journey still passes after the NetworkPolicy:
  `register -> TOTP -> login -> unlock -> create encrypted item -> sync/read`.
- Updated `docs/observability-sre-metrics.md` to remove stale build-info and current-branch claims,
  document the first NetworkPolicy, and add official sources.

## Current Runtime State

- Password Vault API: 3 ready replicas.
- Password Vault PostgreSQL: single `postgres:17-bookworm` StatefulSet replica on node-local
  storage. This remains preview-only and not HA.
- NetworkPolicy: PostgreSQL ingress on TCP/5432 is restricted to Password Vault API pods and Argo CD
  migration hook pods.
- Grafana dashboard: live and returning product/API metrics.
- Argo CD: product and root applications report `Synced` and `Healthy`.
- Edge browser routes:
  - Password Vault: `https://<lan-edge-ip>:<password-vault-port>/`
  - Grafana: `https://<lan-edge-ip>:<grafana-port>/`
  - Argo CD: `https://<lan-edge-ip>:<argocd-port>/`

## PostgreSQL HA Decision

Clustered PostgreSQL is required before real password-manager data. The current single StatefulSet
is acceptable only as bootstrap/demo infrastructure.

There is no observed product-database conflict with another product. The correct pattern is to share
the CloudNativePG operator, not a database. Password Vault should have its own CloudNativePG
Cluster, credentials, services, backups, restore drills, and object-store prefix.

Recommended production-like direction remains:

- 3 CloudNativePG instances spread across workers;
- quorum synchronous replication with one synchronous standby;
- `dataDurability: required` for real user secrets;
- WAL archiving plus scheduled physical base backups;
- restore and failover drills before accepting real secrets.

## Migration Analysis

Stable PostgreSQL versions do not remove application schema migrations. PostgreSQL engine stability
means predictable database server behavior; it does not create or evolve Password Vault tables,
constraints, indexes, auth fields, encrypted sync metadata, or future tenant boundaries.

The desired state is not "constant migrations." The desired state is rare, reviewed,
backward-compatible migrations:

- startup migrations stay disabled for real-user environments;
- schema changes run through a controlled GitOps migration hook or reviewed operator step;
- use expand/contract releases for destructive or high-risk changes;
- verify backups/restore posture before dangerous schema changes.

## SRE And Observability Analysis

Google SRE's Four Golden Signals still map cleanly to this product:

- Latency: product/API/auth request latency, with failed and successful request latency separated.
- Traffic: RPS plus product operation counters such as registration, login, MFA, item writes, and
  sync pulls.
- Errors: 5xx, policy errors, failed synthetic journeys, and product-level failure ratios.
- Saturation: pending requests, DB pool pressure, auth challenge pressure, CPU/memory, replication lag,
  disk pressure, backup age, and WAL/archive health.

Product/business metrics should not be vanity metrics. The MVP dashboard should focus on:

- protected activation: registration finished, MFA confirmed, first encrypted item saved;
- returning access: login proof, MFA, and vault unlock succeed;
- core write success: encrypted item create/update/delete later syncs and decrypts;
- data survival: backup/restore/failover drills are current and passing;
- abuse resistance: aggregate rate-limit, CSRF, MFA failure, and invalid-origin signals.

The live dashboard is useful L1 Golden Signals plus product counters. It is not L2/L3/L4 yet because
alert rules, external synthetic probes, database durability panels, and backup/failover metrics are
not deployed.

## Claude Code Usage

Purpose: independent architecture, SRE, and security review.

Prompt/task given: review browser access, PostgreSQL HA, migrations, Golden Signals, NetworkPolicy,
metrics exposure, and waste reduction for the Password Vault MVP.

Summary of output:

- Confirmed the edge-vs-LXD browser access model.
- Confirmed product-specific CloudNativePG HA is required before real secrets.
- Confirmed synchronous replication with required durability is the right default for a password
  manager.
- Confirmed migrations remain mandatory even with stable PostgreSQL versions.
- Flagged NetworkPolicy and `/metrics` exposure as blockers.
- Flagged alerting and saturation/durability metrics as the main observability gaps.
- Recommended consolidating current-state reports to reduce documentation drift.

Accepted suggestions:

- Treat durability, NetworkPolicy, metrics exposure, and alerting as blockers before real secrets.
- Keep migration execution controlled and separate from API startup.
- Keep docs canonical in plans/ADRs/runbooks and use agent reports as historical evidence only.

Rejected or deferred suggestions:

- Full namespace default-deny was deferred until API ingress and metrics access have a tested
  allow-list design.
- External synthetic deployment was deferred until the probe destination and alert route are chosen.

## Commands Run

Representative commands:

```bash
KUBECONFIG=<redacted-path> kubectl -n argocd get application password-vault prod-root -o wide
KUBECONFIG=<redacted-path> kubectl -n password-vault get networkpolicy,deploy,svc,pods -o wide
KUBECONFIG=<redacted-path> kubectl -n password-vault describe networkpolicy password-vault-postgres
curl -k -sS -o /dev/null -w '%{http_code}\n' https://<lan-edge-ip>:<password-vault-port>/
curl -k -sS -o /dev/null -w '%{http_code}\n' https://<lan-edge-ip>:<grafana-port>/api/health
curl -k -sS -o /dev/null -w '%{http_code}\n' https://<lan-edge-ip>:<argocd-port>/healthz
kubectl kustomize kubernetes/gitops/prod >/tmp/password-vault-networkpolicy-kustomize.yaml
git diff --check
gh pr checks 113 --watch --interval 15
gh pr checks 114 --watch --interval 15
```

Live synthetic:

```bash
RUN_ID=<redacted-run-id> \
BASE_URL=https://<lan-edge-ip>:<password-vault-port> \
SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL=true \
SYNTHETIC_TLS_INSECURE=true \
SYNTHETIC_CHECK_METRICS=false \
node load/synthetic/browser-api-journey.mjs
```

## Validation

Tested:

- Infrastructure PR #113 CI passed.
- Infrastructure PR #114 CI passed.
- Argo CD applied infrastructure main revision `d1ae20f`.
- Kubernetes rendered the GitOps tree with the new NetworkPolicy.
- Runtime NetworkPolicy exists and restricts PostgreSQL ingress to API and migration pods.
- Password Vault, Grafana, and Argo CD edge routes returned HTTP 200-class health responses.
- Live synthetic browser/API journey passed after NetworkPolicy deployment.
- Grafana/VictoriaMetrics returned live product metrics after synthetic traffic.

Not tested:

- Cross-pod negative test proving an arbitrary pod cannot connect to PostgreSQL. A dedicated
  temporary test pod should be added only with a reviewed safe test pattern.
- Full namespace default-deny.
- Internal-only metrics listener.
- PostgreSQL failover, backup, restore, and WAL archiving.
- Alert delivery.

## Risks

- Current PostgreSQL remains single-replica preview infrastructure.
- `/metrics` still shares the API port internally.
- No product-specific alert rules are deployed yet.
- CI still depends on external registries and package indexes, but the most fragile raw release
  downloads have been reduced.
- Documentation drift risk remains if every session creates a parallel current-state report instead
  of updating canonical plans and ADRs.

## Next Steps

1. Design and deploy API/metrics hardening: either a separate internal metrics listener or a tested
   NetworkPolicy/ingress allow-list model.
2. Add product alert rules: target down, fast 5xx burn, latency regression, pending request pressure.
3. Plan and implement product-specific CloudNativePG resources with three instances, sync
   replication, WAL archiving, scheduled backups, and restore/failover drills.
4. Add DB pool/query/error saturation metrics in the API.
5. Add an external synthetic probe and dashboard panel for the full browser/API journey.
6. Consolidate stale agent reports; keep canonical state in the observability plan, ADRs, runbooks,
   and deployment contracts.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- Kubernetes documentation, Network Policies:
  <https://kubernetes.io/docs/concepts/services-networking/network-policies/>
- Kubernetes API reference, NetworkPolicy:
  <https://kubernetes.io/docs/reference/kubernetes-api/networking/network-policy-v1/>
- CloudNativePG documentation, Replication:
  <https://cloudnative-pg.io/docs/1.27/replication/>
- CloudNativePG Barman Cloud Plugin, Main Concepts:
  <https://cloudnative-pg.io/plugin-barman-cloud/docs/concepts/>
- PostgreSQL documentation, Versioning Policy:
  <https://www.postgresql.org/support/versioning/>
