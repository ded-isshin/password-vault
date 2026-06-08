# Access, PostgreSQL, Observability, And Workflow Stability Analysis

Status: draft public-safe report.
Date: 2026-06-08.

## Active Context

- `password-vault`: product observability code, product docs, stabilization backlog.
- `infrastructure-home`: read-only live Kubernetes/Grafana/Argo checks plus GitOps dashboard intent.

Repositories explicitly out of scope:

- `hiringtrace-site`
- `hiringtrace-site-archive`
- unrelated product repositories

## Goal

Answer the current stabilization questions:

- which browser URLs should be used for Grafana, Argo CD, and Password Vault;
- whether PostgreSQL needs to be clustered and whether another product creates a conflict;
- which MVP tasks matter for a stable minimal product;
- how to apply Google SRE Golden Signals and SLO thinking to technical and product metrics;
- why schema migrations still exist on stable PostgreSQL;
- how to reduce hallucinated or throwaway agent work.

## Browser Access Verification

Verified from the mini-PC:

- the mini-PC has a LAN-facing interface and a separate LXD/Kubernetes-side bridge;
- edge listeners exist on the LAN-facing address for Password Vault, Grafana, and Argo CD;
- `curl -k` from the mini-PC returned HTTP 200 for:
  - Password Vault `/`;
  - Grafana `/api/health`;
  - Argo CD `/healthz`;
- Argo CD reports `password-vault`, `observability-vm-stack`, and the root application as
  `Synced` and `Healthy`;
- Grafana MCP found dashboard `Password Vault Overview` and the `VictoriaMetrics` datasource;
- every existing Password Vault dashboard query was executed against the live datasource and
  returned data or an intentional zero vector.

Conclusion:

- browser clients on the normal LAN should use the mini-PC LAN address plus edge ports;
- do not use Kubernetes/LXD `LoadBalancer` addresses as the default MacBook browser target unless
  the MacBook has an explicit route or VPN into that network;
- if the mini-PC checks pass but the MacBook still cannot connect, debug the MacBook/client network
  path first, not Kubernetes.

Public-safe routes:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

## PostgreSQL HA Finding

Live Kubernetes read-only checks found:

- CloudNativePG CRDs are installed;
- no `clusters.postgresql.cnpg.io` resources exist;
- no `backups.postgresql.cnpg.io` or `scheduledbackups.postgresql.cnpg.io` resources exist;
- no obvious CloudNativePG operator/controller pod was observed by name;
- `password-vault` currently uses one `postgres:17-bookworm` StatefulSet replica on `local-path`
  storage;
- the currently deployed PostgreSQL PDB allows one voluntary unavailable pod, which avoids blocking
  node maintenance but does not make the database HA. The follow-up GitOps intent removes this
  single-replica PostgreSQL PDB to avoid implying availability that does not exist.

Decision:

- clustered PostgreSQL is required before accepting real password-vault secrets;
- the current single PostgreSQL StatefulSet is a preview/demo bridge only;
- the blocker is not a logical conflict with another product;
- the likely blocker is missing product-specific CloudNativePG runtime resources and possibly a
  missing/unfinished CNPG operator installation;
- another product may keep its own database, but `password-vault` must not share another product's
  database, credentials, PVCs, migrations, services, or backup prefix.

Recommended production-like shape:

- one product-specific CloudNativePG `Cluster`;
- three PostgreSQL instances spread across workers;
- one primary plus two replicas;
- quorum synchronous replication with one synchronous standby for real password-manager writes;
- product-specific backup/WAL archive target;
- restore and failover drills before real data;
- no public PostgreSQL exposure.

## Migration Analysis

Stable PostgreSQL does not remove application migrations.

The database engine version controls server behavior and support lifecycle. It does not freeze the
product data model. `password-vault` still owns tables, indexes, constraints, auth/MFA state,
encrypted vault metadata, revision sequencing, and compatibility between old and new application
versions.

What should change:

- migrations should be rare, reviewed, immutable after merge, and tied to real product invariants;
- normal app startup should not run production migrations;
- schema-changing releases should use a controlled migration job or operator step;
- after MVP bootstrap, every migration should include a short risk note covering lock/rewrite risk,
  rollback/restore expectation, and compatibility with the previous app version;
- avoid speculative schema for organizations, sharing, plugins, mobile, and browser extension until
  those API contracts are ready.

Good goal: "few controlled migrations", not "zero migrations".

## Observability Update

Official Google SRE sources emphasize:

- dashboards should answer basic service questions and include the four Golden Signals;
- the Golden Signals are latency, traffic, errors, and saturation;
- user-facing systems should reason about availability, latency, and throughput;
- SLOs should be documented with SLI implementation details, error-budget calculation, rationale,
  and review cadence;
- alerting should be simple, actionable, and tied to user-visible or imminent user-visible impact.

Implemented in this slice:

- product code now exposes low-cardinality metrics for:
  - build/version;
  - registration events and account creation;
  - login starts and login proof outcomes;
  - MFA enrollment/login outcomes;
  - session creation/upgrades;
  - vault item changes;
  - sync requests;
- product tests assert that these metrics are emitted without user/account/vault/item identifiers;
- the infrastructure Grafana dashboard intent now includes product panels for registration, login,
  MFA, vault item changes, sync requests, and build info.

Deployment update:

- product PR #63 was merged and published as API image digest
  `sha256:d86238f09b6034512bb4629ec39bd20536aa84732194f421ebeddcadd2fc349a`;
- infrastructure PR #107 was merged and rolled out through Argo CD;
- `password-vault` and `prod-root` reached `Synced` / `Healthy`;
- the API deployment reached 3/3 ready replicas on the new digest;
- `password_vault_build_info`, `password_vault_registration_events_total`, and
  `password_vault_login_starts_total` were verified in VictoriaMetrics after a synthetic smoke run;
- MFA, vault item, and sync panels still show zero fallback until a fuller synthetic/browser journey
  exercises those paths.

## Minimum Stability Backlog

Do next, before broad features:

1. Add browser-side vault unlock and encrypted item create/read/update/delete/sync workflow.
2. Add external synthetic journey checks from the LAN/browser path: register, MFA, login, unlock,
   create item, sync item.
3. Add target-down and fast 5xx burn-rate alerts.
4. Add product auth/MFA/vault alert thresholds only after live data establishes a baseline.
5. Replace the preview database with product-specific CloudNativePG.
6. Add WAL archiving, scheduled backup, restore drill, and failover drill gates.
7. Add NetworkPolicy or internal-only metrics listener before real-user data.
8. Add DB pool/query/replication/backup metrics and panels.
9. Keep browser extension, mobile, orgs, sharing, import, plugins, billing, and advanced analytics
    out of the MVP stabilization path.

## Workflow Waste Reduction

Observed risk:

- repeated reports can become parallel sources of truth;
- agent output can sound complete before validation or deployment evidence exists;
- broad agent fan-out creates collisions and stale docs;
- some tasks consume time but do not map to an MVP gate.

Updated working rule:

- one writer per branch/scope;
- reviewers and Claude Code should be report-only unless assigned a narrow write scope;
- every agent run needs an output file, max runtime, stop condition, and acceptance gate;
- canonical docs win over dated reports;
- final claims must say one of: `planned`, `implemented locally`, `merged`, `deployed`,
  `verified live`;
- new work is worth doing only if it proves an MVP gate, removes a blocker, reduces security/data
  loss/deployment risk, improves a contract, adds a regression check, or updates an operational
  runbook.

## Claude Code Usage

Purpose: independent architecture/security/observability review.

Prompt/task given: review the current uncommitted product and infrastructure diffs report-only,
with focus on metric cardinality, SRE usefulness, Grafana PromQL correctness, PostgreSQL HA and
migration conclusions, public repository safety, and whether this slice should be kept.

Summary of output:

- No critical or high-severity security issues found.
- Medium: grouped Grafana panels using `or vector(0)` would always add an empty `{}` zero series.
- Low/medium: a PDB on one PostgreSQL replica gives a false HA signal.
- Low: sync metric label `operation="pull"` is a dead dimension.
- Low/deferred: counters are not preinitialized to zero; this matters when alert rules are added.
- Low/deferred: `password_vault_build_info` is set on every scrape; acceptable for MVP.

Accepted suggestions:

- Use `or on() vector(0)` for grouped dashboard panels.
- Remove the constant sync `operation` label.
- Remove the single-replica PostgreSQL PDB from GitOps intent.

Deferred suggestions:

- Preinitialize expected counter label sets before alert rules depend on absent/failure series.
- Move build-info recording from scrape time to startup if it becomes noisy or confusing.

Rejected suggestions:

- None.

## Commands Run

Redacted command classes:

- `hostname -I`, `ip -brief address`, `ip route`;
- `KUBECONFIG=<redacted-path> kubectl get svc,pods,applications,crd,clusters,backups,scheduledbackups`;
- `curl -k` against LAN-facing edge routes;
- Grafana MCP dashboard search, datasource listing, panel-query inspection, and PromQL execution;
- Google SRE official documentation lookup;
- `jq empty` on the dashboard JSON;
- `kubectl kustomize kubernetes/gitops/prod`;
- Rust container validation for formatting and locked workspace check.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- PostgreSQL Versioning Policy:
  <https://www.postgresql.org/support/versioning/>
- CloudNativePG documentation:
  <https://cloudnative-pg.io/docs/>
