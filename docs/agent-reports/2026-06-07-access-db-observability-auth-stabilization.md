# Session Report: Access, DB, Observability, And Auth Stabilization

## Goal

Refresh the live access facts for Password Vault, Grafana, and Argo CD; reassess PostgreSQL HA and
migration policy; improve SRE/Golden Signals planning; reduce agent-workflow waste; and close the
current login-finish/TOTP verification evidence gaps.

## Active Context

- `password-vault`: product code, tests, docs, and report artifacts.
- `infrastructure-home`: read-only/live verification plus small documentation consistency fixes in
  the existing observability/password-vault worktree.

Repositories out of scope:

- `hiringtrace-site`
- `hiringtrace-site-archive`
- unrelated product repositories

## Work Completed

- Verified that browser access should use the mini-PC LAN edge address and edge-published ports, not
  Kubernetes/LXD `LoadBalancer` addresses.
- Verified Password Vault, Grafana, and Argo CD health endpoints through the edge path.
- Verified Argo CD reports `password-vault` and `observability-vm-stack` as `Synced` and `Healthy`.
- Verified the Password Vault API has three ready pods.
- Verified edge `/metrics` is blocked, while the namespace still lacks a restrictive
  `NetworkPolicy`.
- Verified Grafana dashboard `Password Vault Overview` exists and key VictoriaMetrics queries return
  data.
- Verified current PostgreSQL remains a preview-only single `StatefulSet` on local storage.
- Verified CloudNativePG CRDs exist but no active CloudNativePG `Cluster` resources exist.
- Confirmed there is no logical database conflict with another product; the gap is product-specific
  HA PostgreSQL resources and operator/controller verification.
- Implemented local `login/finish` and login-time `totp/verify` tests for cross-site rejection,
  replay, and five-attempt exhaustion.
- Hardened TOTP enrollment confirmation so a failed or malformed code consumes the pending factor
  and forces enrollment restart, avoiding a reusable pending TOTP seed without adding a new schema
  migration.
- Tightened unsafe request origin checks to require `https://` origins.
- Fixed API contract drift for auth body limit status and planned audit endpoint status.
- Updated canonical docs to say login finish/TOTP verify are implemented locally, not merged or
  deployed.
- Added three bounded subagent reports and incorporated the accepted findings into canonical docs.
- Ran Claude Code as an independent read-only architecture/security/SRE reviewer.

## Current Browser Access

Use the mini-PC LAN edge address and these ports from a normal LAN browser:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Do not use `<lxd-loadbalancer-ip>` addresses from a MacBook unless the MacBook has explicit routing
or VPN access into the LXD/Kubernetes network.

The certificate is self-signed in the current preview, so browser warnings are expected.

## PostgreSQL And Migrations

Current PostgreSQL is not suitable for real password-vault secrets:

- one PostgreSQL pod;
- local-path PVC;
- no PostgreSQL failover target;
- no product-specific backup/WAL/restore/failover gates.

The target remains a product-specific CloudNativePG-style three-instance PostgreSQL cluster with
product-specific credentials, services, backup target prefix, restore drill, and failover drill.
Sharing a CloudNativePG operator is acceptable; sharing another product's database is not.

Stable PostgreSQL versions do not remove schema migrations. The product still needs migrations for
application-owned tables, indexes, constraints, auth/MFA/session/sync fields, and compatibility
windows. The goal is rare, reviewed, backward-compatible migrations, not no migrations.

Accepted policy:

- keep startup migrations off for real-user environments;
- run schema changes through a controlled migration job or reviewed operator step;
- use expand/contract;
- delay destructive contract changes to later releases;
- verify backup/WAL state before high-risk schema changes.

## Observability

Live dashboard status:

- `sum(up{job="password-vault-api"})` returned `3`.
- Dashboard request-rate, 5xx-ratio, p95-latency, pending-request, and unmatched-404 expressions
  returned data or explicit zero vectors.
- Current dashboard is useful for basic Golden Signals, but it is not a full SLO/product journey
  dashboard.

Minimum next observability work:

- `password_vault_build_info` release/revision metric;
- DB pool/query/error metrics;
- auth hash duration and active-work metrics;
- auth/MFA/security aggregate counters;
- external synthetic journeys through the browser edge path;
- Password Vault-specific VMRule alerts for target down, 5xx burn, latency, pending requests, and
  missing scrape data;
- PostgreSQL HA, backup age, WAL archive, restore drill, and failover drill metrics.

## Agent Workflow

Accepted process changes:

- Use bounded agent work orders with role, mode, allowed write scope, output file, max runtime, and
  acceptance gates.
- Reviewer and Claude Code agents are report-only by default.
- Writer agents need disjoint scopes or separate worktrees.
- Canonical docs are the source of truth; agent reports are dated evidence logs.
- Do not mark work complete without command output, test output, live query results, or an explicit
  `Needs verification` note.

## Claude Code Usage

Purpose: independent architecture/security/SRE review.

Prompt/task given: review browser access, PostgreSQL HA, migrations, observability, waste controls,
and current auth/docs branch gaps. Report only; no edits or commands.

Summary of output:

- Correctly identified the LAN-vs-LXD browser access issue.
- Correctly framed CloudNativePG as a shared operator but product-specific database clusters.
- Correctly reinforced expand/contract migrations and controlled migration jobs.
- Correctly prioritized auth/MFA/security metrics, DB saturation, synthetic probes, and
  release/build info.
- Correctly flagged body-limit docs drift and missing TOTP enrollment failure lockout.
- Incorrectly or partially flagged runtime secret readiness as missing; current code and Helm docs
  already include readiness checks and secret references for the required auth keys.

Accepted suggestions:

- Fix body-limit contract drift.
- Add cross-site and TOTP exhaustion tests.
- Consume pending TOTP enrollment factors on failed confirmation.
- Keep reports separate from canonical docs and triage findings explicitly.

Deferred suggestions:

- Argo CD migration hook/job implementation.
- LAN-trusted certificate strategy.
- Recovery-code verification issue linkage and implementation.

## Subagent Usage

Three bounded subagents completed report-only tasks:

- SRE/Golden Signals report:
  `docs/agent-reports/2026-06-07-sre-golden-signals-observability-review.md`
- PostgreSQL HA/migration stability report:
  `docs/agent-reports/2026-06-07-postgresql-ha-migration-stability-review.md`
- Agent workflow waste reduction report:
  `docs/agent-reports/2026-06-07-agent-workflow-waste-reduction-review.md`

Accepted findings were folded into the MVP plan, API contract, auth/MFA lifecycle doc, observability
plan, and PostgreSQL migration brief.

## Files Changed

Product repository:

- `crates/api/src/auth/routes.rs`
- `crates/api/src/lib.rs`
- `docs/api-contract.md`
- `docs/auth-mfa-lifecycle.md`
- `docs/data-model.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/mvp-implementation-plan.md`
- `docs/observability-sre-metrics.md`
- `docs/security/auth-protocol-v1.md`
- `docs/agent-reports/2026-06-07-access-db-observability-auth-stabilization.md`
- `docs/agent-reports/2026-06-07-agent-workflow-waste-reduction-review.md`
- `docs/agent-reports/2026-06-07-browser-access-sre-stability-refresh.md`
- `docs/agent-reports/2026-06-07-postgresql-ha-migration-stability-review.md`
- `docs/agent-reports/2026-06-07-sre-golden-signals-observability-review.md`

Infrastructure worktree:

- `kubernetes/gitops/prod/apps/password-vault/README.md`
- `kubernetes/gitops/prod/platform/observability/README.md`

## Commands Run

```bash
hostname -I
ip -4 addr show
KUBECONFIG=<redacted-path> kubectl -n argocd get application prod-root password-vault observability-vm-stack
KUBECONFIG=<redacted-path> kubectl -n password-vault get pods,svc,ingress,networkpolicy -o wide
KUBECONFIG=<redacted-path> kubectl get crd
KUBECONFIG=<redacted-path> kubectl get clusters.postgresql.cnpg.io -A -o wide
curl -k https://<mini-pc-lan-ip>:11443/healthz
curl -k https://<mini-pc-lan-ip>:3000/api/health
curl -k https://<mini-pc-lan-ip>:9443/healthz
curl -k https://<mini-pc-lan-ip>:11443/metrics
docker run --rm ... rust:1.96-bookworm cargo fmt --all -- --check
docker run --rm ... rust:1.96-bookworm cargo test --locked --workspace -- --test-threads=1
docker run --rm ... rust:1.96-bookworm cargo clippy --locked --workspace --all-targets -- -D warnings
git diff --check
```

Grafana MCP was used for dashboard search, datasource discovery, and PromQL checks.

## Validation

Tested:

- Edge health endpoints returned HTTP 200 for Password Vault, Grafana, and Argo CD.
- Password Vault edge `/metrics` returned HTTP 404.
- Argo CD reported `password-vault` and `observability-vm-stack` as `Synced` and `Healthy`.
- Grafana/VictoriaMetrics dashboard expressions returned data or explicit zero vectors.
- `cargo fmt --all -- --check` passed.
- `cargo test --locked --workspace -- --test-threads=1` passed: 39 tests plus migration test and
  doctests.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
- `git diff --check` passed.
- Product public-safety scan found only local test DSNs and old disposable PostgreSQL command
  examples, not real secrets.

Not tested:

- Browser access from the MacBook itself.
- Deployed login finish/TOTP verify through the browser; the code is local and not yet merged or
  deployed.
- Full vault unlock, encrypted item CRUD, and sync; these remain unimplemented.
- PostgreSQL failover, backup, restore, and migration job behavior.
- Alert delivery.

## Risks

- The visible product in the browser is still a preview, not a complete password vault.
- Current PostgreSQL is preview-only and not HA.
- No restrictive `NetworkPolicy` exists yet in the `password-vault` namespace.
- Product-specific SLO/burn-rate alerts are not deployed yet.
- Full user journey observability waits on vault CRUD/sync implementation.

## Next Steps

1. Commit and PR the local login-finish/TOTP verification slice after final review.
2. Merge only after CI passes.
3. Publish the new image through GitHub Actions/GHCR and update infrastructure image digest.
4. Add NetworkPolicy/internal-only metrics hardening.
5. Add product-specific alert rules and release/build metrics.
6. Implement browser unlock and encrypted vault item CRUD/sync.
7. Plan product-specific CloudNativePG HA, backup, restore, and failover gates before real secrets.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- PostgreSQL `ALTER TABLE` documentation:
  <https://www.postgresql.org/docs/current/sql-altertable.html>
- CloudNativePG backup documentation:
  <https://github.com/cloudnative-pg/cloudnative-pg/blob/main/docs/src/backup.md>
