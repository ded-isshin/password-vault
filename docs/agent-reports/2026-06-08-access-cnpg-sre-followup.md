# Session Report: Access, CNPG, SRE, And MVP Stabilization Follow-Up

Status: current evidence report. Date: 2026-06-08.

## Goal

Answer the current runtime and stabilization questions for the deployed Password Vault MVP preview:

- which browser URLs to use for Grafana, Argo CD, and Password Vault from a MacBook;
- whether PostgreSQL must be clustered and whether it conflicts with another product database;
- which SRE and business metrics matter next;
- why schema migrations are still needed on stable PostgreSQL;
- what to cut, defer, or tighten to reduce waste.

## Active Context

- Product repository: `password-vault`.
- Infrastructure repository: documentation-only follow-up for GitOps/browser access wording.
- Runtime: read-only Kubernetes, Grafana, and local edge-host checks.
- Public safety: concrete home-network addresses and secrets are intentionally omitted.

## Verified

- Password Vault, Grafana, and Argo CD respond through the mini-PC LAN-facing edge ports.
- Kubernetes `LoadBalancer` addresses in the LXD/Kubernetes network are not the default browser
  path for a MacBook unless that client has an explicit route or VPN into that network.
- Host NGINX listens on the edge ports for Password Vault, Grafana, and Argo CD and proxies to the
  Kubernetes/LXD service addresses.
- Argo CD reports `prod-root` and `password-vault` as `Synced` and `Healthy`.
- Password Vault API has three ready pods spread across the three worker nodes.
- `password-vault-cnpg` has three ready CloudNativePG PostgreSQL 18.4 instances spread across the
  three worker nodes.
- The only CloudNativePG `Cluster` currently present is `password-vault-cnpg`; HiringTrace uses a
  separate product-owned PostgreSQL `StatefulSet` in its own namespace.
- There are no CloudNativePG `Backup` or `ScheduledBackup` resources yet.
- The Grafana/VictoriaMetrics Password Vault dashboard queries all parse and return live data or an
  explicit justified zero with representative `5m` rate and `6h` range windows.
- `PasswordVaultCnpgBackupMissing` is firing as an expected warning because no available base backup
  exists.
- GitHub `main` is protected by an active repository ruleset requiring pull requests, squash merges,
  resolved conversations, linear history, non-fast-forward protection, branch deletion protection,
  and the always-running `docs` and `public-safety` checks.
- Repository settings enable squash-only merging, auto-merge, update-branch, delete-branch-on-merge,
  vulnerability alerts, Dependabot security updates, secret scanning, and push protection.

## Browser Access

Use the mini-PC LAN edge endpoint from a normal home-LAN browser:

```text
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
Password Vault: https://<mini-pc-lan-ip>:11443/
```

Do not use Kubernetes/LXD service, pod, or cluster addresses as the default browser URLs from a
MacBook. Those addresses are useful for cluster-side diagnostics, not for a LAN-only client.

If the MacBook still cannot connect to the edge URL, check these first:

- the MacBook is on the same LAN/VPN path as the mini-PC;
- the browser uses `https://`, not `http://`, for the edge ports;
- the browser accepted the self-signed certificate warning;
- local network client isolation or firewall policy is not blocking MacBook-to-mini-PC traffic.

## PostgreSQL And CNPG

Clustered PostgreSQL is required before real password-manager secrets are accepted. A single
PostgreSQL `StatefulSet` is acceptable only as preview/rollback debt.

There is no inherent conflict with HiringTrace or another product if the boundary stays strict:

- share the CloudNativePG operator as platform infrastructure;
- do not share another product's database, role, Secret, PVC, Service, backup prefix, or migration
  pipeline;
- keep Password Vault data in a product-owned database cluster and namespace.

Current CNPG state is a good HA foundation but not yet a complete durability story:

- three PostgreSQL instances are ready;
- synchronous quorum replication is configured for one required standby;
- WAL archive failures are not currently increasing;
- backup availability is still `0`;
- no restore or failover drill has been completed.

The P0 blocker is therefore recoverability evidence, not CNPG conflict.

## SRE Metrics

The useful observability model is:

- technical Golden Signals: latency, traffic, errors, saturation;
- product journey signals: protected activation, returning access, vault write+sync, recovery;
- durability signals: backup freshness, WAL archive health, restore drill freshness, failover drill
  freshness;
- security signals: rate limits, MFA failures, recovery-code usage, CSRF/origin/content rejection
  rates.

Current state is useful L0/L1 observability:

- API, CNPG, and black-box readiness targets are scraped;
- HTTP request, latency, and pending metrics exist;
- product counters exist for registration, login, MFA, vault item changes, and sync;
- CNPG replication, version, backup timestamp, lag, and WAL archive failure panels exist.

Next maturity:

- add full synthetic journey pass/fail and step-duration metrics;
- add SLO/burn-rate dashboard rows once min-volume or synthetic traffic makes the signal meaningful;
- add DB query latency, DB pool wait, DB error, auth/MFA step duration, and challenge pressure metrics;
- add derived business/product SLIs for activation, access, write+sync, and recovery.

Use `or vector(0)` only when an explicit zero is the correct dashboard fallback. For gate panels and
alerts, missing telemetry must remain distinguishable from a healthy zero.

## Migration Analysis

Stable PostgreSQL versions do not remove schema migrations.

PostgreSQL version stability covers the database engine. Password Vault still owns the application
schema: accounts, sessions, MFA state, recovery codes, device metadata, key wraps, vault revision
metadata, constraints, and indexes. Those must be versioned, tested, and rolled out deliberately.

The stable target is:

- few migrations, not no migrations;
- reviewed, backward-compatible migrations after real users exist;
- no speculative schema churn;
- no API startup migrations in real-user environments;
- Argo CD migration jobs for production-like schema changes;
- expand/contract rollout for populated tables;
- backup and restore readiness before destructive or lock-heavy DDL.

## Cut Or Defer

Cut from the immediate path:

- importing KeePass databases;
- browser extension;
- mobile clients;
- organizations/shared vaults/admin console;
- Vault/OpenBao as the user-vault database or decrypt path;
- dashboards that do not support an MVP gate, alert, release decision, or debugging question;
- new large agent reports when a canonical doc or issue update is enough.

Defer until after P0 durability:

- removing the legacy `password-vault-postgres` StatefulSet/PVC;
- destructive schema migrations;
- deeper network allow-listing that requires an edge routing redesign;
- advanced passkey/WebAuthn unlock flows;
- SLO paging without synthetic/min-volume guardrails.

## Next Tasks

P0:

1. Choose the backup target and credentials path outside Git.
2. Add CloudNativePG-supported scheduled base backups and keep WAL/PITR observable.
3. Get backup timestamp greater than zero.
4. Restore into a non-live namespace or separate cluster and prove the app can connect.
5. Run a controlled failover or switchover drill and record observed RTO/RPO.
6. Smoke-test Alertmanager delivery, not only VMRule evaluation.
7. Keep GitHub branch/ruleset protection under review as workflow ownership matures; CODEOWNERS
   reviews are intentionally not required yet to avoid blocking the current solo operator flow.

P1:

1. Add scheduled full browser/API synthetic journey metrics and cleanup lifecycle.
2. Add SLO/error-budget dashboard rows and burn-rate alerts with low-traffic guardrails.
3. Add DB query/pool-wait/error and auth-hash saturation metrics.
4. Add security rejection counters for CSRF/origin/body/content-type/validation failures.
5. Triage stale issues and close work that is already complete.

## Claude Code Usage

Purpose: independent architecture, observability, database, migration, and workflow review.

Prompt/task given: review browser access, Grafana/Argo connectivity, PostgreSQL/CNPG HA and backup
gaps, SRE/business metrics, PostgreSQL migration policy, and waste-control for Password Vault.

Summary of output:

- Confirmed the same real-data blockers: no completed base backup, no restore/PITR drill, no
  failover drill, and no proven Alertmanager delivery.
- Confirmed that the edge-routing approach is correct, while client-side MacBook verification and a
  scheduled edge-route synthetic check remain open.
- Recommended an ADR for edge routing versus Kubernetes/LXD `LoadBalancer` browser URLs; this report
  cycle added `docs/adr/0006-browser-access-edge-routing.md`.
- Recommended treating old agent reports as evidence-only and keeping canonical docs/runbooks as
  current truth.

Accepted suggestions:

- Keep P0 focused on durability and alert delivery.
- Add an ADR for browser access through edge routing.
- Defer UI polish, browser extension, mobile clients, OTel tracing, and advanced auth work until
  the durability gates are closed.

Corrected suggestions:

- Claude could not access the infrastructure worktree from its sandbox and reported it unavailable.
  Codex verified the infrastructure worktree directly during this session.
- Claude called out default Helm values where `networkPolicy.enabled=false`,
  `vmServiceScrape.enabled=false`, and soft topology spread are present. Those are chart defaults;
  the current production values enable NetworkPolicy, enable `VMServiceScrape`, override database
  egress to `cnpg.io/cluster=password-vault-cnpg`, and use `DoNotSchedule`.
- Claude noted that product repository grep does not contain `VMRule`. That is expected because
  product alert rules are infrastructure-owned and currently live in the infrastructure GitOps
  path.

Deferred suggestions:

- Full report-sprawl consolidation is useful, but it should be a separate docs-cleanup pass after
  the P0 gates are scheduled.

## Agent Review

Accepted from the database/CNPG subagent:

- migrations remain required even with stable PostgreSQL;
- CNPG does not conflict with HiringTrace if product boundaries stay isolated;
- backup, restore, failover, and alert delivery are the current P0 blockers.

Accepted from the observability/SRE subagent:

- current observability is useful L0/L1 but not mature SRE yet;
- `PasswordVaultCnpgBackupMissing` is a valid gate, not noise;
- product counters already support useful business/product SLIs;
- missing telemetry should not always be hidden behind `or vector(0)`.

Accepted from the stabilization subagent:

- Keep the current static browser MVP rather than starting a React/Vite redesign before durability
  and security gates are stable.
- Treat GitHub branch/ruleset protection and stale issue triage as P0 control-plane work.
- Keep legacy PostgreSQL cleanup after backup/restore/failover evidence, not before.

## Commands Run

Representative commands, with sensitive values redacted:

```bash
curl -k -I https://<mini-pc-lan-ip>:11443/
curl -k -I https://<mini-pc-lan-ip>:3000/
curl -k -I https://<mini-pc-lan-ip>:9443/
KUBECONFIG=<redacted-path> kubectl -n argocd get applications prod-root password-vault -o wide
KUBECONFIG=<redacted-path> kubectl get svc -A -o wide
KUBECONFIG=<redacted-path> kubectl get clusters.postgresql.cnpg.io -A -o wide
KUBECONFIG=<redacted-path> kubectl get backups.postgresql.cnpg.io,scheduledbackups.postgresql.cnpg.io -A -o wide
KUBECONFIG=<redacted-path> kubectl -n password-vault describe cluster.postgresql.cnpg.io/password-vault-cnpg
KUBECONFIG=<redacted-path> kubectl -n password-vault get pods,pvc -o wide
gh api repos/ded-isshin/password-vault/branches/main/protection
gh api repos/ded-isshin/password-vault/rulesets
gh api repos/ded-isshin/password-vault/vulnerability-alerts -i
```

Grafana/VictoriaMetrics checks included:

- `sum(up{job="password-vault-api"})`
- `sum(up{job="password-vault-cnpg"})`
- `max(probe_success{job="password-vault-blackbox",service="password-vault",probe="internal-readyz"})`
- `max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"})`
- `sum(ALERTS{alertname=~"PasswordVault.*",alertstate=~"pending|firing"}) by (alertname,alertstate,severity)`

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- CloudNativePG backup documentation:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- PostgreSQL versioning policy:
  <https://www.postgresql.org/support/versioning/>
- PostgreSQL `ALTER TABLE` documentation:
  <https://www.postgresql.org/docs/current/sql-altertable.html>
- PostgreSQL `CREATE INDEX` documentation:
  <https://www.postgresql.org/docs/current/sql-createindex.html>
- Kubernetes Service documentation:
  <https://kubernetes.io/docs/concepts/services-networking/service/>
- NGINX reverse proxy documentation:
  <https://docs.nginx.com/nginx/admin-guide/web-server/reverse-proxy/>
- NGINX TCP/UDP load balancing documentation:
  <https://docs.nginx.com/nginx/admin-guide/load-balancer/tcp-udp-load-balancer/>
