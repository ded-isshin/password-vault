# Session Report: Browser Access, PostgreSQL HA, SRE Metrics, And Migration Hook Review

Status: public-safe report.
Date: 2026-06-08.

## Goal

Answer the stabilization questions for the Password Vault MVP:

- verify how Grafana, Argo CD, and Password Vault should be reached from a browser;
- clarify PostgreSQL HA needs and whether another product creates a conflict;
- apply Google SRE Golden Signals and product-specific metrics to the dashboard plan;
- explain why database migrations remain necessary on stable PostgreSQL;
- reduce waste from duplicated reports and partial agent work;
- fix any small high-confidence issues found during the review.

## Active Context

- `password-vault`: product code, Helm chart, CI, API/sync docs, SRE docs.
- `infrastructure-home`: read-only live Kubernetes/Grafana/Argo checks plus one public-safe README
  wording update.

Out of scope:

- unrelated product repositories;
- direct runtime secret changes;
- direct `kubectl apply/delete/patch/replace`;
- PostgreSQL HA deployment in this slice.

## Work Completed

- Verified the LAN edge routes from the mini-PC side:
  - Grafana health returned HTTP 200 through the HTTPS edge route;
  - Argo CD health returned HTTP 200 through the HTTPS edge route;
  - Password Vault browser route returned HTTP 200 through the HTTPS edge route.
- Confirmed Grafana MCP access:
  - datasource `VictoriaMetrics` exists;
  - dashboard `Password Vault Overview` exists;
  - `sum(up{job="password-vault-api"})` returned `3`;
  - dashboard panel queries return data or intentional zero fallback.
- Confirmed Argo CD applications are generally `Synced` / `Healthy`.
- Found an important Argo CD debt: the latest `password-vault` operation state is `Failed` because
  the fixed-name `password-vault-migrate` hook Job could not be dry-run applied after the image
  digest changed. Kubernetes Jobs have immutable pod templates.
- Updated the product Helm chart to render Argo migration hooks with `metadata.generateName` and
  `HookSucceeded`, avoiding fixed-name Job immutability on subsequent digest rollouts.
- Updated product and infrastructure docs to reflect generated-name migration hook behavior.
- Updated browser sync logic so the client no longer adopts server `to_head` as an unverified local
  checkpoint. The final sync page must match the locally verified keyed head-hash chain.
- Removed a misleading no-op `vaultKeyAad()` helper from the browser code.
- Updated the API contract and sync protocol to state that final `to_head` is a verification target,
  not a trusted checkpoint.
- Updated SRE docs with a daily operating view and product-specific metrics that focus on protected
  activation, returning access, encrypted write/read/sync, durability, and abuse resistance.

## PostgreSQL HA Finding

There is no logical conflict with another product. The real issue is maturity:

- the current Password Vault database is a single PostgreSQL `StatefulSet`;
- it uses node-local storage;
- CloudNativePG CRDs exist in the cluster;
- no active product `Cluster`, `Backup`, or `ScheduledBackup` resources exist;
- no CloudNativePG operator/controller was observed in the live cluster scan.

The recommended path remains:

- product-owned CloudNativePG cluster;
- PostgreSQL 17 for the migration path, avoiding a simultaneous major upgrade;
- three instances across worker nodes;
- quorum synchronous replication with one standby and required durability;
- S3-compatible object storage with WAL archiving and scheduled physical backups;
- restore and failover drills before real user secrets.

## Migration Analysis

Stable PostgreSQL does not remove application schema migrations. PostgreSQL stability means the
engine behavior is supported and predictable; it does not create or evolve Password Vault tables,
indexes, constraints, auth/MFA fields, encrypted vault metadata, or sync revision invariants.

The correct goal is:

- few migrations;
- reviewed migrations;
- immutable migration files after merge;
- expand/contract compatibility for real-user data;
- controlled Argo/operator migration job;
- startup migrations disabled for real-user environments.

## Observability State

Current state:

- dashboard is useful L1 Golden Signals coverage;
- product counters exist for registration, login, MFA, session, vault item, sync, and build info;
- basic live panel queries return data;
- no tested product alert rules yet;
- no external browser-equivalent synthetic journey yet;
- no PostgreSQL HA/backup/restore panels yet;
- no DB pool/query/auth-hash saturation metrics yet.

The MVP north-star synthetic journey is:

```text
register -> confirm TOTP -> lock/return -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt item
```

## Claude Code Usage

Purpose: independent architecture/security/GitOps/SRE review.

Prompt/task given: report-only review of the current uncommitted product diff and infra findings,
focused on Argo migration hooks, browser crypto/sync, PostgreSQL HA, SRE metrics, and workflow
waste.

Summary of output:

- Critical deferred risk: real secrets remain blocked by single PostgreSQL StatefulSet, missing CNPG
  operator, missing backups, and missing drills.
- Medium accepted finding: browser sync must not trust `to_head` without a locally verified chain.
- Medium deferred finding: browser crypto/sync needs behavioral WebCrypto or headless-browser tests,
  not only `node --check`.
- Medium deferred finding: observability is L1 until alerts and synthetics are deployed.
- Low accepted finding: generated-name migration hook fix is correct.
- Low accepted finding: `genesis_head_hash` design is sound.
- Low accepted finding: remove or implement the no-op `vaultKeyAad()`.

Accepted suggestions:

- Fix final-page `to_head` verification in browser sync.
- Remove the misleading no-op AAD helper.
- Keep CNPG HA, backup, restore, alerts, and synthetics as explicit gates before real secrets.

Deferred suggestions:

- Add WebCrypto/headless-browser sync behavior tests.
- Add full synthetic journey metrics.
- Add VMRule alerts and DB/auth-hash saturation metrics.
- Deploy CloudNativePG HA and backup resources through GitOps.

Rejected suggestions:

- None.

## Validation

Tested:

- `cargo fmt --all -- --check`
- `cargo check --locked --workspace`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`
- `node --check crates/api/static/app.js`
- Helm lint and render using `alpine/helm:3.19.0`
- Helm migration hook assertions for `generateName`, `HookSucceeded`, `PreSync`, no fixed Job name,
  no TTL, no service account token, and required database secret reference
- negative Helm render check for forbidden non-Argo migration Job
- `git diff --check` in product and infra worktrees
- public-safety heuristic scan of changed files
- Grafana MCP datasource and dashboard queries
- read-only Kubernetes checks for apps, services, pods, CRDs, jobs, storage class, and Argo app
  status

Not tested:

- direct MacBook browser connectivity;
- full browser UI journey after the local browser-vault branch is deployed;
- second digest rollout after generated-name migration hook fix;
- CloudNativePG failover or restore drills.

## Files Changed

Product repository:

- `.github/workflows/docs.yml`
- `.github/workflows/helm.yml`
- `crates/api/src/lib.rs`
- `crates/api/src/vault.rs`
- `crates/api/static/app.css`
- `crates/api/static/app.js`
- `crates/api/static/index.html`
- `deploy/helm/password-vault/README.md`
- `deploy/helm/password-vault/templates/migration-job.yaml`
- `deploy/helm/password-vault/values.yaml`
- `docs/api-contract.md`
- `docs/lock-unlock-state.md`
- `docs/mvp-implementation-plan.md`
- `docs/observability-sre-metrics.md`
- `docs/runbooks/release-and-rollout.md`
- `docs/sync-protocol.md`

Infrastructure repository:

- `kubernetes/gitops/prod/apps/password-vault/README.md`

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google Cloud Observability, service monitoring concepts:
  <https://docs.cloud.google.com/stackdriver/docs/solutions/slo-monitoring>
- CloudNativePG replication:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG recovery:
  <https://cloudnative-pg.io/docs/1.29/recovery>
- Argo CD resource hooks:
  <https://argo-cd.readthedocs.io/en/release-2.14/user-guide/resource_hooks/>
- Argo CD sync options:
  <https://argo-cd.readthedocs.io/en/latest/user-guide/sync-options/>

## Next Tasks

1. Publish and deploy the browser-vault workflow plus generated-name migration hook fix.
2. Verify a second digest rollout clears the Argo migration hook immutability problem.
3. Add full synthetic journey coverage and one low-cardinality journey success/failure metric.
4. Add VMRule alerts for target down, fast 5xx burn, p99 latency, and unavailable replicas.
5. Add DB pool/query/error and auth-hash saturation metrics.
6. Deploy product-owned CloudNativePG HA with backups, restore drill, and failover drill.
7. Consolidate duplicated current-state claims into canonical docs; keep agent reports as evidence.
