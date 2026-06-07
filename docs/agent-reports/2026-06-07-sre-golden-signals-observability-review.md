# SRE Golden Signals Observability Review

Status: draft review. Date: 2026-06-07.

Scope: `password-vault` product documentation and public-safe SRE planning only.

This report does not refresh the live Kubernetes, Grafana, Argo CD, or VictoriaMetrics state. It is
based on repository documents and the official Google SRE material listed in the sources. Any prior
live-cluster statements referenced from older reports should be treated as point-in-time evidence,
not as current verification.

## Active Context

- Active repository: `password-vault` only.
- Repositories explicitly out of scope: `infrastructure-home`, `agent-platform`, unrelated product
  repositories.
- Risk level: medium. This is a public documentation report for an observability and stability plan.
- Roles: SRE/Observability analyst, Documentation/Knowledge.
- Write scope: this file only.

## Materials Reviewed

Local repository documents:

- `docs/observability-sre-metrics.md`
- `docs/agent-reports/2026-06-07-current-stabilization-sre-review.md`
- `docs/agent-reports/2026-06-07-stabilization-observability-access-review.md`
- `docs/decision-briefs/2026-06-07-auth-crypto-mvp.md`
- `docs/decision-briefs/2026-06-07-client-roadmap.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-backup.md`
- `docs/research/container-ci-observability-load-2026-06-07.md`
- `docs/research/official-docs-mvp-stack-2026-06-07.md`
- `docs/research/source-baseline-2026-06-06.md`
- `docs/research/vault-openbao-platform-secrets.md`
- `docs/research/auth-login-protocol-options.md`
- `docs/research/auth-crypto-v1-analysis.md`

Official SRE sources:

- Google SRE Book, Monitoring Distributed Systems:
  <https://sre.google/sre-book/monitoring-distributed-systems/>
- Google SRE Book, Service Level Objectives:
  <https://sre.google/sre-book/service-level-objectives/>
- Google SRE Workbook, Implementing SLOs:
  <https://sre.google/workbook/implementing-slos/>

## Google SRE Principles Applied

Google's Four Golden Signals are latency, traffic, errors, and saturation. For `password-vault`,
they should be interpreted through user-visible password-manager journeys instead of only raw HTTP
traffic.

Key implications:

- Monitoring must answer both "what is broken?" and "why might it be broken?", but paging should
  prefer user-visible symptoms over noisy causes.
- Black-box checks are required because only end-to-end probes can prove that a user can register,
  log in, complete MFA, unlock locally, write an encrypted item, read it back, and sync.
- White-box metrics are still required for diagnosis: API route metrics, database pool pressure,
  PostgreSQL replication/backup state, auth hashing pressure, and rollout state.
- Dashboards should stay simple enough to support action. Metrics that are not used by dashboards,
  alerts, load tests, or release gates should be removed or deferred.
- Candidate SLOs are useful now, but they should not be treated as contractual production SLOs until
  real traffic, synthetic journeys, alert delivery, and database durability evidence exist.

## Product Reliability Model

The reliability question for a password manager is not simply "is the API up?" The core question is:

```text
Can a user safely get back to their encrypted secrets when they need them?
```

That means the MVP reliability model needs five layers:

| Layer | User-visible question | Minimum evidence |
| --- | --- | --- |
| Access | Can a user register, log in, complete MFA, and obtain a session? | Synthetic auth/MFA journey and auth outcome counters. |
| Unlock | Can the browser derive local unlock material without the server seeing vault secrets? | Client KDF tests, unlock failure counters, and no plaintext vault data in backend logs/metrics. |
| Vault data | Can a user create, update, delete, read, and sync encrypted items? | API tests, synthetic write/read journey, conflict counters, and revision continuity checks. |
| Durability | Does an acknowledged saved secret survive rollout, pod restart, database failover, and restore? | HA PostgreSQL state, backup age, restore drill result, failover drill result, and RPO/RTO observations. |
| Abuse resistance | Is attack pressure visible without leaking user identity? | Low-cardinality auth/MFA/rate-limit/CSRF/security counters with no login handles or IDs as labels. |

## Candidate SLIs And SLOs

These are proposed MVP SLO candidates. They are not live-verified in this report.

| Area | SLI | Candidate SLO | Notes |
| --- | --- | --- | --- |
| Public API availability | Ratio of non-health product API requests completing with status `< 500`. | 99.5% over 30 days for MVP preview. | Exclude `/healthz`, `/readyz`, `/metrics`, and unmatched 404s from product availability. |
| Login journey availability | Ratio of synthetic login plus TOTP journeys that reach a session. | 99.0% over 30 days after login/TOTP flow is complete. | This is more meaningful than generic 2xx rate for auth. |
| Vault write/read journey | Ratio of synthetic encrypted item write plus read-back journeys that preserve expected revision state. | 99.0% over 30 days after vault CRUD exists. | Should fail if API returns 200 but content/revision semantics are wrong. |
| Product API latency | p95 successful non-auth product endpoint latency. | p95 under 500 ms, p99 under 1500 ms. | Auth hashing endpoints need separate thresholds. |
| Auth latency | p95 successful auth/MFA endpoint latency. | p95 under 2 s, p99 under 5 s. | Slow KDF/server-side verification is expected but must be bounded. |
| Database durability readiness | Latest restore drill age and latest failover drill age. | Restore and failover drills pass before real secrets; then at least periodic. | This is a release gate more than a request SLO. |
| Backup freshness | Latest successful base backup age and WAL archive health. | Fresh enough to satisfy the selected RPO. | Exact target depends on backup backend and accepted RPO. |
| Scrape health | `up` for expected API targets. | 99.9% over 30 days. | Scrape health is not equivalent to user-facing availability. |

Recommended first error-budget policy:

- If fast 5xx burn, target-down, or synthetic login/write-read failures consume budget rapidly, stop
  feature rollout work and stabilize.
- If only expected attacker/user 4xx traffic rises, create a security/product ticket instead of
  paging unless it causes saturation or server errors.
- Do not gate MVP readiness on vanity metrics. Gate on access, unlock, vault write/read, durability,
  and abuse visibility.

## Technical Metrics

### Already Planned Or Implemented In Product Docs

The existing observability plan correctly starts with low-cardinality HTTP metrics:

- request count by safe route/method/status labels;
- request duration histogram by safe route/method/status labels;
- pending request gauge;
- target scrape health from the monitoring stack.

These are necessary but not sufficient. They prove that the API process is being scraped and that
basic Golden Signals can be rendered.

### Required MVP Additions

| Metric family | Signal | Priority | Why |
| --- | --- | --- | --- |
| `password_vault_build_info` | release context | P0 | Correlates incidents with exact revision/image digest. |
| DB pool connection gauges | saturation | P0 | Detects pool exhaustion before API failures. |
| DB wait/query duration histograms | latency/saturation | P0 | Separates app latency from PostgreSQL latency. |
| DB error counters by safe class | errors | P0 | Shows database faults without exposing SQL values. |
| Auth hash duration histogram | latency/saturation | P0 | Auth work is intentionally expensive and can become DoS pressure. |
| Auth hash active gauge | saturation | P0 | Shows concurrent expensive verification pressure. |
| Rate-limit counters | errors/security | P0 | Proves abusive traffic is being absorbed. |
| CSRF/origin/fetch rejection counters | errors/security | P0 | Shows browser-boundary enforcement without user labels. |
| Background job run/duration counters | errors/latency | P1 | Required for migration, cleanup, backup verification, or future maintenance jobs. |
| Vault operation counters | traffic/errors | P1 | Needed once encrypted item CRUD exists. |
| Sync conflict/stale revision counters | errors/product | P1 | Needed once multi-device sync exists. |

Safe label rule:

- Allowed: route template, method, status class or status code, outcome, operation, safe error
  class, policy name, job name, release revision.
- Forbidden: login handle, email, account ID, device ID, item ID, vault ID, session ID, OTP, raw
  request path, request body, ciphertext, plaintext metadata, private host/IP/domain, secret names
  that reveal environment details.

## Business, Product, And Security Metrics

The first business question is not revenue. For this MVP, it is protected activation:

```text
Can a new user complete registration, enable MFA, unlock locally, and save at least one encrypted
vault item?
```

Recommended aggregate metrics:

| Metric | Category | Priority | Why |
| --- | --- | --- | --- |
| Accounts created by outcome | product | P1 | Registration funnel health. |
| Registration starts/finishes | product | P1 | Detects onboarding breakage. |
| Account secret key confirmed | product/security | P1 | Prevents data-loss-prone onboarding. |
| TOTP enrollment starts/confirmed | product/security | P1 | Measures whether users become protected. |
| Login starts/password-proof/MFA/session-created | product/security | P0/P1 | Separates auth stages and failure causes. |
| TOTP verification success/failure/replay/lockout | security | P0 | Required for MFA abuse and usability. |
| Active sessions | product/security | P1 | Tracks session growth and anomaly spikes. |
| Vault item writes/reads/deletes by outcome | product | P1 | Core product health. |
| First vault item created | product | P1 | True activation milestone. |
| Sync pulls/pushes/conflicts/stale revisions | product | P1 | Multi-device readiness. |
| Recovery-code generation/use/failure | security/product | P2 | Needed when recovery-code flow exists. |
| Restore/failover drill success timestamp | operational/business | P0 | Password-manager trust depends on recoverability. |

Derived dashboard ratios:

- registration completion ratio;
- protected activation ratio;
- login-to-session success ratio;
- MFA success ratio;
- first-secret activation ratio;
- vault write success ratio;
- vault read-after-write success ratio;
- sync freshness success ratio;
- security rejection rate by class;
- backup freshness and restore-drill freshness.

## Current Gaps

This section describes gaps found from documents, not a live-cluster audit.

### P0 Stability Gaps

- Full product journey is not yet the main observable unit. Basic HTTP metrics are necessary, but
  the MVP needs synthetic register/login/MFA/unlock/write/read journeys.
- Database HA remains a hard gate for real secrets. The documented direction is a product-specific
  three-instance CloudNativePG-style cluster with backup, restore, and failover drills.
- Backup and restore are not just infrastructure hygiene. They are product correctness for a password
  manager because losing acknowledged encrypted secrets is a user-visible data-loss incident.
- Startup migrations must stay disabled for real-user environments. Migrations should run as an
  explicit controlled job with prechecks, observability, backup verification, and rollback/restore
  notes.
- Alerting must progress from dashboard-only to actionable alerts: target down, fast 5xx burn,
  synthetic journey failure, database saturation, replication/backup failure, and missing telemetry.
- Metrics exposure must remain internal-only or blocked at public edges. Public `/metrics` exposure
  can leak product and traffic metadata even when labels are low-cardinality.

### P1 Product Observability Gaps

- No complete auth funnel dashboard exists until aggregate login/TOTP/session counters exist.
- No vault CRUD/sync dashboard exists until encrypted item and sync APIs emit safe operation
  counters.
- No release context metric exists unless `password_vault_build_info` or equivalent is added.
- No business-ready activation dashboard exists until first-secret activation and protected
  activation can be measured.
- No load-test scenario should be called product-realistic until it covers at least auth/MFA and
  vault write/read. Health-only load checks are useful smoke tests, not product load tests.

### P2 Maturity Gaps

- WebAuthn/passkey readiness is post-MVP, but the observability model should leave room for
  phishing-resistant factor metrics later.
- Chrome extension and mobile clients are post-MVP, but sync and device metrics should be designed
  now so they do not require a telemetry redesign.
- Security dashboards must avoid per-user drilldown in public-safe metrics. Investigation can use
  redacted audit records with strict access controls, not high-cardinality metric labels.

## Migrations: Why They Still Exist

Stable PostgreSQL versions do not remove application schema migrations.

For `password-vault`, migrations are needed when the product adds or changes:

- auth verifier fields and protocol versions;
- TOTP seed custody metadata and replay-protection state;
- sessions, devices, revocation fields, and audit events;
- vault key-wrap metadata;
- encrypted item revisions, tombstones, cursors, and conflict constraints;
- indexes required for latency SLOs;
- database constraints that enforce account/vault/item isolation.

The operational goal is not frequent risky churn. The goal is reviewed, backward-compatible,
observable schema evolution:

1. expand schema safely;
2. deploy compatible code;
3. backfill through controlled jobs when needed;
4. verify SLOs, errors, and data invariants;
5. contract only in a later release.

## Priority Backlog For Stable MVP

### P0: Minimum Stability Gates

1. Finish login-finish plus login-time TOTP verification and add negative tests for CSRF/fetch-site,
   replay, attempt exhaustion, and generic auth failure behavior.
2. Implement or confirm internal-only metrics exposure through Kubernetes networking and edge rules.
3. Add `password_vault_build_info` or equivalent release/revision metric.
4. Add DB pool, query latency, DB error, auth hash duration, auth hash active, rate-limit, and
   CSRF/security rejection metrics.
5. Add VM/Grafana alert rules for target down, fast 5xx burn, sustained p99 latency, pending request
   growth, and missing scrape data.
6. Add external synthetic probe through the same browser edge route for health/readiness and, once
   implemented, auth/MFA.
7. Move real-secret readiness behind a product-specific HA PostgreSQL plan with backup, restore, and
   failover gates.
8. Keep startup migrations off for real-user environments and create a controlled migration job
   runbook before accepting real data.

### P1: Product-Meaningful Observability

1. Add aggregate registration, login, MFA, session, vault write/read/delete, and sync counters.
2. Add synthetic protected-activation journey: register, save account secret key confirmation,
   enroll TOTP, login, unlock, create first encrypted item, read it back.
3. Add k6 scenarios for health smoke, auth/MFA smoke, and vault CRUD smoke. Keep PR tests small;
   run heavier tests manually or on schedule.
4. Add Grafana rows for auth funnel, MFA health, vault operations, sync conflicts, DB health, backup
   freshness, and release context.
5. Add dashboard review checks that query the real datasource after deployment and record whether
   panels return data.

### P2: Production Hardening

1. Add recovery-code metrics and recovery UX once recovery-code flow is implemented.
2. Add WebAuthn/passkey metrics after the factor design is approved.
3. Add restore/fork/conflict metrics for point-in-time restore scenarios where clients may have
   newer sync cursors than the restored database.
4. Add long-window capacity and storage forecasting for PostgreSQL, object storage, and API CPU.
5. Add alert-delivery drills and incident/runbook exercises.

## Dashboard Shape

The first useful dashboard should be organized by questions, not metric names:

1. Is the user-facing product broken right now?
   - external synthetic status;
   - API availability/error budget;
   - p95/p99 latency;
   - target health.
2. Which journey is broken?
   - registration;
   - login proof;
   - TOTP verify;
   - unlock metadata;
   - vault write/read;
   - sync.
3. Is this capacity or dependency pressure?
   - pending requests;
   - auth hash active work;
   - DB pool wait;
   - DB query latency;
   - CPU/memory;
   - PostgreSQL replication and disk pressure.
4. Is data safe?
   - HA state;
   - backup age;
   - WAL archive health;
   - restore drill age;
   - failover drill age.
5. Is abuse pressure rising?
   - rate-limit hits;
   - CSRF/fetch/origin rejections;
   - TOTP failures/replays/lockouts;
   - session revocation spikes.
6. What changed?
   - deployed revision;
   - image digest;
   - rollout annotations;
   - migration version/status.

## Acceptance Criteria For Observable MVP

The MVP should not be called stable until all P0 criteria are met:

- HTTP Golden Signals return data for API targets.
- Public edge does not expose `/metrics`.
- Dashboard has API availability, request rate, p95/p99 latency, errors, pending requests, and
  target health.
- Alerts exist for target down and fast 5xx burn.
- Auth/MFA synthetic journey exists after the flow is implemented.
- Vault write/read synthetic journey exists after encrypted item CRUD is implemented.
- Metrics labels pass public-safety review.
- Product-specific HA PostgreSQL plan is implemented or real-secret use is explicitly blocked.
- Backup, restore, and failover gates are documented and tested before real secrets.
- Startup migrations are disabled for real-user environments.
- Controlled migration job/runbook exists before real-user schema changes.
- Load tests use synthetic reserved-domain users and fake secrets only.

## Anti-Hallucination And Process Controls

To reduce wasted work and unreliable claims:

- Mark every dashboard, alert, cluster, and test claim as `Verified`, `Assumed`, `Needs
  verification`, or `Not tested`.
- Do not reuse point-in-time cluster findings as current facts without rerunning checks.
- Give subagents disjoint write scopes and a clear output file before starting them.
- Let advisor/reviewer agents finish within the agreed runtime unless they are unsafe or blocked.
- Treat Claude Code and other advisors as evidence sources, not authorities. Record accepted,
  rejected, and deferred recommendations.
- Prefer small SRE increments with validation evidence over broad observability docs that cannot be
  tested.
- Remove metrics or dashboards that do not support an alert, release gate, debugging question, or
  product decision.

## Validation For This Report

Tested:

- Repository documents listed above were inspected.
- Official Google SRE pages listed above were checked.

Not tested:

- Live Kubernetes state.
- Live Grafana dashboard rendering.
- Live VictoriaMetrics queries.
- Argo CD application state.
- Browser access from any client.
- Alert delivery.
- Load-test execution.

Public-safety notes:

- This report intentionally avoids private IPs, hostnames, domains, kubeconfig paths, tokens,
  secrets, live log excerpts, and real user identifiers.
