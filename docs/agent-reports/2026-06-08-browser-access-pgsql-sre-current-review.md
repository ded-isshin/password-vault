# Agent Report: Browser Access, PostgreSQL, SRE, And Waste Review

Date: 2026-06-08.

Status: current-state review. Public-safe: concrete home-network addresses, tokens, cookies,
kubeconfig paths, and secrets are intentionally omitted or represented as placeholders.

## Goal

Answer the current stabilization questions:

- why a MacBook should not use Kubernetes/LXD `LoadBalancer` addresses as browser URLs;
- whether Grafana, Argo CD, and Password Vault are alive through the mini-PC edge path;
- whether Password Vault needs clustered PostgreSQL and whether it conflicts with HiringTrace;
- which observability signals matter under Google SRE guidance;
- why database migrations remain necessary on stable PostgreSQL;
- what work should be deleted, deferred, or tightened to reduce wasted agent effort.

## Active Context

Active repositories:

- `password-vault` for product docs and runtime interpretation;
- `infrastructure-home` GitOps worktree for edge, Argo CD, observability, and CNPG facts.

Out of scope:

- modifying firewall, router, NGINX, Kubernetes, or database state;
- deleting legacy PVCs or Secrets;
- accepting real user secrets.

Risk level: medium for analysis, high for any future edge/firewall/database backup changes.

## Verified Runtime Facts

Mini-PC edge checks from the mini-PC returned HTTP 200 for:

- Password Vault `/` and `/readyz`;
- Grafana `/` and `/api/health`;
- Argo CD `/` and `/healthz`.

Read-only host inspection showed the Password Vault, Grafana, and Argo CD edge ports listening on
all host interfaces. This confirms that the edge path is alive, but it does not prove that the ports
are restricted to the intended LAN/VPN client paths.

Kubernetes and Argo CD state:

- `prod-root`, `password-vault`, `observability-vm-stack`, `data-cloudnative-pg`, and
  `data-plugin-barman-cloud` were `Synced` and `Healthy`;
- Password Vault API had three ready replicas;
- Password Vault CNPG had three ready PostgreSQL 18.4 instances on three workers;
- HiringTrace used a separate PostgreSQL `StatefulSet` in a different namespace.

Database verification:

- the active CNPG primary reported `synchronous_commit=on`;
- `synchronous_standby_names` required any one standby;
- both standbys were `streaming` with `sync_state=quorum`;
- no `Backup` or `ScheduledBackup` resources existed for Password Vault;
- the default storage class was `local-path` with node-local RWO volumes and no expansion;
- a legacy preview PostgreSQL PVC, legacy preview database Secrets, and an old completed migration
  Job still existed as cleanup debt.

Grafana/VictoriaMetrics verification:

- datasource `VictoriaMetrics` was reachable through the Grafana MCP integration;
- dashboard UID `password-vault-overview` existed with 26 panels;
- `sum(up{job="password-vault-api"})` returned `3`;
- `sum(up{job="password-vault-cnpg"})` returned `3`;
- internal and edge black-box readiness probes returned `probe_success=1`;
- API p95 latency was in the single-digit millisecond range during the checked low-traffic window;
- live synthetic runs generated registration, login, TOTP, recovery, vault-item, and sync counters;
- the only Password Vault firing alert was `PasswordVaultCnpgBackupMissing`.

## Conclusions

### Browser Access

The correct MacBook/browser path is the mini-PC LAN edge address with the documented edge ports.
Kubernetes/LXD `LoadBalancer` addresses are backend service-routing details and are not expected to
work from a normal MacBook unless the client has explicit routing into that network.

If mini-PC `curl -k` checks pass but a MacBook browser fails, the first investigation path is:

1. confirm the MacBook is on the same LAN or intended VPN path;
2. confirm the browser uses `https`;
3. expect a self-signed certificate warning in the current preview;
4. check client-side firewall/VPN/router reachability;
5. only then inspect Kubernetes Services or Argo CD.

The current edge path is a preview path. Before real secrets, access control must be explicitly
versioned and verified as LAN/VPN-only for Password Vault, Grafana, and Argo CD, and the certificate
model must move away from ad-hoc self-signed trust.

### PostgreSQL

Clustered PostgreSQL is justified for a password manager. A single PostgreSQL pod can be acceptable
for preview or disposable demos, but it is not enough for acknowledged password writes.

The current CNPG design is the right direction:

- one product-owned cluster in the product namespace;
- three PostgreSQL instances;
- synchronous quorum replication requiring one standby;
- no reuse of HiringTrace database, Secrets, PVCs, or migrations.

There is no inherent conflict with HiringTrace. The shared layer is the operator/platform, not the
product data plane.

The current blocker is not replication. The blocker is durability beyond the worker-local volumes:
object-store backed base backup, WAL/PITR evidence, restore drill, and failover drill are still
missing.

### Migrations

Stable PostgreSQL versions do not eliminate application schema migrations. PostgreSQL stability
means the engine behavior is supported and predictable; it does not create or evolve
Password Vault's accounts, MFA state, encrypted key-wrap metadata, constraints, indexes, or vault
sync schema.

The target is not frequent migrations. The target is:

- few migrations;
- no speculative schema churn;
- immutable migration files after real data exists;
- expand/contract changes for live compatibility;
- one controlled GitOps migration job, not API pods racing on startup;
- backup/restore evidence before destructive or high-lock schema changes.

### Observability

The current dashboard and alert rules are useful for an MVP and follow the SRE shape:

- latency: HTTP p95, readiness DB pool wait/query latency;
- traffic: request rate and product operation counters;
- errors: 5xx, auth/MFA failures, rate limits, DB errors;
- saturation: pending requests, DB pool pressure, replication lag, backup age.

The important product/business signals are not marketing numbers. They are whether a user can:

- become protected: registration, TOTP confirmation, recovery codes, first encrypted item;
- return and unlock: login proof, MFA, session upgrade, vault unlock;
- save and sync: encrypted item write, revision validation, sync read/decrypt;
- recover safely: recovery-code login followed by TOTP re-enrollment;
- avoid data loss: fresh backup, WAL archive, restore drill, failover drill.

The largest observability gap is delivery and external journey proof. Alerts exist, but alert
routing must be verified. The synthetic journey exists and has passed manually, but scheduled
pass/fail metrics are not yet deployed.

### Waste Reduction

The biggest waste source is re-deriving settled analysis across many dated reports. Going forward:

- update the canonical document for the topic first;
- use agent reports only as evidence snapshots;
- do not add dashboards or metrics that do not answer an incident, SLO, security, product, or
  release-gate question;
- do not create migrations for speculative future product ideas;
- do not install or keep platform components indefinitely unless they are wired to a real control.

## Claude Code Usage

Purpose: independent architecture/SRE/security review.

Prompt/task given: read-only inspection of product and infra worktrees, focused on browser access,
CNPG, migrations, SRE metrics, wasted work, and minimal stabilization gates.

Summary of output:

- confirmed the LAN edge vs internal LXD/Kubernetes address distinction;
- agreed that CNPG is justified and isolated from HiringTrace;
- flagged backup/PITR/failover and alert delivery as hard gates;
- agreed that migrations are still necessary but should be rare and controlled;
- flagged edge exposure control and self-signed TLS as gates before real secrets;
- flagged documentation sprawl and idle platform foundation as waste risks.

Accepted suggestions:

- add edge lockdown and trusted TLS to the stabilization gates;
- keep Alertmanager delivery and backup/restore/failover ahead of feature volume;
- keep canonical docs authoritative and stop re-deriving settled analysis;
- treat legacy preview PostgreSQL artifacts as cleanup debt after backup/restore evidence.

Rejected or qualified suggestions:

- Claude inferred some edge exposure risk from manifests only. Live external reachability from
  outside the LAN was not tested in this session, so the conclusion is a risk requiring verification,
  not proof of public exposure.
- Claude treated WAL archiving as not wired because no object-store backup config exists in the
  product cluster manifest. Runtime archiver counters showed zero failures, but base backup
  availability still returned zero. The accepted conclusion is narrower: no usable base backup/PITR
  evidence exists yet.

## Commands And Checks Run

Representative read-only checks:

```bash
kubectl -n argocd get applications -o wide
kubectl get svc -A -o wide
kubectl -n password-vault get pods -o wide
kubectl -n password-vault get cluster,backup,scheduledbackup
kubectl get backups.postgresql.cnpg.io,scheduledbackups.postgresql.cnpg.io -n password-vault
kubectl get vmprobe,vmrule,vmservicescrape,vmpodscrape -A
kubectl -n password-vault get pvc,secrets,jobs
kubectl get storageclass
curl -k -I https://<mini-pc-lan-ip>:<password-vault-port>/
curl -k -I https://<mini-pc-lan-ip>:<grafana-port>/
curl -k -I https://<mini-pc-lan-ip>:<argocd-port>/
ss -ltn
```

Representative PromQL checks:

```promql
sum(up{job="password-vault-api"})
sum(up{job="password-vault-cnpg"})
max by (probe) (probe_success{job="password-vault-blackbox",service="password-vault",probe=~"internal-readyz|edge-readyz"})
histogram_quantile(0.95, sum by (le) (rate(axum_http_requests_duration_seconds_bucket{job="password-vault-api",exported_endpoint!="/metrics"}[5m])))
ALERTS{service="password-vault",alertstate="firing"}
max(cnpg_collector_last_available_backup_timestamp{job="password-vault-cnpg"}) or vector(0)
```

## Files Updated

- `docs/mvp-implementation-plan.md`
- `docs/observability-sre-metrics.md`
- `docs/decision-briefs/2026-06-08-postgresql-ha-migrations-stability.md`
- `docs/runbooks/release-and-rollout.md`
- infrastructure GitOps root README in the infra worktree

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- CloudNativePG backup documentation:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG recovery documentation:
  <https://cloudnative-pg.io/docs/1.29/recovery/>
- CloudNativePG replication documentation:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- PostgreSQL 18 table modification documentation:
  <https://www.postgresql.org/docs/18/ddl-alter.html>

## Next Steps

1. Verify MacBook access with the mini-PC LAN edge URLs and expected self-signed certificate warning.
2. Add explicit edge access-control design and verification for LAN/VPN-only access:
   <https://github.com/ded-isshin/infrastructure-home/issues/146>.
3. Configure and smoke-test Alertmanager delivery:
   <https://github.com/ded-isshin/infrastructure-home/issues/147>.
4. Choose an off-node object-store target and wire CNPG base backups/WAL/PITR:
   <https://github.com/ded-isshin/infrastructure-home/issues/148>.
5. Run a restore drill and failover drill before real user secrets.
6. Add scheduled synthetic journey pass/fail metrics and bounded cleanup.
7. Remove legacy preview PostgreSQL artifacts only after backup/restore evidence or an explicit
   decision that preview data is disposable.
