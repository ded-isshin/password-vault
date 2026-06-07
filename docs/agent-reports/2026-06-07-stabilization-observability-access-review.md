# Session Report: Stabilization, Observability, And Browser Access Review

## Goal

Review the current Password Vault MVP preview, Grafana and Argo CD browser access path,
PostgreSQL HA posture, migration policy, SRE observability needs, and agent workflow quality.

## Active Context

- `password-vault`: product docs, static UI work-in-progress, observability and database analysis.
- `infrastructure-home`: read-only cluster/edge checks plus a public-safe browser access runbook.
- `agent-platform`: agent orchestration stability guidance.

Repositories out of scope:

- `hiringtrace-site`
- `hiringtrace-site-archive`

## Work Completed

- Verified the live app, Grafana, and Argo CD through the mini-PC LAN edge ports.
- Confirmed that Kubernetes/LXD `LoadBalancer` addresses are not the right browser target for a
  normal LAN client.
- Verified Argo CD reports `password-vault` as synced and healthy.
- Verified the Password Vault API deployment has three ready replicas and uses a rolling update
  strategy with zero max unavailable.
- Verified the current Grafana dashboard exists and its key VictoriaMetrics queries return data.
- Confirmed the current Password Vault PostgreSQL deployment is a single-instance temporary
  StatefulSet and is not HA.
- Confirmed there is no direct database conflict with the other product database; the risk is
  copying a single-instance pattern into a password manager.
- Added an SRE observability plan with technical, product, business, and security metrics.
- Added a PostgreSQL HA and migration decision brief.
- Added an agent workflow stability document in `agent-platform`.
- Added an infrastructure runbook for browser access to LAN-published services.
- Ran Claude Code as an independent read-only reviewer for the static UI and docs diff.

## Current Product Reality

Verified:

- The current browser page is reachable through the edge route.
- The deployed browser page is still a preview screen, not a complete vault product.
- Backend API smoke for registration and TOTP enrollment has previously worked through HTTPS edge
  testing.
- The deployed API exposes HTTP metrics and is scraped by VictoriaMetrics.

Needs verification:

- Full browser registration plus TOTP enrollment after the frontend and backend KDF contract are
  made compatible.
- Login finish and login-time TOTP verification.
- Browser vault unlock and encrypted item CRUD.
- PostgreSQL failover, backup, and restore.

## Grafana And Argo CD Access

Use the mini-PC LAN edge address and the edge-published ports:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Do not use Kubernetes/LXD `LoadBalancer` addresses from a normal LAN-only browser unless the client
has routing into that network.

## PostgreSQL Findings

Verified:

- Password Vault currently uses a single PostgreSQL `StatefulSet`.
- The other product database is separate by namespace, service, secret, and PVC.
- CloudNativePG CRDs exist in the cluster, but no active password-vault CloudNativePG cluster was
  found during this check.

Decision direction:

- Use a product-specific three-instance CloudNativePG-style PostgreSQL cluster before real user
  secrets.
- Prefer synchronous replication with required durability for acknowledged password-manager writes.
- Block real-user use until backup, restore, and failover drills pass.

## Migration Analysis

Stable PostgreSQL versions do not remove the need for schema migrations. They only reduce database
engine drift.

Migrations are still needed when the product adds or changes accounts, devices, sessions, TOTP,
vault key wraps, encrypted item revisions, indexes, constraints, and compatibility windows.

The right target is not constant churn. The target is:

- stable supported PostgreSQL versions;
- conservative schema changes;
- expand/contract migration policy;
- controlled migration jobs;
- no startup migrations for real users;
- restore-aware rollback planning.

## Observability Direction

Current implemented metrics:

- `up{job="password-vault-api"}`
- `axum_http_requests_total`
- `axum_http_requests_duration_seconds_bucket`
- `axum_http_requests_pending`

Immediate gaps:

- SLO and burn-rate panels.
- Target-down and fast 5xx burn alerts.
- DB pool/query/error metrics.
- PostgreSQL HA, replica lag, backup age, and restore drill metrics.
- Auth/TOTP/security aggregate metrics.
- Product funnel metrics for registration, login, MFA, unlock, and vault sync.
- External synthetic browser/API probe from outside the Kubernetes/LXD network.

## Claude Code Usage

Purpose: independent UI/security/architecture review.

Prompt/task given: review the current product diff, focusing on blocking issues, security concerns,
UX/design concerns, observability gaps, and whether the static UI can work with the current backend
contract.

Summary of output:

- Blocking: the new static UI expects `pbkdf2-sha256-browser-v1`, while the current backend/deploy
  still serves the older Argon2id profile.
- Blocking: the account secret key is shown once without a save/copy/download/confirm gate, creating
  a high data-loss risk.
- Blocking: if registration succeeds but CSRF or TOTP enrollment fails, the user can be stranded
  without a login/resume path.
- Non-blocking: PBKDF2 is browser-native but weaker than Argon2id and must be a documented decision.
- Accepted direction: account secret key, HKDF domain separation, SCRAM verifier provisioning, and
  AES-GCM envelopes are directionally sound.

Accepted suggestions:

- Do not ship the static UI until backend KDF contract and frontend KDF expectations are aligned.
- Add mandatory account secret key save/confirm UX before treating browser registration as MVP.
- Record the PBKDF2 versus Argon2id decision explicitly.

Deferred suggestions:

- Full recovery-code save/download UX.
- Client-side telemetry beyond flow-step state.

Rejected suggestions:

- None.

## Anti-Hallucination Improvements

Accepted:

- Agents must get bounded scopes and disjoint write sets.
- Agents should be allowed to finish within their runtime limits instead of being killed early.
- Claude Code and subagent output must be bucketed as accepted, rejected, or deferred.
- Every claim about tests, dashboards, deployment, or official docs must be backed by command output,
  source links, or marked `Needs verification`.

## Files Changed

Product repository:

- `docs/observability-sre-metrics.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/development.md`
- `docs/agent-reports/2026-06-07-stabilization-observability-access-review.md`
- `crates/api/static/index.html` (`work-in-progress`, not ready to ship)
- `crates/api/static/app.js` (`work-in-progress`, not ready to ship)
- `crates/api/static/app.css` (`work-in-progress`, not ready to ship)

Infrastructure repository:

- `README.md`
- `docs/runbooks/browser-access-to-lan-services.md`

Agent-platform repository:

- `docs/agent-workflow-stability.md`

## Commands Run

```bash
KUBECONFIG=<redacted-path> kubectl get nodes -o wide
KUBECONFIG=<redacted-path> kubectl get svc -A -o wide
KUBECONFIG=<redacted-path> kubectl -n argocd get application password-vault
KUBECONFIG=<redacted-path> kubectl -n password-vault get pods -o wide
curl -k -I https://<mini-pc-lan-ip>:11443/
curl -k -I https://<mini-pc-lan-ip>:3000/api/health
curl -k -I https://<mini-pc-lan-ip>:9443/healthz
node --check crates/api/static/app.js
git diff --check
bash scripts/validate-docs.sh
bash scripts/redact-check.sh
```

## Sources Consulted

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>
- CloudNativePG documentation:
  <https://cloudnative-pg.io/docs/1.29/>
- PostgreSQL Versioning Policy:
  <https://www.postgresql.org/support/versioning/>

## Validation

Tested:

- LAN edge endpoints from the mini-PC returned HTTP 200.
- Argo CD `password-vault` application reported synced and healthy.
- Grafana dashboard datasource queries returned data for target health, request rate, p95 latency,
  5xx ratio, pending requests, and unmatched 404.
- Product docs/static diff passed `git diff --check`.
- Static JavaScript passed `node --check`.
- Agent-platform docs validation and public-safety checks passed.
- Public-safety regex checks over new docs found no private IPs or secrets.

Not tested:

- Browser access from the MacBook itself.
- Full browser registration/TOTP flow after the static UI work-in-progress.
- Login finish and vault CRUD.
- PostgreSQL failover, backup, restore, and migration job behavior.
- Load tests against full user journeys.

## Risks

- The deployed browser page is still not a real usable password vault.
- The static UI work-in-progress is incompatible with the current backend KDF contract.
- The current database is not HA and should not accept real secrets.
- Startup migrations are enabled in the live preview and must be replaced before real users.
- Current observability is route-level and needs product/security/database metrics.

## Next Steps

1. Decide and implement the browser KDF contract: WebCrypto PBKDF2 for MVP or reviewed Argon2id WASM.
2. Add account secret key save/copy/download/confirm gate before registration can continue.
3. Add login finish and login-time TOTP verification.
4. Add vault unlock and encrypted item CRUD/sync.
5. Replace startup migrations with a controlled migration job.
6. Move PostgreSQL to HA operator-managed deployment with backup and restore drills.
7. Add SLO panels, alerts, and external synthetic probes.
8. Re-run Claude Code review after the backend/frontend KDF contract is integrated.
