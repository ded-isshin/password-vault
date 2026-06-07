# Session Report: Browser Access, SRE, PostgreSQL, And Process Refresh

## Goal

Refresh the current facts for browser access to Password Vault, Grafana, and Argo CD; reassess the
PostgreSQL HA and migration posture; sharpen the SRE/Golden Signals plan; and identify process
changes that reduce wasted work and hallucinated progress.

## Active Context

- `password-vault`: product documentation and stabilization backlog.
- `infrastructure-home`: read-only live checks and one documentation consistency fix.

Repositories explicitly out of scope:

- `hiringtrace-site`
- `hiringtrace-site-archive`
- unrelated product repositories

## Verified Current State

- Mini-PC LAN address was observed for browser access and redacted for the public repository.
- LXD/Kubernetes-side service addresses are on `<lxd-loadbalancer-ip>` via `lxcbr0`; they are not the normal
  browser targets for a MacBook unless the MacBook has an explicit route or VPN into that network.
- Edge listeners are present on `0.0.0.0:11443`, `0.0.0.0:3000`, and `0.0.0.0:9443`.
- Password Vault browser preview responds with HTTP 200 at `https://<mini-pc-lan-ip>:11443/`.
- Grafana health responds with HTTP 200 at `https://<mini-pc-lan-ip>:3000/api/health`.
- Grafana dashboard page responds with HTTP 200 at
  `https://<mini-pc-lan-ip>:3000/d/password-vault-overview/password-vault-overview`.
- Argo CD health responds with HTTP 200 at `https://<mini-pc-lan-ip>:9443/healthz`.
- Argo CD applications including `password-vault` and `observability-vm-stack` are `Synced` and
  `Healthy`.
- Password Vault API has three ready replicas and zero restarts in the current rollout.
- The app image is deployed by immutable GHCR digest.
- `PV_RUN_MIGRATIONS_ON_STARTUP=false` in the current deployment.
- `/metrics` is blocked at the Password Vault edge route with HTTP 404.
- `/metrics` is still reachable on the internal application `LoadBalancer` service, and no
  `NetworkPolicy` exists in the `password-vault` namespace.
- Grafana datasource `VictoriaMetrics` exists and is default.
- Dashboard `Password Vault Overview` exists with six panels.
- Live VictoriaMetrics queries returned:
  - `sum(up{job="password-vault-api"}) = 3`
  - request rate data for `/healthz` and `/readyz`
  - 5xx ratio `0`
  - p95 request duration data
- Current password-vault PostgreSQL is one `postgres:17-bookworm` `StatefulSet` replica on a
  `local-path` PVC.
- CloudNativePG CRDs are installed, but there are no active `clusters.postgresql.cnpg.io` resources.
- No CloudNativePG operator pod or deployment was found in the current cluster scan.

## Browser Access Conclusion

The likely MacBook problem is using the wrong address class. Use:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Do not use `<lxd-loadbalancer-ip>` service addresses from a normal LAN browser unless the client has
routing into the LXD/Kubernetes network.

The current TLS certificate is self-signed. Browser certificate warnings are expected.

## PostgreSQL HA Conclusion

The current database is preview-only. It is not acceptable for real password-vault user secrets
because it is a single PostgreSQL pod with node-local storage.

There is no technical need to share another product's database. The correct model is:

- shared CloudNativePG operator as platform machinery;
- separate password-vault PostgreSQL cluster resource;
- separate namespace, users, services, secrets, backup target prefix, restore drill, and runbook;
- no reuse of the HiringTrace database or secrets.

For real secrets, the target remains a three-instance CloudNativePG cluster spread across workers,
preferably with quorum synchronous replication using one synchronous standby and required durability.

## Migration Conclusion

Stable PostgreSQL versions do not remove application schema migrations. PostgreSQL stability means
supported engine behavior and safe minor updates; it does not create or evolve application-owned
tables, constraints, indexes, auth fields, MFA state, encrypted vault metadata, or compatibility
windows.

The target is not "constant migrations." The target is controlled, rare, backward-compatible
migrations:

1. expand schema in a backward-compatible way;
2. deploy code compatible with old and new schema;
3. backfill if needed;
4. verify metrics and invariants;
5. contract only in a later release.

Startup migrations are disabled in the current deployment and should stay disabled for real users.
Schema-changing production releases need an explicit GitOps migration job or reviewed operator step.

## SRE / Observability Direction

The current dashboard proves scraping and basic Golden Signals. It is not yet full product
observability.

Minimum next observability work:

- target-down alert for `password-vault-api`;
- fast 5xx error-budget burn alert;
- p99 latency and pending-request alerts with request-volume guards;
- external synthetic probe through `https://<mini-pc-lan-ip>:11443/`;
- restrictive NetworkPolicy or a separate internal-only metrics listener;
- auth and MFA counters;
- vault write/read/sync counters;
- DB pool, query latency, errors, replica lag, disk, backup age, WAL archive, and restore drill
  metrics;
- release/build info metric and rollout annotations;
- alert delivery test.

Product/business metrics should measure useful readiness, not vanity:

- registration completion ratio;
- protected activation ratio: registration plus MFA plus first encrypted item saved;
- returning access success ratio: login plus MFA plus vault unlock;
- vault write success ratio;
- sync conflict and stale revision rejection rates;
- recovery-code attempt and success rates;
- backup restore confidence: backup age, restore drill age, and failover drill age.

## Minimum Stabilization Backlog

Blocking before real secrets:

1. Finish login finish and login-time TOTP verification.
2. Implement browser unlock plus encrypted vault item CRUD/sync.
3. Add rate limiting and abuse-visible auth/MFA/security counters.
4. Add product-specific VMRule alerts and test alert delivery.
5. Replace preview PostgreSQL with product-specific CloudNativePG HA.
6. Add object-store backup/WAL archiving, restore drill, and failover drill.
7. Add a controlled migration job/runbook.
8. Add external synthetic journeys through the same edge path a browser uses.
9. Keep `/metrics` blocked at the edge and restrict internal `/metrics` access to the scraper path.
10. Install/verify the CloudNativePG operator, not only CRDs, before creating the product cluster.
11. Run load tests against auth, vault write/read, and sync paths once those paths exist.

Non-blocking but important:

- trusted TLS strategy for LAN/browser use;
- Argo CD/Grafana access hardening once dashboards include more sensitive operational context;
- better pod spread for the API when capacity allows;
- cleanup of stale docs and duplicated issue references.

## Process Improvements

To reduce hallucinated or throwaway work:

- Keep one active writer per file or use separate worktrees for writer agents.
- Require agents to produce report-only output unless they are assigned a disjoint write scope.
- Do not mark a task done from a plan line; require command output, test result, or live query.
- Put stale facts behind dated reports instead of copying them forward.
- Before implementation, write the smallest acceptance gate that would prove the slice works.
- For long-running advisors such as Claude Code, record purpose, max runtime, and output location,
  then let the run complete unless it is clearly blocked or unsafe.
- Prefer deleting duplicated docs/issue references over expanding them.
- Keep current-state docs short and link older reports instead of restating old assumptions.

## Claude Code Usage

Purpose: independent architecture/platform/security review.

Prompt/task given: review browser access, PostgreSQL HA, migrations, SRE/Golden Signals, minimum
stabilization backlog, and process improvements. Report only; no edits or commands.

Summary of output:

- Correctly identified the LAN-vs-LXD address issue and self-signed certificate expectation.
- Correctly flagged that the current PostgreSQL deployment is a preview-only single `StatefulSet`.
- Correctly flagged missing product-specific alert rules, restrictive NetworkPolicy, controlled
  migration job, backup/restore/failover drills, and key restore drills.
- Correctly identified stale documentation risk and the need to consolidate accepted report findings
  into the canonical MVP plan.
- Overstated some details: Grafana and Password Vault are HTTP reverse-proxied, while Argo CD is the
  TCP stream proxy path; platform VMRule objects exist, but Password Vault-specific VMRules do not.

Accepted suggestions:

- Add internal metrics exposure and NetworkPolicy to the blocking stabilization queue.
- Verify CloudNativePG operator deployment state separately from CRD existence.
- Treat product-specific alert rules and alert delivery as required before real users.
- Keep migration jobs separate from normal API pod startup.
- Add process rules for stale snapshot claims and report consolidation.

Deferred suggestions:

- Add in-cluster TLS or mTLS between edge and the application.
- Add TLS certificate renewal automation.
- Add a separate ADR for the browser KDF decision only if the current ADR wording remains ambiguous
  after the next documentation cleanup.

Rejected or corrected suggestions:

- Do not describe all three edge paths as TCP stream proxying; only Argo CD uses the stream path in
  the current edge config.
- Do not say there are zero alert rules in the cluster; say there are no Password Vault-specific
  VMRules yet.

## Commands Run

```bash
hostname -I
KUBECONFIG=<redacted-path> kubectl get applications -n argocd -o wide
KUBECONFIG=<redacted-path> kubectl get pods,svc,endpoints -n argocd -o wide
KUBECONFIG=<redacted-path> kubectl get pods,svc,endpoints -n observability -o wide
KUBECONFIG=<redacted-path> kubectl get pods,svc,endpoints -n password-vault -o wide
ss -ltn
curl -sk -D - https://<mini-pc-lan-ip>:11443/
curl -sk -D - https://<mini-pc-lan-ip>:3000/api/health
curl -sk -D - https://<mini-pc-lan-ip>:9443/healthz
curl -sk -D - https://<mini-pc-lan-ip>:11443/metrics
KUBECONFIG=<redacted-path> kubectl get statefulsets,pdb,pvc -n password-vault -o wide
KUBECONFIG=<redacted-path> kubectl get crd
KUBECONFIG=<redacted-path> kubectl get clusters.postgresql.cnpg.io -A -o wide
KUBECONFIG=<redacted-path> kubectl get vmrules -A -o wide
KUBECONFIG=<redacted-path> kubectl get vmservicescrapes -A -o wide
KUBECONFIG=<redacted-path> kubectl get networkpolicy -n password-vault -o wide
KUBECONFIG=<redacted-path> kubectl get pods -A -o wide
KUBECONFIG=<redacted-path> kubectl get deploy -A -o wide
curl -sS http://<lxd-loadbalancer-ip>:8080/metrics
```

Grafana MCP was used for datasource, dashboard, and PromQL checks.

## Sources Consulted

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- CloudNativePG 1.29 Replication:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG 1.29 Backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- PostgreSQL Versioning Policy:
  <https://www.postgresql.org/support/versioning/>

## Validation

Tested:

- Edge HTTP(S) responses from the mini-PC.
- Kubernetes resource health using read-only `kubectl`.
- Grafana datasource and dashboard queries through Grafana MCP.
- Password Vault `/metrics` blocked through the edge.
- Internal `/metrics` currently reachable through the application LoadBalancer path.

Not tested:

- Browser access from the MacBook itself.
- Alert notification delivery.
- CloudNativePG failover.
- Backup/restore.
- Load tests for real auth/vault journeys.
- Login finish/TOTP login branch validation.

## Open Questions

- Whether the MacBook is on the same LAN/subnet and whether browser trust/certificate handling is
  the remaining issue.
- Which object-store/S3-compatible target will be used for CloudNativePG backups.
- Whether Password Vault real-user mode should choose `dataDurability: required` permanently or
  allow a documented degraded-write availability tradeoff later.
