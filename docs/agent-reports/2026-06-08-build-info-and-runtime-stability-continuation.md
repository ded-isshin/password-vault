# Agent Report: Build Info And Runtime Stability Continuation

Status: public-safe evidence report.
Date: 2026-06-08.

## Goal

Continue the Password Vault stabilization review with current live access checks, PostgreSQL HA
evidence, SRE source refresh, Claude Code review, and the `password_vault_build_info` revision fix.

## Active Context

- Product repository: `password-vault`.
- Infrastructure worktree: read-only live Kubernetes and Grafana checks only.
- Out of scope: unrelated product repositories and destructive cluster actions.

## Verified Runtime State

- Password Vault, Grafana, and Argo CD respond through the mini-PC LAN-facing edge routes.
- The edge host listens on the LAN-facing ports for Password Vault, Grafana, and Argo CD.
- Argo CD reports `password-vault` and `observability-vm-stack` as `Synced` and `Healthy`.
- Password Vault API is `3/3` ready on an immutable GHCR digest and is spread across three worker
  nodes.
- Grafana dashboard `Password Vault Overview` is provisioned with 12 panels.
- VictoriaMetrics query `sum(up{job="password-vault-api"}) or vector(0)` returned `3`.
- Product registration and vault item counters returned live nonzero series after synthetic traffic.
- The current deployed digest still reports `password_vault_build_info{revision="unknown"}`.
- Current product database is one `postgres:17-bookworm` StatefulSet with one `local-path` PVC.
- Another product has a separate PostgreSQL StatefulSet. This is not a direct conflict as long as
  namespaces, services, credentials, databases, migrations, PVCs, and backup prefixes remain
  product-specific.
- CloudNativePG CRDs exist, but no `Cluster`, `Backup`, or `ScheduledBackup` resources were found.
- No `NetworkPolicy` resource exists in the `password-vault` namespace.

## Build Info Fix

Implemented locally:

- Docker builds now accept `BUILD_REVISION`.
- The Docker build passes `BUILD_REVISION` into the Rust compile-time environment variable
  `PASSWORD_VAULT_BUILD_REVISION`.
- `build.rs` declares `cargo:rerun-if-env-changed=PASSWORD_VAULT_BUILD_REVISION`.
- GitHub container and load-smoke workflows pass `${{ github.sha }}` / `${GITHUB_SHA}` as
  `BUILD_REVISION`.
- Documentation now uses `BUILD_REVISION` instead of the previous misleading Docker build arg name.

Validation:

- `git diff --check`: passed.
- GitHub workflow YAML parse: passed.
- `node --check load/synthetic/browser-api-journey.mjs`: passed.
- Local Docker build with `--build-arg BUILD_REVISION=local-test-revision`: passed.
- Local container smoke verified:
  `password_vault_build_info{version="0.1.0",revision="local-test-revision"} 1`.

Not tested:

- Local host `cargo test`, because `cargo` is not installed on the mini-PC host.
- Live Grafana build-info revision after rollout. The fixed image still needs to be published and
  rolled out through GitOps.

## SRE Source Refresh

Official sources checked:

- Google SRE Book, Monitoring Distributed Systems.
- Google SRE Workbook, Monitoring.
- Google SRE Workbook, Implementing SLOs.
- Google SRE Workbook, Alerting on SLOs.
- PostgreSQL Versioning Policy.
- CloudNativePG releases and supported releases.
- CloudNativePG backup, replication, recovery, and Barman Cloud Plugin documentation.

Current conclusions:

- SRE dashboards should answer service questions and include latency, traffic, errors, and
  saturation.
- Alerts should remain simple, actionable, and tied to user-visible or imminent user-visible impact.
- SLOs should be user-centric and separate SLI specification from concrete SLI implementation.
- PostgreSQL 17 remains supported and is not legacy; PostgreSQL 18 is the latest major version, but
  a major upgrade should be a planned database task, not mixed into this stabilization slice.
- CloudNativePG 1.29.x is the current supported operator line as of this review.
- CloudNativePG backup/recovery should use a supported backup path with WAL archiving and restore
  drills; the Barman Cloud Plugin is the preferred object-store direction in current documentation.

## Decisions Recommended

- Keep MacBook/browser URLs on the mini-PC LAN-facing edge routes, not Kubernetes/LXD service IPs.
- Treat the current PostgreSQL StatefulSet as preview-only.
- Move Password Vault to a product-owned CloudNativePG cluster before real secrets.
- Prioritize off-box backup, WAL archiving, restore drill, and failover drill before accepting real
  user data.
- Use synchronous quorum replication for real password-manager writes once the HA database is
  deployed, while documenting the write-availability tradeoff.
- Keep migrations, but keep them rare, reviewed, immutable after merge, and run through controlled
  Argo/GitOps migration jobs rather than normal API startup.
- Keep MVP scope narrow: register, MFA, browser unlock, encrypted item CRUD, sync, synthetic tests,
  observability, backup/restore, and safe rollout.

## Claude Code Usage

Purpose: independent architecture/SRE/security review.

Prompt/task given: report-only review of verified runtime facts, PostgreSQL HA, browser access,
Golden Signals, SLOs, migration policy, and agent waste reduction.

Summary of output:

- Confirmed blockers: no off-box backups/PITR, no PostgreSQL HA, no NetworkPolicy.
- Confirmed MacBook should use the mini-PC LAN edge routes, not LXD/Kubernetes addresses.
- Confirmed migrations are still necessary because application schema changes are separate from
  PostgreSQL engine stability.
- Recommended off-box backup and restore drill as the first data-safety priority.
- Recommended CNPG three-instance cluster, explicit replication choice, and product-specific
  database isolation.
- Recommended one source of truth for cluster access and strict evidence gates for agent work.

Accepted suggestions:

- Treat backup/restore and NetworkPolicy as blocking gates before real secrets.
- Keep product-specific database isolation and avoid sharing another product's PostgreSQL instance.
- Keep migration jobs separate from API startup.
- Add process guardrails: one writer per scope, verified evidence before claims, and no speculative
  manifests without an MVP gate.

Rejected or adjusted suggestions:

- A single p95 latency target below 300 ms is too aggressive for all routes because auth paths may
  include expensive password hashing. Keep separate candidate SLOs for product API latency and auth
  latency.
- The current three worker nodes are still useful for Kubernetes rollout and pod-level resilience,
  but they do not replace off-box backup or physical-host resilience.

## Next Steps

1. Merge and roll out the `build_info` revision fix, then verify Grafana reports a GitHub SHA.
2. Add NetworkPolicy or an internal-only metrics listener and restrict PostgreSQL access.
3. Deploy product-owned CloudNativePG with off-box WAL archiving and scheduled backups.
4. Run and document restore and failover drills.
5. Add SLO/burn-rate alert rules and dashboard rows for backup age, replication lag, DB pressure,
   and synthetic journey pass/fail.
