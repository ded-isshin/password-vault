# Agent Report: Grafana, Argo CD, CNPG, SRE Follow-Up

Status: historical evidence report.

Date: 2026-06-08.

## Goal

Answer the current stabilization questions for the deployed Password Vault MVP preview:

- browser access to Grafana, Argo CD, and Password Vault;
- PostgreSQL HA, product isolation, and migration policy;
- Google SRE Golden Signals and product/business observability;
- minimum stabilization tasks before real secrets;
- waste reduction and stale task cleanup.

Canonical current-state updates were made in the MVP plan, observability plan, PostgreSQL decision
brief, load README, and release runbook. This report records what was checked in this session.

## Active Context

- Product repository: `password-vault`.
- Infrastructure worktree: Password Vault GitOps/edge docs only.
- Live cluster diagnostics: read-only.
- Public safety: no secrets, kubeconfigs, concrete private addresses, private hostnames, tokens, or
  runtime logs are recorded here.

## Runtime Findings

- The default shell kubeconfig had no current context after session restart. Read-only cluster checks
  require an explicit production kubeconfig path.
- Argo CD Applications checked through Kubernetes were `Synced` and `Healthy`, including
  `password-vault` and `prod-root`.
- Grafana dashboard `Password Vault Overview` exists, is provisioned, and has 23 panels.
- Grafana datasource queries returned live Password Vault API, CNPG, blackbox, latency, error,
  backup, and product-event data.
- Password Vault API had three ready replicas and a successful Deployment rollout.
- Password Vault CNPG had three ready PostgreSQL instances spread across worker nodes.
- The active primary reported synchronous commit with `ANY 1` quorum synchronous standby behavior,
  and both standbys were streaming with quorum sync state.
- No CNPG `Backup` or `ScheduledBackup` resources exist for Password Vault yet.
- `PasswordVaultCnpgBackupMissing` was firing as expected and remains a real-secret-use blocker.
- The legacy single PostgreSQL StatefulSet is still present as rollback debt and is not the active
  API database.
- HiringTrace PostgreSQL is separate. There is no conflict as long as Password Vault keeps its own
  namespace, CNPG cluster, database, credentials, PVCs, services, backup prefix, and migrations.

## Runtime Re-Check: 2026-06-08T18:20Z

This re-check was triggered after a browser-access report and after product PR #99 was merged.

Verified from the mini-PC:

- The mini-PC LAN interface remained the browser-facing route for Grafana, Argo CD, and Password
  Vault.
- HTTPS requests to the LAN edge returned page titles for all three services:
  `Grafana`, `Argo CD`, and `Password Vault`.
- The host had listeners bound to `0.0.0.0` for the expected browser ports:
  Grafana `3000`, Argo CD `9443`, and Password Vault `11443`.
- The Kubernetes/LXD `LoadBalancer` address for Argo CD also returned HTTP 200 from the mini-PC, but
  it should not be treated as the normal MacBook URL unless that client has routing into the
  Kubernetes/LXD network.
- A plain `kubectl` command without `KUBECONFIG` still defaulted to `localhost:8080` and failed. The
  read-only cluster checks succeeded with the explicit production kubeconfig path from
  `infrastructure-home` documentation.

Read-only Kubernetes findings:

- All six Kubernetes nodes were `Ready`.
- Argo CD Applications, including `prod-root` and `password-vault`, were `Synced` and `Healthy`.
- `argocd-server-external` remained a cluster `LoadBalancer` service. Browser access for a normal
  LAN client should still go through the mini-PC edge route.
- Grafana was exposed through the observability `LoadBalancer` service and was also reachable
  through the mini-PC edge route.
- Password Vault API had three ready replicas spread across three worker nodes.
- Password Vault API was still deployed at the previous runtime image digest. Product PR #99
  published a newer GHCR image digest for the same application code lineage and pinned base images,
  but that digest had not been promoted through GitOps during this re-check.
- Password Vault CNPG had three ready instances and the cluster status was healthy.
- No Password Vault CNPG `Backup` or `ScheduledBackup` resources were present.
- The legacy single-node PostgreSQL `StatefulSet` remained present as rollback debt and was not the
  active API database path.

Grafana/VictoriaMetrics findings:

- Grafana MCP datasource discovery succeeded; `VictoriaMetrics` was the default Prometheus-compatible
  datasource.
- Dashboard `password-vault-overview` was found.
- `sum(up{job="password-vault-api"}) or vector(0)` returned `3`.
- `sum(up{job="password-vault-cnpg"}) or vector(0)` returned `3`.
- `max(cnpg_pg_replication_streaming_replicas{job="password-vault-cnpg"}) or vector(0)` returned
  `2`.
- `max(cnpg_pg_replication_lag{namespace="password-vault",pod=~"password-vault-cnpg-.*"}) or
  vector(0)` returned `0`.
- `max(cnpg_collector_last_available_backup_timestamp{namespace="password-vault",pod=~"password-vault-cnpg-.*"})
  or vector(0)` returned `0`.
- `sum by (state) (password_vault_db_pool_connections{namespace="password-vault"})` returned
  `max=15`, `idle=3`, and `used=0` across the API replicas at the instant checked.
- `ALERTS{alertname=~"PasswordVault.*",alertstate="firing"}` returned
  `PasswordVaultCnpgBackupMissing`, which is expected while no available base backup exists.
- A stale local check used the wrong old metric name
  `cnpg_collector_pg_replication_streaming_replicas`; the deployed dashboard uses the current
  `cnpg_pg_replication_streaming_replicas` metric and returned the expected value.

Product PR #99 follow-up:

- PR #99 was merged to `main`.
- Main `docs`, `public-safety`, `rust`, `postgres-migrations`, and `container publish` checks passed.
- The new published image digest was
  `ghcr.io/ded-isshin/password-vault-api@sha256:27d69503a77da36c58bc36c0e4430d3ca4c6b013ef085c2341d10f424e65a6b2`.
- This improves supply-chain drift/integrity by pinning CI and build base images by digest. It does
  not fully remove Docker Hub availability risk because digest pulls still contact Docker Hub unless
  mirrored or cached.

Infra source-of-truth follow-up:

- The default `<infra-repo-checkout>` checkout was on an older dirty feature branch and
  must not be used as current Password Vault GitOps evidence.
- The current Password Vault infra worktree was `<infra-current-worktree>`.
- That worktree was clean, on `main`, equal to `origin/main`, and at
  `0fdeb9205d52fd29611675993961b017c0a61fc0`.
- The committed GitOps source contains the Password Vault Argo Application, CNPG cluster,
  CNPG NetworkPolicy, CNPG scrape, blackbox probe, VMRule alerts, and Grafana dashboard.
- Therefore the corrected top finding is not "missing GitOps source." The corrected blocker is
  missing CNPG backup/WAL/PITR/restore/failover evidence.
- The legacy `password-vault-postgres` StatefulSet is still committed and live as rollback debt.
  It should be removed only after backup, restore, and failover evidence are recorded.

## Browser Access

Correct browser/LAN entrypoints are the mini-PC edge HTTPS ports:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Kubernetes `LoadBalancer` addresses are backend/internal targets for this home platform and should
not be given to a normal MacBook browser unless that client has explicit routing into the
Kubernetes/LXD network.

Verified from the mini-PC:

- edge listeners were bound to `0.0.0.0` for the three expected LAN-facing ports;
- Password Vault health, Grafana health, and Argo CD health/root checks returned success through
  the edge path.

Needs verification from the MacBook:

- run the documented `curl -k -I` checks against the mini-PC LAN address;
- use `https`, not `http`;
- if mini-PC checks pass and MacBook checks fail, investigate MacBook LAN/VPN/firewall/client
  routing before changing Kubernetes.

## Product Validation

Added a local browser-crypto synthetic guard:

- `SYNTHETIC_SELF_TEST_ONLY=true node load/synthetic/browser-api-journey.mjs`
- checks account-secret display parsing;
- checks AES-GCM decrypt-time authentication failure for tampered ciphertext, nonce, and item
  metadata bound through associated data;
- is wired into the always-running `docs` workflow.

Validated:

- `node --check crates/api/static/app.js`
- `node --check load/synthetic/browser-api-journey.mjs`
- `SYNTHETIC_SELF_TEST_ONLY=true node load/synthetic/browser-api-journey.mjs`
- full live edge synthetic journey:
  `register -> TOTP enroll -> logout -> return login -> TOTP -> unlock -> encrypted item create -> sync/decrypt -> recovery-code login -> vault denial -> TOTP re-enrollment`

The live synthetic journey used reserved `.invalid` synthetic data and did not print account secret
keys, TOTP seeds, TOTP codes, recovery codes, cookies, plaintext item passwords, account IDs, vault
IDs, item IDs, or device IDs.

## Container CI Follow-Up

The PR smoke build initially failed while resolving `docker.io/docker/dockerfile:1.18` with a Docker
Hub gateway timeout before product code was built. The Dockerfile does not use features requiring a
custom Dockerfile frontend, so the optional `# syntax=docker/dockerfile:1.18` directive was removed.
This keeps product image builds on GitHub Actions/GHCR while reducing unnecessary Docker Hub
dependency during the build setup path. Docker Hub remains acceptable for reviewed, trusted,
versioned base/test images; it should not be the product release registry.

## Observability

The observability plan is aligned with Google SRE guidance:

- dashboard and alerts start with latency, traffic, errors, and saturation;
- symptom checks are separated from cause-level debugging panels;
- product/business signals are treated as reliability signals for a password manager, not marketing
  metrics;
- high-cardinality or sensitive user/object labels remain forbidden.

Current maturity:

- L0/L1 visibility is useful: targets, request rate, 5xx ratio, p95 latency, pending requests, CNPG
  scrape, blackbox readyz, and first product counters return live data.
- L2 is incomplete: alert rules exist, but notification delivery has not been smoke-tested.
- L3 is incomplete: ad-hoc synthetic journey works, but scheduled external synthetic pass/fail and
  cleanup metrics are not deployed.
- L4 is incomplete: backup, WAL archive, restore drill, and failover drill evidence are missing.

## PostgreSQL And Migrations

PostgreSQL clustering direction is technically sound for a password manager:

- use product-owned CloudNativePG cluster, not another product database;
- use quorum synchronous replication with one synchronous standby for real-secret durability;
- accept that `dataDurability: required` can pause writes when the required standby is unavailable;
- prefer temporary write unavailability over acknowledged secret loss;
- keep backups and PITR as separate mandatory controls.

Stable PostgreSQL versions do not remove schema migrations. PostgreSQL version maintenance controls
the engine; migrations control application-owned tables, constraints, indexes, auth/MFA/session
state, sync metadata, and crypto/key-wrap metadata.

MVP posture:

- schema freeze by default;
- no speculative migrations;
- add migrations only for required security invariants, durability gates, MVP journey support, or
  rollout safety;
- keep startup migrations disabled for real-user API pods;
- use reviewed migration jobs and expand/contract patterns for future real-user environments.

## Claude Code Usage

Purpose: independent architecture, SRE, PostgreSQL, observability, and browser-access review.

Prompt/task given: review current diffs and runtime conclusions for Password Vault MVP
stabilization, with no edits or mutating commands.

Summary of output:

- Found no blockers in the current diff.
- Confirmed PostgreSQL synchronous replication and migration explanation is technically sound.
- Confirmed browser-access diagnosis is sound: use mini-PC edge URLs, not internal LoadBalancer
  addresses.
- Confirmed observability direction matches Google SRE style.
- Flagged remaining blockers before real secrets: backup/WAL/PITR/restore/failover and alert
  delivery.
- Recommended tightening the crypto self-test error expectation, documenting `ANY 1`
  durability/availability tradeoff, and adding edge listener bind checks.

Accepted suggestions:

- `assertRejects` was tightened to expect AES-GCM `OperationError`.
- PostgreSQL decision brief now documents the write-unavailability tradeoff.
- Release and browser-access runbooks now include edge listener bind checks.
- Image Renderer and dashboard expansion remain deferred until alert delivery and durability gates
  are proven.

Rejected suggestions:

- Claude's initial top finding claimed the Password Vault GitOps manifests were missing from the
  default infra checkout. That was rejected after verifying that the checkout Claude inspected was
  an older dirty feature branch. The current infra worktree and `origin/main` do contain the
  Password Vault Application, CNPG, dashboard, blackbox, and alert manifests.

Follow-up review:

- Claude re-ran a corrected review against the current infra worktree.
- Accepted: CNPG backup/WAL/PITR remains the top blocker; restore drill, failover drill, alert
  delivery smoke, scheduled external synthetic metrics, and MacBook-side reachability remain open.
- Accepted: promoting the PR #99 image digest is a routine GitOps promotion task, not a durability
  blocker.
- Verified locally after Claude follow-up: the infra worktree was clean and equal to `origin/main`;
  the legacy PostgreSQL StatefulSet remains present in GitOps and runtime.

Deferred suggestions:

- Scheduled external synthetic and cleanup metrics remain deferred until backup/HA posture is
  better understood.
- Additional crypto vectors beyond the accepted format are deferred.

## GitHub Issue Hygiene

Closed as completed with evidence comments:

- #16 auth sessions and TOTP MFA server flows;
- #18 encrypted vault item API and sync conflict checks;
- #19 browser web app;
- #20 Docker build, CI tests, and GHCR release workflow;
- #21 Helm chart;
- #22 infrastructure GitOps application.

Kept open:

- #17 until the tamper self-test and CI gate land through PR;
- #5, #23, #73 for backup/restore/failover, runbook, SLO/observability gates;
- #79 until stale issue triage is fully reconciled;
- #11 MVP epic.

## Files Changed

Product repository:

- `.github/workflows/docs.yml`
- `load/synthetic/browser-api-journey.mjs`
- `load/README.md`
- `docs/runbooks/release-and-rollout.md`
- `docs/observability-sre-metrics.md`
- `docs/decision-briefs/2026-06-08-postgresql-ha-migrations-stability.md`
- `docs/mvp-implementation-plan.md`
- `docs/agent-reports/2026-06-08-grafana-argo-cnpg-sre-followup.md`
- `Dockerfile`
- `docs/research/container-ci-observability-load-2026-06-07.md`

Infrastructure worktree:

- `docs/runbooks/browser-access-to-lan-services.md`

## Commands Run

Representative commands, with sensitive local paths and private addresses redacted:

```bash
KUBECONFIG=<redacted-path> kubectl -n argocd get applications
KUBECONFIG=<redacted-path> kubectl -n password-vault get deploy,pods,svc
KUBECONFIG=<redacted-path> kubectl -n password-vault get cluster,backup,scheduledbackup
KUBECONFIG=<redacted-path> kubectl -n password-vault exec <cnpg-primary> -- psql -Atqc '<replication checks>'
curl -kfsS https://<mini-pc-lan-ip>:11443/healthz
curl -kfsS https://<mini-pc-lan-ip>:3000/api/health
curl -kfsS https://<mini-pc-lan-ip>:9443/healthz
ss -ltn | grep -E ':(11443|3000|9443)\b'
node --check crates/api/static/app.js
node --check load/synthetic/browser-api-journey.mjs
SYNTHETIC_SELF_TEST_ONLY=true node load/synthetic/browser-api-journey.mjs
BASE_URL=https://<mini-pc-lan-ip>:11443 SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL=true \
  SYNTHETIC_TLS_INSECURE=true SYNTHETIC_CHECK_METRICS=false \
  node load/synthetic/browser-api-journey.mjs
git diff --check
```

The Dockerfile frontend dependency was removed after PR smoke hit Docker Hub 504 while fetching the
optional `docker/dockerfile` frontend image.

Grafana MCP was used for read-only dashboard and VictoriaMetrics datasource checks.

## Sources Consulted

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- CloudNativePG replication documentation:
  <https://cloudnative-pg.io/docs/1.25/replication/>
- Barman Cloud Plugin documentation:
  <https://cloudnative-pg.io/plugin-barman-cloud/docs/intro/>
- Dockerfile reference:
  <https://docs.docker.com/reference/dockerfile/>

## Risks

- MacBook browser reachability has not been directly tested from the MacBook.
- Backup/PITR/restore/failover remains missing and blocks real-secret use.
- Alertmanager delivery has not been smoke-tested.
- Scheduled external synthetic pass/fail and cleanup metrics are not deployed.
- Host Rust tooling is unavailable locally; Rust tests depend on CI/container workflow unless a
  separate toolchain decision is made.

## Next Steps

1. Open and merge a product PR for the crypto self-test/docs/workflow updates.
2. Open and merge an infrastructure docs PR for the browser-access runbook update.
3. Run the MacBook-side browser/curl checks against the mini-PC edge URLs.
4. Configure CNPG backup/WAL/PITR target and run restore/failover drills.
5. Smoke-test Alertmanager delivery.
6. Add scheduled external synthetic pass/fail metrics and cleanup lifecycle after backup posture is
   understood.
