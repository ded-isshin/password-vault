# Session Report: Runtime CNPG And SRE Stabilization Refresh

Status: evidence report. Date: 2026-06-08.

## Goal

Refresh the current runtime truth after the CloudNativePG API cutover, answer browser-access and
PostgreSQL questions, reduce stale documentation drift, and identify the smallest stability-first
MVP queue.

## Active Context

- Product repository: `password-vault`.
- Infrastructure context: read-only runtime checks and existing GitOps state for Password Vault.
- Public safety: concrete home-network IPs, private hostnames, kubeconfig paths, tokens, and secret
  values are intentionally omitted.

## Verified

- Browser-facing edge endpoints respond from the mini-PC for Password Vault, Grafana, and Argo CD.
- Kubernetes/LXD service addresses are internal routing details and are not the default browser URLs
  for a MacBook without a route into that network.
- `kubectl` in a fresh shell needs the production kubeconfig set explicitly; without it, it falls
  back to `localhost:8080`.
- Argo CD reports `prod-root` and `password-vault` as `Synced` and `Healthy`.
- The Password Vault API Deployment has `3/3` ready replicas and uses an immutable GHCR digest.
- The API reads `PV_DATABASE_URL` from the `password-vault-cnpg-app` application Secret.
- API egress NetworkPolicy now targets `cnpg.io/cluster=password-vault-cnpg`.
- `password-vault-cnpg` has three ready PostgreSQL 18.4 instances across worker nodes.
- PostgreSQL reports `synchronous_commit=on`, `synchronous_standby_names=ANY 1 (...)`, and two
  streaming quorum standbys.
- VictoriaMetrics reports API scrape targets `3`, CNPG scrape targets `3`, streaming replicas `2`,
  and replication lag `0`.
- Backup availability remains `0`; this is the main red gate before real secrets.
- WAL archiver activity is present and failure count is `0`, but WAL activity is not a substitute
  for a successful base backup and restore drill.
- No `PasswordVault.*` alerts were firing during the final check.

## Documentation Updated

- `README.md`
- `docs/architecture.md`
- `docs/mvp-implementation-plan.md`
- `docs/observability-sre-metrics.md`
- `docs/runbooks/release-and-rollout.md`
- `docs/development.md`
- `docs/foundational-decisions.md`
- `docs/whitepaper.md`
- `docs/decision-briefs/2026-06-08-postgresql-ha-migrations-stability.md`
- `docs/agent-reports/2026-06-08-grafana-argo-postgres-sre-waste-review.md`
- `.github/workflows/container.yml`
- `.github/workflows/load.yml`
- `.github/workflows/rust.yml`

## Key Decisions

- The current MacBook browser path should use the mini-PC LAN edge address plus published ports,
  not Kubernetes/LXD service IPs.
- The API is already cut over to the product-owned CloudNativePG cluster; docs that still described
  a future cutover were stale and were updated.
- Clustered PostgreSQL is still required for the product, but replication is not enough. Backup,
  restore, failover, and alert delivery are the next hard gates.
- PostgreSQL 18 is the current runtime major version, so CI PostgreSQL service containers were
  aligned to `postgres:18-bookworm`.
- Schema migrations remain necessary even on a stable PostgreSQL engine. The target is deliberate,
  reviewed, backward-compatible migrations, not no migrations.
- Agent reports remain historical evidence. Current truth belongs in the MVP plan, observability
  plan, ADRs, and runbooks.

## Claude Code Usage

Purpose: independent architecture, SRE, PostgreSQL, and workflow review.

Prompt/task given: review current Password Vault runtime state, browser connectivity, PostgreSQL
HA/conflicts, SRE metrics, schema migrations, and waste reduction in report-only mode.

Summary of output:

- Confirmed the main blocker before real secrets is durability, not broad feature volume.
- Recommended P0 work on backup, restore/failover drills, and alert enforcement.
- Recommended P1 work on external synthetic pass/fail metrics, DB latency/saturation metrics,
  network hardening, and legacy PostgreSQL cleanup after restore evidence.
- Flagged documentation/report sprawl and required direct file/line evidence for future agent work.
- Flagged a KDF documentation nit: current browser MVP uses PBKDF2 through WebCrypto while Argon2id
  remains the target.

Accepted suggestions:

- Keep P0 focused on backup, restore/failover, and alert delivery.
- Add KDF cross-reference for PBKDF2 current profile versus Argon2id target.
- Keep legacy PostgreSQL cleanup after restore evidence, not before.
- Keep canonical docs as the source of current truth and mark older same-day reports as historical
  when they record pre-cutover state.

Corrected suggestions:

- Claude stated that WAL archiving was absent because no `backup:` block was present in the GitOps
  manifest. Live PostgreSQL showed `archive_mode=on`, an active CNPG archive command, archived WAL
  count greater than zero, and failed count `0`. The accepted finding is narrower: backup
  availability is `0`, no successful base backup is recorded, and restore/failover evidence is
  missing.

Deferred suggestions:

- Default-deny and deeper network hardening remain P1 because current API and metrics NetworkPolicy
  already cover the main preview path.
- Legacy PostgreSQL object removal remains deferred until backup/restore evidence and rollback-window
  closure.

## Validation

Commands and checks run, with sensitive details redacted:

```bash
curl -kfsS https://<mini-pc-lan-ip>:<password-vault-port>/ >/dev/null
curl -kfsS https://<mini-pc-lan-ip>:<grafana-port>/api/health >/dev/null
curl -kfsS https://<mini-pc-lan-ip>:<argocd-port>/healthz >/dev/null
KUBECONFIG=<redacted-path> kubectl -n argocd get applications.argoproj.io prod-root password-vault -o wide
KUBECONFIG=<redacted-path> kubectl -n password-vault get cluster password-vault-cnpg
KUBECONFIG=<redacted-path> kubectl -n password-vault get deploy password-vault-api
KUBECONFIG=<redacted-path> kubectl -n password-vault exec password-vault-cnpg-1 -c postgres -- psql ...
docker manifest inspect postgres:18-bookworm
python3 -c '<yaml parse workflow files>'
node --check crates/api/static/app.js
node --check load/synthetic/browser-api-journey.mjs
git diff --check
```

Grafana/VictoriaMetrics queries verified:

- `sum(up{job="password-vault-api"}) or vector(0)` returned `3`.
- `sum(up{job="password-vault-cnpg"}) or vector(0)` returned `3`.
- `max(cnpg_pg_replication_streaming_replicas{job="password-vault-cnpg"}) or vector(0)` returned
  `2`.
- `max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"}) > bool 0 or
  vector(0)` returned `0`.
- `ALERTS{alertname=~"PasswordVault.*",alertstate="firing"}` returned no firing alerts.

## Not Tested

- MacBook-side browser connection was not directly tested from the MacBook.
- Alertmanager delivery was not smoke-tested.
- Backup/restore/failover drills were not run.
- Full Rust test suite was not rerun for this docs/workflow refresh.

## Next Steps

1. Implement CloudNativePG backup/base-backup configuration and schedule through GitOps.
2. Run restore and failover drills and record observed RTO/RPO.
3. Add backup freshness and WAL archive failure alerting with delivery smoke evidence.
4. Add scheduled external synthetic journey pass/fail metrics and cleanup lifecycle.
5. Remove legacy preview PostgreSQL after restore evidence and rollback-window closure.
6. Continue issue triage so stale completed work does not drive duplicate agent tasks.

## Sources

- Google SRE Book: Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook: Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- Google SRE Workbook: Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- CloudNativePG 1.29 backup documentation:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG 1.29 replication documentation:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- PostgreSQL 18 `ALTER TABLE` documentation:
  <https://www.postgresql.org/docs/current/sql-altertable.html>
- PostgreSQL 18 `CREATE INDEX` documentation:
  <https://www.postgresql.org/docs/current/sql-createindex.html>
