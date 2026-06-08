# MVP Implementation Plan

Status: draft. Created for milestone `v0.2-working-mvp`.

Current implementation status, 2026-06-08:

- Deployed browser preview, health/readiness/metrics, active CloudNativePG preview PostgreSQL,
  Helm/GitOps, and Grafana dashboard exist.
- The current GitOps preview is `Synced/Healthy/Succeeded` in Argo CD. API pods are three ready
  replicas. The chart now supports topology spread `nodeAffinityPolicy` and `nodeTaintsPolicy`
  controls plus `matchLabelKeys: [pod-template-hash]` so production can combine live
  `maxUnavailable: 0` / `maxSurge: 1` updates with hard worker-node spreading once the
  infrastructure values are updated and verified.
- The generated-name Argo CD migration hook fix is merged, published, and rolled out. The previous
  fixed-name `password-vault-migrate` Job from an older digest remains visible as historical
  pruning debt, but it no longer blocks the current application sync.
- `/v1/auth/register/start`, `/v1/auth/login/start`, `/v1/auth/register/finish`,
  `/v1/session`, `/v1/csrf`, `/v1/auth/logout`, `/v1/mfa/totp/enroll/start`, and
  `/v1/mfa/totp/enroll/confirm` are implemented.
- `register/finish` creates the account, encrypted account keyset metadata, initial vault, encrypted
  vault key wrap, device record, and setup session.
- `GET /v1/csrf` rotates the per-session CSRF token hash, and `POST /v1/auth/logout` validates
  session plus CSRF before deleting the current session.
- TOTP enrollment starts a pending factor, encrypts the server-owned seed with the runtime
  `PV_TOTP_SEED_KEY_B64` key, and confirmation upgrades the setup session to `mfa_verified` while
  returning one-time recovery codes.
- Login finish and login-time TOTP verification are merged and deployed in the current preview.
- Vault list, encrypted item create/update/delete revision writes, and delta sync are merged and
  deployed in the current GitOps preview with database-backed tests.
- The browser preview supports registration, TOTP enrollment, return login, in-memory vault unlock,
  encrypted item create/update/delete, and sync on top of the deployed vault API.
- The browser vault workflow is merged, published, rolled out, and visible through the mini-PC edge
  route.
- A dependency-free Node browser API synthetic journey exists in
  `load/synthetic/browser-api-journey.mjs`. It exercises registration, TOTP enrollment, logout,
  return login, login-time TOTP, vault unlock, encrypted item create, sync, MAC/head validation, and
  item decryption. It also verifies recovery-code login into an `mfa_recovery` session, confirms
  that recovery sessions cannot access vault APIs, and re-enrolls TOTP. It is wired into PR
  container smoke and the manual `load-smoke` workflow. A live edge run after the CNPG cutover
  succeeded on 2026-06-08; future deployed changes still need a fresh live edge run before their
  browser path can be treated as proven end-to-end.
- The browser synthetic now has a local `SYNTHETIC_SELF_TEST_ONLY=true` crypto guard that checks
  AES-GCM rejection for tampered ciphertext, nonce, and authenticated metadata before any API
  account is created.
- The current branch adds a dry-run-first `cleanup-synthetic` maintenance command for old
  reserved-domain synthetic accounts. This enables bounded cleanup of live-test data, but a
  scheduled external synthetic probe and cleanup job are still future work.
- Recovery-code verification is implemented for the MVP preview. It can only be used after primary
  login proof succeeds, consumes one unused recovery code, creates an `mfa_recovery` session without
  vault access, and requires TOTP re-enrollment before vault APIs are available again.
- The live preview is reachable through the mini-PC HTTPS edge route with a self-signed certificate.
  The in-cluster app service remains plain HTTP behind the edge proxy.
- Grafana and Argo CD are also reachable through the mini-PC HTTPS edge route from the mini-PC.
  MacBook/browser reachability still needs a client-side check; do not use Kubernetes/LXD
  `LoadBalancer` addresses as MacBook URLs.
- Grafana `Password Vault Overview` is deployed and live queries return API scrape health, request
  rate, p95 latency, 5xx ratio, pending requests, and unmatched 404 rate data.
- Product-specific observability counters for registration, login, MFA, sessions, vault item
  changes, sync requests, and build information are merged, published, deployed, and covered by a
  low-cardinality metrics test. Live checks verified `password_vault_build_info`,
  `password_vault_registration_events_total`, `password_vault_mfa_events_total`,
  `password_vault_vault_item_changes_total`, and `password_vault_sync_requests_total` in
  VictoriaMetrics after synthetic runs.
- The shared CloudNativePG operator is deployed through infrastructure GitOps. A product-owned
  `password-vault-cnpg` CloudNativePG `Cluster` is deployed and verified live with three PostgreSQL
  18.4 instances spread across the three worker nodes. The API is cut over to the CNPG application
  Secret. Real password data remains blocked until backup availability, restore drills, failover
  drills, alert delivery, and scheduled synthetic monitoring gates are complete.
- A controlled migration runner is merged, published, and deployed: the API image supports a
  `password-vault-api migrate` command, startup migrations remain disabled in production values, and
  generated-name Argo CD `PreSync` migration hooks have completed successfully during rollout.
- GitHub `main` is protected by an active ruleset requiring PRs, squash merges, resolved
  conversations, linear history, non-fast-forward protection, branch deletion protection, and the
  always-running `docs` and `public-safety` checks. Repository security features for vulnerability
  alerts, Dependabot security updates, secret scanning, and push protection are enabled.

## Stabilization-First Queue

The current goal is not to add broad feature volume. The next slices should make the smallest useful
MVP dependable:

1. Prove browser access from the client side, not only from the mini-PC: Password Vault, Grafana,
   and Argo CD should be checked from the MacBook/browser path with the expected self-signed TLS
   warning.
2. Run the full synthetic browser/API journey in CI and against the live edge route:
   `register -> confirm TOTP -> logout -> login -> verify TOTP -> unlock -> create item -> sync -> read/decrypt`.
3. Keep browser crypto tests non-negotiable: keep the local tamper self-test and add future test
   vectors only when they directly protect the accepted crypto format.
4. Keep live synthetic data bounded: use reserved `.invalid` handles, dry-run cleanup first, and do
   not schedule production cleanup until HA/backup posture is understood.
5. Complete the database durability track: keep the active CloudNativePG cluster healthy, add
   backup/WAL/restore/failover gates, and remove the legacy preview PostgreSQL rollback artifact only
   after recorded restore evidence exists. Do not accept real secrets before this is complete.
6. Add backup, WAL archiving, restore drill, and failover drill gates before real-user use.
7. Restrict internal API and `/metrics` access with NetworkPolicy or a separate internal metrics
   listener before real-user use.
8. Deploy and test SLO/alert rules for target-down and fast 5xx burn-rate before adding broader
   alert volume.
9. Verify the auth/MFA/session/vault/sync product metrics through the full synthetic journey, then
   expand observability
   further to database health, backup freshness, and security aggregate metrics.
10. Add external synthetic checks from a client path equivalent to a MacBook/browser path, not only
   from inside the Kubernetes/LXD network.
11. Consolidate current-state documentation before creating new agent reports or GitHub issues, so
   stale bootstrap claims do not become false work items.

Anything outside this queue should be deferred unless it directly reduces risk for these gates.

## Execution Hygiene

The project already has enough evidence logs for the current MVP stage. New work should update the
canonical document that owns the topic before creating another agent report:

- access and rollout behavior: `docs/runbooks/release-and-rollout.md`;
- current MVP state and queue: this document;
- PostgreSQL HA, backups, and migrations:
  `docs/decision-briefs/2026-06-08-postgresql-ha-migrations-stability.md`;
- observability/SRE metrics: `docs/observability-sre-metrics.md`;
- historical command evidence: `docs/agent-reports/`.

Do not create a new issue, report, metric, dashboard panel, migration, or architectural spike unless
it has one of these outcomes:

- removes a blocker before real password data;
- proves an existing claim with runtime evidence;
- adds or validates core MVP behavior;
- reduces an operational/security risk;
- replaces stale or duplicated documentation with a clearer canonical source.

Defer or delete work that only restates existing analysis, adds speculative future features, creates
unverified dashboard panels, or introduces schema changes for ideas that are not part of the
stabilization queue.

Temporary worktrees and side copies are allowed only as short-lived implementation tools. They must
be merged, reconciled, or deleted after their PR/task completes. Do not keep parallel documentation
copies as alternate sources of truth; they cause stale conclusions and agent collisions.

## Active Context

Task: deliver a working browser-first MVP and prepare deployment through GitHub, GHCR, Helm, and
Argo CD.

Active repositories:

- `password-vault`: product code, product docs, CI, Helm chart, product issues and PRs.
- `infrastructure-home`: read-only analysis now; future GitOps PR only after a dedicated approval
  point.

Repositories out of scope:

- `hiringtrace-site`
- `hiringtrace-site-archive`
- unrelated products

Risk level: high. This product handles authentication, client-side encryption, TOTP MFA, database
state, public CI, and public deployment.

## Goal

Build a deployed MVP where a personal user can:

1. register an account;
2. enroll TOTP MFA;
3. log in from a browser;
4. unlock the vault locally;
5. create, read, update, delete, and sync encrypted vault items;
6. use a Kubernetes-deployed instance with health checks, CI, rollback docs, and public-safety
   review.

## Non-Goals

- Organization or team vaults.
- Sharing between users.
- Chrome extension implementation.
- iOS/mobile/desktop clients.
- KeePass/KDBX import.
- Billing.
- Admin recovery that can decrypt a user's vault.
- Accepting real user secrets before backup, restore, and deployment gates are proven.

## Current GitHub Control Plane

- Project: <https://github.com/users/ded-isshin/projects/2>
- Research milestone: `v0.1-research`
- Delivery milestone: `v0.2-working-mvp`
- Delivery epic: #11

Existing blockers:

- #2 Auth/login and key-derivation protocol ADR.
- #3 Browser KDF and crypto v1 format ADR.
- #4 TOTP seed custody and MFA hardening.
- #5 PostgreSQL HA, backup, and restore ADR.
- #9 Multi-device client and browser extension roadmap.

Recently resolved control-plane gate:

- #7 Branch ruleset and public repository safety gates. GitHub `main` protection and repository
  security features are enabled. CODEOWNERS review remains a later tightening step once reviewer
  ownership is stable.

Delivery issues:

- #12 MVP implementation plan and dependency graph.
- #13 `/v1` API contract for MVP auth, devices, and vault sync.
- #14 Rust API service scaffold.
- #15 PostgreSQL schema and migrations.
- #16 Auth sessions and TOTP MFA server flows.
- #17 Browser crypto package and encrypted payload test vectors.
- #18 Encrypted vault item API and sync conflict checks.
- #19 Browser web app.
- #20 Docker build, CI tests, and GHCR release workflow.
- #21 Helm chart.
- #22 GitOps application in `infrastructure-home`.
- #23 Deploy, backup, restore, and rollback runbook.
- #24 OPAQUE browser/Rust library compatibility spike.
- #25 Vault revision freshness and rollback-resistance.
- #26 Rust build environment for MVP implementation.

## Dependency Graph

```text
Threat model and foundation docs
  -> #24 OPAQUE/browser compatibility spike
  -> #2 auth/login ADR
  -> #3 browser KDF/crypto ADR
  -> #4 TOTP custody/MFA research
  -> #9 multi-device roadmap
  -> #25 vault revision freshness and rollback-resistance
  -> #13 /v1 API contract
  -> #15 database schema
  -> #26 Rust build environment
  -> #14 backend scaffold
  -> #16 auth/session/TOTP implementation
  -> #17 browser crypto package
  -> #18 encrypted vault CRUD/sync API
  -> #19 browser web MVP
  -> #20 CI/image/GHCR
  -> #21 Helm chart
  -> #5 PostgreSQL HA/backup ADR
  -> #22 infrastructure-home GitOps PR
  -> #23 deploy/rollback/restore runbook
  -> explicit human approval for Argo CD sync or any cluster mutation
  -> deployed smoke test
```

Parallel tracks:

- #7 GitHub branch ruleset and public-safety gates can proceed while auth/crypto research runs.
- #5 PostgreSQL HA/backup can proceed while product scaffolding starts.
- #26 Rust build environment can proceed while #24/#2/#3 are still open.
- Backend health/readiness scaffold can start before final crypto implementation, but it must not
  implement security-sensitive auth until #2, #3, #4, and #24 are resolved.
- Frontend layout can start after the API contract draft exists, but crypto and login behavior must
  wait for accepted security decisions.

## Recommended MVP Stack

- Backend: Rust, Axum, Tokio, SQLx.
- Frontend: TypeScript, React, Vite.
- Browser crypto: WebCrypto for AES-GCM/HKDF; Argon2id via reviewed WASM only after #3/#24.
- Authentication direction: derived-auth-key remains the documented MVP default unless #24 proves
  OPAQUE is practical for the selected Rust/browser stack. OPAQUE is preferred security direction,
  but it must not become an indefinite blocker or an untested protocol dependency.
- MFA: TOTP as login MFA, not as a vault encryption factor.
- Database: PostgreSQL.
- Kubernetes database direction: CloudNativePG, with backup/restore and failover gates before real
  user secrets.
- CI/CD: GitHub Actions on GitHub-hosted runners, GHCR images, GitOps deployment through
  `infrastructure-home` and Argo CD.

## Critical Decisions Before Security-Sensitive Code

### Auth/Login

Issue #2 must decide:

- whether #24 justifies OPAQUE in the MVP or confirms the temporary derived-auth-key default;
- exact registration/login message sequence;
- whether the backend stores an OPAQUE credential record or a weaker temporary verifier;
- account enumeration behavior;
- session creation point and MFA challenge sequencing;
- required tests.

### Browser Crypto

Issue #3 must decide:

- KDF and parameters;
- browser/WASM dependency choice and supply-chain review;
- key hierarchy and domain separation;
- AES-GCM envelope format;
- nonce strategy;
- associated data;
- test vectors.

Issue #3 and #13 must also define rollback/freshness protection for encrypted item sync. Binding an
item payload to `revision_id` in AAD proves the ciphertext belongs to a revision; it does not prove
that the server returned the latest revision. The MVP needs an explicit design for stale revision
replay, such as a per-vault monotonic counter, client-verifiable hash chain, or another accepted
freshness signal.

### TOTP MFA

Issue #4 must decide:

- TOTP seed custody;
- encryption-at-rest for server-owned TOTP seeds;
- time-step and drift window;
- replay denial;
- recovery code storage and rotation;
- rate limits and audit events.

### Database/Backup

Issue #5 must decide:

- CloudNativePG mode;
- synchronous versus asynchronous replication;
- backup target and credentials contract;
- restore drill;
- real-user-data gate.

## Implementation Sequence

### Phase 1: Design Closure

Target issues: #12, #13, #24, #25, #26, #2, #3, #4, #5, #7, #9.

Outputs:

- MVP implementation plan.
- Official-docs research note.
- OPAQUE/browser spike.
- Auth/crypto/TOTP/database ADRs.
- Updated API contract.
- GitHub public-safety settings proposal.

### Phase 2: Product Scaffold

Target issues: #14, #15, #20.

Outputs:

- Rust workspace and backend service.
- Health/readiness endpoints.
- Database migration skeleton and local test harness.
- Initial CI for Rust, TypeScript, docs, YAML, and secret-pattern checks.

Entry gate: choose a build/test environment for Rust before #14. Local `rustc` and `cargo` are not
currently installed. The selected MVP default is container/CI-based Rust builds, documented in
[Development Environment](development.md).

Security constraint: product scaffold must not create a fake auth protocol just to move faster.

### Phase 3: Auth, Crypto, Vault Sync

Target issues: #16, #17, #18.

Outputs:

- Registration/login/session implementation.
- TOTP enrollment/verification/recovery codes.
- Browser crypto package.
- Encrypted item CRUD/sync API.
- Cross-account isolation tests.
- No-plaintext persistence tests.

### Phase 4: Browser MVP

Target issue: #19.

Outputs:

- Register/login/TOTP/unlock flow.
- Vault item list/create/edit/delete.
- In-browser decrypt/search after unlock.
- Browser e2e happy path.

Frontend/design review: run Claude Code before marking the UI PR review-ready.

### Phase 5: Packaging And GitOps Handoff

Target issues: #20, #21, #22, #23.

Outputs:

- Container image build and GHCR release workflow.
- Product-owned Helm chart.
- `infrastructure-home` GitOps PR modeled after the existing application handoff pattern.
- Deploy/rollback/restore/smoke-test runbook.

Cluster mutation gate: do not run `kubectl apply`, `helm upgrade`, Argo CD sync, or equivalent
deployment commands without explicit human approval.

## GitOps Deployment Shape

The current infrastructure pattern uses an Argo CD `Application` with:

- Helm chart source from the product repository;
- production values from `infrastructure-home` via Argo CD multi-source values;
- a namespace dedicated to the app;
- `CreateNamespace=true`;
- image tag pinned in values.

For `password-vault`, the expected future infra files are:

- `kubernetes/gitops/prod/apps/password-vault/application.yaml`
- `kubernetes/gitops/prod/apps/password-vault/kustomization.yaml`
- `kubernetes/gitops/prod/apps/password-vault/values-prod.yaml`
- `kubernetes/gitops/prod/apps/password-vault/README.md`
- update to `kubernetes/gitops/prod/apps/kustomization.yaml`

Potential platform/data files may be needed if CloudNativePG is not already installed.

## Public Safety Gates

Before each public-facing PR:

- no secrets, tokens, private keys, kubeconfigs, real `.env` values, private hostnames, private IPs,
  private domains, or sensitive logs;
- redaction placeholders for infrastructure details;
- no self-hosted runner dependency for public CI;
- minimal GitHub Actions permissions;
- no unsafe `pull_request_target` workflow;
- third-party actions reviewed and pinned/trusted where practical.

Before deployment:

- runtime secrets provisioned outside Git;
- GHCR visibility and pull secret contract decided;
- ingress host/path/port recorded without private details in public docs;
- backup target and restore process validated before real user data.

## Agent Coordination

Codex remains the orchestrator and final integrator.

Default model:

- reviewer/advisor agents are report-only;
- writer agents may edit only explicitly assigned, disjoint scopes;
- shared ADRs and architecture docs have a single writer at a time;
- Claude Code is used for architecture/security/frontend/GitOps review and must be allowed to
  finish unless blocked or unsafe.

## Validation Plan

MVP validation must include:

- Rust unit/integration tests.
- SQL migration validation.
- API contract tests.
- TOTP RFC-vector or deterministic validation tests.
- Browser crypto test vectors.
- Cross-account access denial tests.
- Stale revision conflict tests.
- No-plaintext persistence test.
- Frontend e2e happy path: register, enroll TOTP, login, unlock, create item, reload, decrypt.
- Helm template validation.
- GitOps render validation.
- Deployed smoke test after explicit approval.

## Current Risks

- OPAQUE may be correct architecturally but impractical if browser/server library maturity is not
  sufficient. Current default remains derived-auth-key until #24 proves OPAQUE practical.
- Local Rust tooling is not currently available on the mini-PC: `rustc` and `cargo` were not found
  during the initial implementation-readiness check. The MVP default is container/CI-based builds;
  a host Rust installation remains a separate approval point.
- Browser-delivered JavaScript remains a residual risk for a web password manager.
- Argon2id in browser likely introduces a WASM dependency that needs supply-chain review.
- TOTP seeds are server-owned secrets and must be protected separately from user vault data.
- Database rollback/replay can expose stale encrypted state unless revision/conflict handling is
  strong.
- The infrastructure repository currently has unrelated dirty work; future GitOps changes require
  a clean branch/worktree plan.

## Approval Points

Routine GitHub issues, branches, PRs, and safe docs/code changes can proceed under the current
orchestrator workflow.

Explicit human approval is still required before:

- GitHub repository settings/secrets/deploy keys are changed;
- any runtime secret is created or exposed;
- any `kubectl`, `helm`, `terraform`, `lxc`, or Argo CD command mutates infrastructure;
- deployment-impacting infrastructure PR is merged or synced;
- real user secrets are accepted into the MVP instance.
