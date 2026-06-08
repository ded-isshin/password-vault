# Runtime Access, PostgreSQL, Observability, And Migration Review

Status: current point-in-time report. Date: 2026-06-08.

## Active Context

- Active repositories:
  - `password-vault` for product code, Helm chart, CI, and product documentation.
  - `infrastructure-home` read-only for GitOps/runtime inspection.
- Repositories explicitly out of scope:
  - unrelated product repositories;
  - `hiringtrace-site-archive`.
- Risk level: high. This work touches deployment safety, observability, database durability, and
  migration policy for a password-manager product.

## Goal

Verify the current browser access model, Grafana/Argo CD health, PostgreSQL HA posture, migration
need, and the next stable-MVP tasks. Keep the report public-safe and avoid private network details.

## Current Findings

### Browser Access

Grafana, Argo CD, and Password Vault are published for normal LAN browser access through the mini-PC
edge NGINX ports:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Do not give a MacBook the Kubernetes/LXD `LoadBalancer` address as the default browser target. Those
addresses are part of the cluster/LXD routing path and may be unreachable from a normal client
without an explicit route or VPN.

Verified from the mini-PC:

- edge NGINX is listening on the LAN-facing service ports;
- Grafana `/api/health` returned HTTP 200 on the LAN edge port;
- Argo CD `/healthz` returned HTTP 200 on the LAN edge stream port;
- Password Vault `/healthz` and `/readyz` returned HTTP 200 on the LAN edge port;
- the same edge ports also answered HTTP 200 for browser entry pages from the mini-PC side.

Client-side verification from the MacBook is still a separate check because local success on the
mini-PC proves the edge listener and upstreams, not the MacBook route, firewall, or VPN state.

### Argo CD And Deployment State

Read-only Kubernetes inspection showed:

- all cluster nodes are `Ready`;
- `prod-root` is `Synced` and `Healthy`;
- `password-vault` is `Synced` and `Healthy`;
- the API Deployment has three ready replicas;
- the API pods are spread across the worker nodes;
- the API image is pinned by immutable digest in the live Deployment.

### Grafana And Metrics

Grafana MCP inspection showed:

- datasource `VictoriaMetrics` is configured and default;
- dashboard `Password Vault Overview` exists and is provisioned;
- the dashboard has six panels: scrape targets up, 5xx ratio, request rate, p95 request duration,
  pending requests, and unmatched 404 rate.

Live datasource checks returned data:

- expected API scrape targets up: `3`;
- 5xx ratio: `0`;
- request-rate data for `/healthz` and `/readyz`;
- p95 latency data for `/healthz` and `/readyz`;
- pending requests: `0`;
- unmatched 404 rate: `0`.

Important limitation: current latency and traffic data are mostly health/readiness/scrape traffic.
That proves collection and dashboard wiring, not the real product journey. Vault write/read/sync and
auth funnel metrics still need product instrumentation and synthetic user journeys.

Grafana image rendering is not installed. Automated PNG evidence through Grafana MCP is therefore
not available yet; dashboard verification currently relies on live queries and browser access.

No Password Vault-specific `VMRule` objects were found in the cluster. Platform-level
VictoriaMetrics/Kubernetes alert rules exist, but product SLO, target-down, burn-rate, latency, and
security alert rules are still missing.

### PostgreSQL HA

There is no direct conflict with another product database:

- Password Vault and the other product use separate namespaces and separate PostgreSQL StatefulSets.
- No CloudNativePG `Cluster`, `Backup`, or `ScheduledBackup` resources are active.
- CloudNativePG CRDs are present, but no active CloudNativePG operator Deployment was found in the
  runtime scan.

The current Password Vault database is still a single PostgreSQL StatefulSet. That is acceptable for
preview and demo only. It is not acceptable for real password-vault secrets because a single
database pod with node-local storage has no database-level failover target.

Recommended next data-plane direction remains:

- install or restore the CloudNativePG operator through GitOps;
- create a product-specific three-instance PostgreSQL cluster;
- spread instances across worker nodes;
- use synchronous replication with required durability for real user data unless failure testing
  forces a documented risk acceptance;
- add WAL archiving, scheduled base backups, restore drill, failover drill, and alerts before real
  secrets are accepted.

The current problem is not a schema conflict with another product. The conflict risk would come from
sharing another product's database, secret, PVC, backup prefix, or migration target. The safer model
is a shared platform operator and product-owned PostgreSQL resources.

### Migration Policy

Stable PostgreSQL versions do not remove application schema migrations.

PostgreSQL provides a stable database engine. It does not create or evolve the Password Vault
application schema: auth records, MFA state, device/session tables, encrypted vault metadata,
revision constraints, sync cursors, and indexes are application-owned.

The policy should be:

- no normal startup migrations for real-user API pods;
- controlled migration job before rollout when a schema change is required;
- expand/contract only;
- backup/WAL state checked before real-user migrations;
- no same-release destructive drops or renames for live data;
- rollback compatibility recorded before merge.

The current product release contains a controlled migration runner and an opt-in Argo CD Helm
migration hook. Local validation proved the API image can run migrations first and then start the
server with startup migrations disabled. The live GitOps rollout also completed the migration Job
successfully before the API rollout.

Independent reviews found no blocking issues. Accepted fixes from review:

- `ttlSecondsAfterFinished` is unset by default so Kubernetes TTL cleanup does not remove hook
  evidence before the next sync.
- The chart now rejects `migrations.job.enabled=true` unless the Argo hook is also enabled, avoiding
  repeated apply failures against an immutable completed Job pod template.
- The migration Job sets `automountServiceAccountToken: false`.
- Helm CI assertions now check the migration arg, secret ref, restart policy, hook delete policy,
  sync wave, service-account-token setting, and default absence of Job TTL.
- Container CI now checks that invalid CLI use and invalid migration configuration fail closed.

Deferred hardening:

- Add a dedicated egress NetworkPolicy for the migration Job after the database service contract is
  finalized.
- Add a cluster-side PreSync evidence check when the GitOps values enable the hook.

## Stable MVP Backlog

P0 means it blocks a credible stable MVP.

| Priority | Task | Why |
| --- | --- | --- |
| P0 | Prove browser access from the MacBook/client path for Password Vault, Grafana, and Argo CD. | Mini-PC local `200` responses do not prove client routing, TLS warning handling, or browser login usability. |
| P0 | Implement browser vault unlock and encrypted item CRUD/sync with revision conflict checks. | This is the core product, not optional feature work. |
| P0 | Add product-specific application metrics: build info, DB pool/wait/query metrics, auth/MFA step duration, rate-limit, CSRF/security rejection counters. | Golden Signals alone do not prove password-manager correctness or abuse resistance. |
| P0 | Add SLO/burn-rate and target-down alerts after live metrics exist. | A dashboard without actionable alerts is not enough for operations. |
| P0 | Replace preview PostgreSQL with a product-specific CloudNativePG cluster plus backup/restore/failover drills. | Real secrets require durable write survival and recovery evidence. |
| P0 | Add external synthetic journey probes from a client-equivalent route. | Internal scrape health is not the same as "a user can reach and use it." |
| P0 | Add NetworkPolicy or a separate internal-only metrics listener before real secrets. | Edge `/metrics` is blocked, but internal service metrics remain reachable from the cluster network path. |
| P1 | Add protected-activation business metrics: registration finish, TOTP confirm, login-to-session, first vault item created. | These are meaningful product readiness metrics without exposing user identity. |
| P1 | Add realistic load tests for register/login/MFA/vault write/read once those flows exist. | Health-only load tests are smoke tests, not product load tests. |
| P1 | Decide Grafana renderer versus browser automation for dashboard visual evidence. | Current MCP cannot render dashboard PNGs. |

## GitHub Control Plane Hygiene

Read-only GitHub triage found several open MVP issues that are stale or partially done after recent
rollouts, plus four open Dependabot PRs with failing checks. This is operational noise:

- stale issues can cause agents to re-plan already completed work;
- failing dependency PRs should be handled from a clean baseline, one at a time;
- completed work should be reflected in issue comments, labels, or closures before spawning more
  implementation agents.

Recommended hygiene sequence:

1. Update or close issues for completed Helm/GHCR/GitOps/migration slices.
2. Keep the MVP epic focused on remaining blockers: vault CRUD/sync, HA database, backup/restore,
   product metrics/alerts, NetworkPolicy, and synthetic journeys.
3. Defer Dependabot PRs until the main branch and release path remain green after the next product
   slice, then process them individually.

## Work To Defer Or Drop For Now

- Plugin architecture and third-party integrations.
- KeePass/KDBX import.
- Chrome extension implementation.
- Mobile clients.
- Organization/team vaults.
- Advanced growth analytics beyond protected activation and core reliability.
- UI polish beyond a usable browser MVP.

These can return after login, vault CRUD/sync, HA database, backup/restore, alerts, and synthetic
journeys are proven.

## Waste Reduction Notes

- Keep agent reports as evidence logs, not parallel sources of truth.
- Update canonical docs after accepting findings: MVP plan, API contract, ADRs, runbooks, and
  observability plan.
- Use one writer for a scoped implementation branch and one report-only reviewer for high-risk
  work.
- Let Claude Code finish full architecture/security reviews, then triage findings as accepted,
  rejected, corrected, or deferred.
- Do not spawn broad analysis agents unless their output maps to a P0 gate or a user-facing MVP
  feature.
- Treat claims as one of `observed`, `configured`, `tested`, or `production-ready`; do not collapse
  these into "done."
- Require evidence for high-risk claims: command, context, timestamp or commit/PR, and short output
  summary.
- Use one active writer per file or disjoint write scopes for implementation agents. Reviewers and
  advisors should be report-only unless assigned a bounded patch.
- Keep a small canonical current-state section in the MVP plan; do not keep copying stale point-in-
  time report text forward.

## Independent Review Notes

Codex subagent reviewer `Linnaeus` completed a report-only review. Accepted findings:

- The current PostgreSQL deployment is the main blocker before real secrets.
- CNPG CRDs are not equivalent to a running operator and product `Cluster`.
- Current dashboard data proves scrape/dashboard wiring, not real product Golden Signals.
- Missing Password Vault VMRules and NetworkPolicy are real stabilization gaps.
- GitHub issue and Dependabot noise should be cleaned before broad new agent work.

Claude Code advisor was invoked as report-only. The first read-only filesystem run stalled and was
stopped after it produced no output. A second bounded no-tools run completed and independently
confirmed the core findings:

- `3/3` API replicas do not make the product highly available while all writes depend on one
  PostgreSQL pod.
- CNPG CRDs are not equivalent to a reconciled CNPG operator and product `Cluster`.
- A healthy dashboard from health/readiness traffic is not proof of product Golden Signals.
- Product-specific VMRules, NetworkPolicy, and real browser/TLS/auth checks remain required.
- Agent recommendations must distinguish desired manifests from running reconciled state.

Accepted Claude additions:

- Treat API replica health and database HA as separate reliability claims.
- Add explicit "CRD exists" versus "controller reconciles" checks to future Kubernetes reviews.
- Consider image digest pinning for database images when the production database path is finalized.

Corrected or deferred Claude additions:

- Preview PostgreSQL is a blocker before real secrets, but data migration from preview PostgreSQL is
  only needed if preview data must be preserved. A clean CNPG cutover is acceptable while the
  preview contains no real secrets.

## Validation

Tested:

- YAML workflows parsed successfully.
- Helm chart linted successfully.
- Helm rendered the default chart successfully.
- Helm rendered the opt-in migration Job with Argo CD PreSync annotations successfully.
- Rust `cargo fmt --all -- --check`, `cargo test --locked --workspace`, and
  `cargo clippy --locked --workspace --all-targets -- -D warnings` passed in a Rust container.
- Docker smoke built the API image, started disposable PostgreSQL, ran `password-vault-api migrate`,
  started the API with startup migrations disabled, and verified `/readyz` and `/healthz`.
- Public-safety scan over the changed product repository files returned no private kubeconfig path,
  private cluster IP pattern, or GitHub-token pattern matches outside the security workflow regex.
- Read-only Kubernetes checks verified Argo CD app health, Password Vault pod readiness, and service
  state.
- Grafana MCP queries verified that dashboard PromQL expressions return live data.
- Live GitOps rollout verified that the Argo CD PreSync migration Job completed successfully before
  the API Deployment ran the new image.
- Read-only checks verified no Password Vault-specific `VMRule`, no `NetworkPolicy` in the
  `password-vault` namespace, no active CNPG `Cluster`, and no running CNPG operator/controller
  matching the expected names.
- GitHub CLI read-only triage listed open issues, open PRs, and recent workflow status.

Not tested:

- MacBook-to-mini-PC browser route from the MacBook side.
- CloudNativePG database rollout, backup, restore, or failover.
- Product synthetic journey for register/login/MFA/unlock/write/read/sync, because vault CRUD/sync
  is not implemented yet.
- Grafana dashboard PNG rendering, because the renderer is not installed.
- MacBook browser TLS/auth/UI behavior for Grafana and Argo CD.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- Kubernetes Jobs:
  <https://kubernetes.io/docs/concepts/workloads/controllers/job/>
- Argo CD sync phases and waves:
  <https://argo-cd.readthedocs.io/en/stable/user-guide/sync-waves/>
- Argo CD resource hooks:
  <https://argo-cd.readthedocs.io/en/stable/user-guide/resource_hooks/>
- CloudNativePG 1.29:
  <https://cloudnative-pg.io/docs/1.29/>
- CloudNativePG architecture:
  <https://cloudnative-pg.io/docs/1.29/architecture/>
- CloudNativePG backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- PostgreSQL `ALTER TABLE`:
  <https://www.postgresql.org/docs/current/sql-altertable.html>
- PostgreSQL `CREATE INDEX`:
  <https://www.postgresql.org/docs/current/sql-createindex.html>
