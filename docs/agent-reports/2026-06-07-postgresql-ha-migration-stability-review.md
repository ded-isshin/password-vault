# PostgreSQL HA Migration Stability Review

Status: draft public-safe analyst report.
Date: 2026-06-07.
Scope: `password-vault` PostgreSQL reliability, schema migration safety, HA readiness, backup and
restore readiness, and zero-downtime release implications.

This report intentionally contains no secrets, private addresses, kubeconfigs, live credentials,
private hostnames, or environment-specific connection strings.

## Executive Summary

Schema migrations are still required even on a stable PostgreSQL version. PostgreSQL stability means
the engine behavior and upgrade policy are predictable; it does not freeze the product data model.
`password-vault` will still need controlled changes for auth protocols, MFA state, session policy,
encrypted key material, vault sync metadata, indexes, constraints, and compatibility windows between
old and new application versions.

The current migration count is not evidence of churn by itself. The first three migrations reflect
normal bootstrap discovery: base schema, registration key material, and browser-compatible KDF
profile correction. The stability target is not "no migrations"; it is "rare, reviewed,
backward-compatible migrations with clear release gates."

For real user secrets, single-instance PostgreSQL is not sufficient. The product should keep the
single PostgreSQL `StatefulSet` pattern only for local, disposable, or explicit no-real-secret demo
environments. Production-like use needs a product-owned CloudNativePG cluster, backup/WAL archiving,
restore drills, failover drills, and migration runbooks before accepting real password-vault data.

Based on the reviewed documents, there is no logical database conflict with another product. The
conflict risk is operational, not conceptual: do not reuse another product's namespace, database,
service, credentials, PVCs, backup object-store prefix, or migration target. A CloudNativePG operator
can be a shared platform component, but the `password-vault` PostgreSQL `Cluster` and backup
resources must be product-specific. The likely current gap is missing product-specific CNPG
resources, and the operator/controller installation still needs live infrastructure verification if
only CRDs are present.

## Inputs Reviewed

- `docs/data-model.md`
- `migrations/202606070001_initial_schema.sql`
- `migrations/202606070002_registration_key_material.sql`
- `migrations/202606070003_browser_pbkdf2_profile.sql`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/research/cloudnativepg-platform-analysis.md`
- Official PostgreSQL versioning, replication, `ALTER TABLE`, and `CREATE INDEX` documentation.
- Official CloudNativePG 1.29 replication, backup, recovery, rolling update, and Barman Cloud Plugin
  documentation.

## Current Schema Readiness

The current schema is directionally appropriate for a single-user encrypted password-vault MVP:

- plaintext server-visible data is limited to account, vault, item IDs, revision metadata,
  timestamps, device/session state, and audit metadata;
- item title, URL, username, password, notes, tags, and custom fields are intended to remain inside
  encrypted item envelopes;
- account keysets and vault key wraps are stored as ciphertext plus non-secret metadata;
- constraints enforce important boundaries such as account-scoped device sessions, account-scoped
  vault key wraps, expected hash lengths, TOTP seed protection metadata, and append-only revision
  sequencing.

Important limitations remain:

- the database cannot prove client-side cryptographic correctness of encrypted envelopes, head
  hashes, or change MACs;
- business logic still has to enforce authorization, optimistic concurrency, and item sync rules;
- future sharing, recovery, WebAuthn, device-specific key wraps, and richer audit semantics may
  require schema expansion;
- migration `202606070003` shows that early bootstrap assumptions can change when browser platform
  constraints are verified.

## Why Migrations Are Required On Stable PostgreSQL

Stable PostgreSQL and schema migrations solve different problems.

PostgreSQL version stability covers the database engine. The PostgreSQL project publishes major
versions with feature changes, supports each major version for five years, and recommends running the
current minor release of the chosen major version. This gives us an engine lifecycle and patching
policy, not a product schema lifecycle.

Application migrations remain necessary because the product owns:

- table creation and table relationships;
- constraints that enforce security boundaries;
- indexes that keep auth, sync, and audit APIs within latency budgets;
- account, MFA, session, vault, and revision state transitions;
- compatibility between old and new application versions during live rollouts;
- deterministic creation of the schema from scratch in CI and disaster recovery;
- safe correction of early design mistakes, such as changing the browser KDF profile after
  implementation evidence invalidates the original assumption.

Without migrations, schema changes become manual drift. Manual drift is worse for reliability: it is
harder to review, harder to reproduce in CI, harder to restore into a clean environment, and harder
to prove during incident response.

## How To Minimize Migration Frequency

The product should reduce avoidable migrations by moving unstable decisions out of rigid columns and
by finalizing protocol contracts before real data.

Recommended controls:

- Freeze a small set of MVP invariants before real-user data: account identity, auth verifier shape,
  TOTP factor shape, session state names, account keyset shape, vault key wrap shape, item revision
  append-only model, and audit event naming.
- Keep encrypted payload formats versioned inside envelopes instead of adding plaintext columns for
  every future item field.
- Prefer explicit `crypto_version`, `key_id`, and protocol profile fields over schema changes for
  every crypto iteration.
- Use JSONB only for bounded public metadata and encrypted envelope content, not for hiding
  important relational ownership rules.
- Avoid speculative tables for organizations, sharing, browser extension state, mobile sync, and
  enterprise policy until the corresponding API contract is ready.
- Batch compatible changes into planned release trains instead of creating a migration for every
  small implementation thought.
- Require a short migration design note for any migration after MVP bootstrap.
- Add database-backed tests that run all migrations from an empty database on every PR.

Migration count is not the main metric. The better metrics are: backward compatibility, lock risk,
rollback/restore clarity, tested restore from scratch, and whether the migration supports a real
feature or invariant.

## How To Minimize Migration Risk

For real-user environments, use an expand/contract policy:

1. Expand: add backward-compatible tables, nullable columns, constraints, or indexes.
2. Deploy application code that can run against both the old and expanded schema.
3. Backfill in a controlled job if data movement is needed.
4. Verify invariants, API errors, latency, replication lag, and audit events.
5. Contract later: drop old columns, remove old constraints, or enforce stricter `NOT NULL` only
   after old application versions are gone.

Practical PostgreSQL guidance for safer migrations:

- avoid table rewrites on large tables during normal traffic;
- avoid long `ACCESS EXCLUSIVE` locks on hot tables;
- use `CREATE INDEX CONCURRENTLY` for large or hot-table indexes when the migration framework can
  run it outside a transaction;
- add expensive constraints as `NOT VALID` first, validate later, and then enforce stricter
  application behavior;
- separate data backfills from schema DDL when the table may grow large;
- measure migration duration on representative data volume before real-user rollout;
- never combine irreversible schema contraction with a first-time application rollout.

`PV_RUN_MIGRATIONS_ON_STARTUP` should remain a local/bootstrap/demo convenience. For real users,
migrations should be an explicit release step or GitOps-controlled job, not a side effect of API pod
startup.

## CNPG And Clustered PostgreSQL Decision

Clustered PostgreSQL is required when the environment may hold real password-vault secrets or needs
to survive one worker-node failure without treating restore as the normal failover path.

Single-instance PostgreSQL is acceptable only for:

- local development;
- disposable CI;
- internal demos with no real secrets;
- early preview environments clearly labeled as non-production and no-real-secret.

CloudNativePG should be used for production-like operation when these are true:

- the Kubernetes cluster has enough worker capacity to spread instances;
- each PostgreSQL instance can use its own PVC;
- application-level PostgreSQL replication is preferred over pretending that local storage is
  portable;
- backup and restore resources are available;
- the team is ready to operate failover, WAL archiving, and restore drills.

Recommended first production-like shape:

- `instances: 3`;
- one primary and two replicas;
- anti-affinity across worker hostnames where capacity allows;
- application connects through the operator-managed read-write service;
- quorum synchronous replication with one synchronous standby;
- `dataDurability: required` for real secrets unless a documented product decision accepts the
  availability/data-loss tradeoff of `preferred`;
- no public PostgreSQL exposure;
- product-specific credentials, database name, namespace/service contract, and backup object-store
  prefix.

Synchronous replication is the better default for a password manager because an acknowledged saved
secret should survive failover. The tradeoff is that writes can pause when the required standby is
unavailable. That is preferable to silently acknowledging data that can disappear after primary loss.

## Other Product Conflict Assessment

No logical schema conflict is indicated by the reviewed material.

The safe interpretation:

- another product's PostgreSQL deployment pattern can coexist with `password-vault`;
- another product's database must not be reused;
- another product's migrations must not target the `password-vault` database, and vice versa;
- backup prefixes and restore targets must be product-specific;
- the CloudNativePG operator can be shared as platform infrastructure, similar to an ingress
  controller or cert manager;
- the `password-vault` database should be a distinct CNPG `Cluster` resource.

The unresolved platform check is whether the CloudNativePG controller/operator is actually installed
and running, not merely whether CRDs exist. CRDs without a running controller do not provide failover,
backup orchestration, or reconciliation. If only CRDs exist, the gap is an infrastructure installation
gap, not an application database conflict.

## Backup And Restore Requirements

Backups are not optional for a password manager, even though vault contents are encrypted. The
server still owns availability of encrypted ciphertext, key wrapping metadata, TOTP factors, sessions,
audit history, and sync revision order.

Minimum real-data gate:

- CloudNativePG cluster resource exists for `password-vault`.
- WAL archiving is enabled.
- Scheduled physical base backups are enabled.
- Backup object-store path is product-specific.
- Retention policy is documented.
- Backup credentials are stored outside the public repository.
- Restore is tested into an isolated namespace or separate cluster object.
- Restored API can connect and pass a controlled smoke test.
- RTO/RPO observations are recorded.
- Alerts cover WAL archive failures, backup failures, replication lag, disk pressure, unavailable
  primary, and restore drill age.

CloudNativePG 1.29 documentation points toward CNPG-I plugins and the Barman Cloud Plugin for object
store backup integration. A valid WAL archive is required for PITR. Recovery should bootstrap a new
cluster from backup rather than overwriting the live cluster during drills.

## Zero-Downtime Release Implications

Zero-downtime for `password-vault` means no planned application outage for compatible changes, not
"every possible database operation is invisible." PostgreSQL primary/standby operations can still
have short failover or switchover effects.

Application-level zero-downtime requirements:

- API replicas must be able to run old and new versions during rolling deployment.
- New code must tolerate old schema during rollout until the expand step is complete.
- Old code must tolerate expanded schema until all old pods are drained.
- Contract migrations must be delayed to a later release.
- Session and auth challenge semantics must remain compatible through rollback windows.
- Feature flags or version checks should gate new paths until schema readiness is verified.

Database-level zero-downtime requirements:

- use controlled migration jobs instead of startup migrations;
- keep schema expand steps short and backward-compatible;
- build large indexes concurrently when needed;
- backfill separately and observe replication lag;
- keep a current base backup and WAL archive before high-risk migrations;
- rehearse app rollback and PITR separately because they solve different failure modes.

CloudNativePG rolling updates help with PostgreSQL minor updates and operator-managed pod cycling:
replicas are updated first, the primary last, and services move endpoints with cluster status.
However, this does not eliminate application schema migration design. Engine rolling updates and
application schema migrations must remain separate release concerns.

## Migration Hygiene Policy

Recommended policy before accepting real secrets:

- Do not run migrations automatically on normal API startup.
- Every migration file must include a matching test path that creates the database from scratch.
- Any migration touching populated tables must include a risk note: lock risk, rewrite risk,
  rollback path, restore dependency, expected duration, and whether it can run in a transaction.
- Prefer additive migrations in the same release as code changes.
- Delay destructive changes by at least one release.
- Keep migration names monotonic and immutable after merge.
- Do not edit already-applied migration files; add a new migration instead.
- Track applied versions through the migration framework.
- Record migration output without logging database URLs, credentials, or secret values.

## Stable Version Policy

Use a supported PostgreSQL major version and current minor release. As of the official PostgreSQL
versioning policy checked on 2026-06-07, supported majors include PostgreSQL 18, 17, 16, 15, and 14;
PostgreSQL 13 is end-of-life. Product CI may use a disposable newer version for compatibility tests,
but deployment should standardize on one supported major version selected in infrastructure.

Recommended near-term standard:

- choose PostgreSQL 17 or 18 deliberately for deployment, not by accidental image drift;
- pin the CNPG-compatible image family in infrastructure;
- treat PostgreSQL major upgrades as separate platform projects;
- allow minor updates through CNPG rolling update policy after reading release notes and passing
  backup/restore gates.

## Findings

### Blocking Before Real Secrets

- Replace the single-instance preview PostgreSQL with a product-owned CNPG cluster.
- Confirm the CNPG controller/operator is installed and reconciling, not only that CRDs exist.
- Configure WAL archiving and scheduled physical base backups.
- Complete a restore drill into an isolated target.
- Complete a primary-failure or switchover drill.
- Disable startup migrations for real-user environments.
- Implement a controlled migration job/runbook.
- Add alerts and dashboard panels for backup status, WAL archiving, replication lag, database
  availability, disk usage, connection pressure, and migration failure.

### Important But Not Blocking For Local MVP

- Clean documentation drift around PostgreSQL image versions.
- Decide PostgreSQL 17 versus 18 for deployment and CI consistency.
- Add migration risk notes for future non-bootstrap migrations.
- Add representative data-volume tests before large-table migrations.
- Document restore behavior for client sync checkpoints after PITR.

### No Conflict Found

- No evidence of a logical conflict with another product database was found in the reviewed docs.
- The correct design is separate product-owned PostgreSQL resources behind a shared platform
  operator, not shared application databases.

## Recommended Next Work Items

1. Infrastructure: install or confirm the CloudNativePG operator/controller and create a
   product-specific `password-vault` CNPG `Cluster`.
2. Infrastructure: add Barman Cloud Plugin or another documented CNPG-supported backup path with a
   product-specific object-store boundary.
3. Product/infrastructure: replace startup migrations with a controlled migration job for
   production-like environments.
4. Product: keep SQL migrations immutable and add short migration risk notes for every
   post-bootstrap migration.
5. Reliability: run a restore drill and document RTO/RPO observations.
6. Reliability: run a one-worker-loss drill and document observed write behavior under synchronous
   replication.
7. Observability: add PostgreSQL panels for availability, replication lag, WAL/archive health,
   backup age, disk pressure, connection count, slow queries, and migration job status.

## Evidence Standard

Verified from local repository files:

- the product has three SQL migrations;
- the data model separates encrypted vault content from server-visible metadata;
- existing docs classify single-instance PostgreSQL as temporary/demo only;
- existing docs recommend product-specific CloudNativePG HA for production-like use;
- existing docs recommend controlled migration jobs and no startup migrations for real users.

Verified from official documentation:

- PostgreSQL major versions and minor versions have separate support and upgrade implications;
- streaming replication is asynchronous by default unless synchronous replication is configured;
- synchronous replication trades write availability for stronger acknowledged-write durability;
- `ALTER TABLE` subcommands can require strong locks, so migration design matters;
- `CREATE INDEX CONCURRENTLY` can avoid locking out writes at the cost of longer work;
- CloudNativePG manages physical streaming replicas declaratively inside a `Cluster`;
- CloudNativePG supports quorum synchronous replication with `method: any` and `number: 1`;
- CloudNativePG `dataDurability: required` can pause writes if required standbys are unavailable;
- CNPG backup direction is moving toward CNPG-I plugins and Barman Cloud Plugin for object stores;
- CNPG recovery/PITR depends on valid WAL archives and bootstraps a new cluster.

Needs live verification:

- exact CNPG operator/controller installation state in the target cluster;
- exact PostgreSQL version used by the target environment after standardization;
- backup object-store availability and credentials path;
- restore duration and failover behavior on the actual home cluster;
- whether current storage classes support volume snapshots or whether object-store backups are the
  only business-continuity mechanism.

## Sources

- PostgreSQL Versioning Policy: <https://www.postgresql.org/support/versioning/>
- PostgreSQL Log-Shipping Standby Servers: <https://www.postgresql.org/docs/current/warm-standby.html>
- PostgreSQL `ALTER TABLE`: <https://www.postgresql.org/docs/current/sql-altertable.html>
- PostgreSQL `CREATE INDEX`: <https://www.postgresql.org/docs/current/sql-createindex.html>
- CloudNativePG 1.29 Architecture: <https://cloudnative-pg.io/docs/1.29/architecture/>
- CloudNativePG 1.29 Replication: <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG 1.29 Backup: <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG 1.29 Recovery: <https://cloudnative-pg.io/docs/1.29/recovery/>
- CloudNativePG 1.29 Rolling Updates: <https://cloudnative-pg.io/docs/1.29/rolling_update/>
- Barman Cloud CNPG-I Plugin Concepts: <https://cloudnative-pg.io/plugin-barman-cloud/docs/concepts/>
