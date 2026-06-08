# Session Report: Vault API, Access, PostgreSQL, And Observability

## Goal

Implement and validate the first encrypted vault item API slice, refresh browser access facts for
Password Vault, Grafana, and Argo CD, and keep PostgreSQL/observability status truthful before
deployment.

## Active Context

- `password-vault`: product code, tests, API contract, MVP plan, observability plan, and report.
- `infrastructure-home`: read-only live checks through the Kubernetes control plane and public edge.

Repositories out of scope:

- `hiringtrace-site`
- `hiringtrace-site-archive`
- unrelated product repositories

## Work Completed

- Added local backend routes for:
  - `GET /v1/vaults`
  - `GET /v1/vaults/{vault_id}/sync`
  - `POST /v1/vaults/{vault_id}/items`
  - `POST /v1/vaults/{vault_id}/items/{item_id}/revisions`
- Required `mfa_verified` sessions for vault APIs.
- Required JSON, session, CSRF, Fetch Metadata, and Origin checks for vault write routes.
- Made `item_id` and `revision_id` client-generated so encrypted envelopes can bind them into AEAD
  associated data and change authentication.
- Added bounded sync responses with `has_more`; the implementation caps one response at 500 changes.
- Kept cross-account access as `404 not_found`.
- Fixed `vault_conflict` response shape so conflict paths return `current_head`.
- Updated the API contract, sync protocol, MVP plan, and observability plan.

## Live Access Findings

Verified:

- The mini-PC LAN-facing edge routes respond for Password Vault, Grafana, and Argo CD health.
- Grafana has the provisioned `Password Vault Overview` dashboard.
- Grafana datasource queries for target health, request rate, 5xx ratio, p95 latency, pending
  requests, and unmatched 404 rate return live data.
- Argo CD reports `password-vault` and the observability applications as `Synced` and `Healthy`.

Important access note:

- A normal MacBook browser should use the mini-PC LAN-facing address and edge ports.
- Kubernetes `LoadBalancer`, pod, and service addresses from the LXD/Kubernetes network are not
  expected to work from the MacBook unless explicit routing or VPN access exists.
- Current TLS is self-signed, so browser certificate warnings are expected.

## PostgreSQL Findings

Verified:

- The deployed preview still uses one `postgres:17-bookworm` StatefulSet replica on node-local
  `local-path` storage.
- No product CloudNativePG `Cluster`, `Backup`, or `ScheduledBackup` resources exist.
- CloudNativePG CRDs exist, but no CloudNativePG operator/controller was observed in the live scan.
- No `NetworkPolicy` exists in the `password-vault` namespace.

Conclusion:

- The current PostgreSQL deployment is preview-only and must not hold real password-vault secrets.
- A separate product PostgreSQL cluster can coexist with other products. There is no need to share
  another product database; the safer model is a shared operator with separate product `Cluster`,
  credentials, backups, restore drills, and runbooks.

## Migration Analysis

Stable PostgreSQL versions reduce engine drift; they do not remove application schema migrations.
The target is not frequent migrations. The target is rare, reviewed, backward-compatible migrations
with explicit release gates.

For real users:

- keep startup migrations disabled;
- use the controlled GitOps migration job/runbook;
- prefer expand/contract schema changes;
- verify migrations against tests and representative data;
- keep backup and restore evidence before risky changes.

## Observability Analysis

Google SRE's Four Golden Signals are the base layer:

- latency
- traffic
- errors
- saturation

For this product, dashboards also need password-manager-specific signals:

- protected activation: registration plus MFA plus first encrypted item saved;
- returning access: login plus MFA plus vault unlock;
- vault write and sync success;
- conflict and stale revision rates;
- abuse resistance: rate limits, CSRF failures, MFA failures, recovery-code attempts;
- durability confidence: PostgreSQL HA, backup age, restore drill age, failover drill result.

Current maturity:

- Live preview is between L1 Golden Signals and L2 actionable alerts.
- Product/business/security counters are still planned, not implemented.
- External synthetic journeys are still planned.

## Claude Code Usage

Purpose: independent architecture/security review.

Prompt/task given: review the uncommitted vault API diff, browser/Grafana/Argo access findings,
PostgreSQL HA/migration posture, and observability plan; report only; no edits.

Summary of output:

- Accepted: `vault_conflict` must always include `current_head`.
- Accepted: sync responses must be bounded and paginated/capped before real user data.
- Accepted: remove dead `name_ciphertext` runtime field while encrypted vault metadata is planned.
- Accepted: add account ownership filter to `update_vault_head` as defense in depth.
- Deferred: product-specific vault metrics remain planned for the next observability slice.
- Deferred: cleaner session helper extraction can wait until there is a third consumer.

## Validation

Tested:

- `cargo fmt --all -- --check` in `rust:1.96.0-bookworm`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` in `rust:1.96.0-bookworm`.
- `cargo test --locked --workspace -- --test-threads=1` in `rust:1.96.0-bookworm` against a
  disposable PostgreSQL `18-alpine` container.
- Local docs required-file check equivalent to `.github/workflows/docs.yml`.
- Local public-safety grep equivalent to `.github/workflows/security.yml`.
- Live edge HTTP checks for Password Vault, Grafana health, Grafana dashboard, and Argo CD health.
- Live Grafana datasource queries for dashboard panels.
- Read-only Kubernetes checks through the control-plane container.

Not tested:

- Browser UI unlock/write/sync flow. The browser unlock UI is not implemented yet.
- Deployment of the local vault API branch. The branch is implemented locally, not deployed.
- Product-specific auth/vault/business metrics. They remain planned.
- CloudNativePG failover, backup, WAL archive, or restore drills. The product cluster does not
  exist yet.

## Risks

- The deployed preview database is still single-instance and preview-only.
- The current product does not yet have NetworkPolicy.
- The vault API is not deployed yet, so live product validation is still limited to existing
  deployed auth/preview paths.
- Observability does not yet include vault write/sync counters or synthetic journeys.

## Next Steps

1. Open/merge the vault API PR after CI and review.
2. Build and deploy the new API image through GitHub/GitOps.
3. Implement browser vault unlock and encrypted item UI using the API-first contract.
4. Add product-specific application metrics and update the Grafana dashboard/alerts.
5. Replace preview PostgreSQL with a product-specific CloudNativePG cluster plus backup and restore
   gates before real secrets.

## Sources

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Workbook, Monitoring:
  <https://sre.google/workbook/monitoring/>
- Google SRE Workbook, Alerting on SLOs:
  <https://sre.google/workbook/alerting-on-slos/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
