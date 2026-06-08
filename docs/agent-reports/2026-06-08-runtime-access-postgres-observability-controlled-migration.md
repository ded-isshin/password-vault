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
- Grafana `/api/health` returned HTTP 200;
- Argo CD `/healthz` returned HTTP 200;
- Password Vault `/healthz` and `/readyz` returned HTTP 200.

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

Grafana image rendering is not installed. Automated PNG evidence through Grafana MCP is therefore
not available yet; dashboard verification currently relies on live queries and browser access.

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

This product branch now contains a controlled migration runner and an opt-in Argo CD Helm migration
hook. Local validation proved the API image can run migrations first and then start the server with
startup migrations disabled.

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
| P0 | Merge controlled migration runner and enable the GitOps migration hook in production values. | Prevents schema mutation from being a side effect of API pod startup. |
| P0 | Implement browser vault unlock and encrypted item CRUD/sync with revision conflict checks. | This is the core product, not optional feature work. |
| P0 | Add product-specific application metrics: build info, DB pool/wait/query metrics, auth hash pressure, rate-limit, CSRF/security rejection counters. | Golden Signals alone do not prove password-manager correctness or abuse resistance. |
| P0 | Add SLO/burn-rate and target-down alerts after live metrics exist. | A dashboard without actionable alerts is not enough for operations. |
| P0 | Replace preview PostgreSQL with a product-specific CloudNativePG cluster plus backup/restore/failover drills. | Real secrets require durable write survival and recovery evidence. |
| P0 | Add external synthetic journey probes from a client-equivalent route. | Internal scrape health is not the same as "a user can reach and use it." |
| P1 | Add protected-activation business metrics: registration finish, TOTP confirm, login-to-session, first vault item created. | These are meaningful product readiness metrics without exposing user identity. |
| P1 | Add realistic load tests for register/login/MFA/vault write/read once those flows exist. | Health-only load tests are smoke tests, not product load tests. |
| P1 | Decide Grafana renderer versus browser automation for dashboard visual evidence. | Current MCP cannot render dashboard PNGs. |

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

Not tested:

- MacBook-to-mini-PC browser route from the MacBook side.
- Actual Argo CD PreSync execution of the new migration Job in the live cluster.
- CloudNativePG database rollout, backup, restore, or failover.
- Product synthetic journey for register/login/MFA/unlock/write/read/sync, because vault CRUD/sync
  is not implemented yet.
- Grafana dashboard PNG rendering, because the renderer is not installed.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
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
