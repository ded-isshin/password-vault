# Decision Brief: PostgreSQL HA, Migrations, And Stability

Status: draft decision brief.

Date: 2026-06-08.

## Scope

This brief clarifies the PostgreSQL high-availability and schema-migration policy for the
`password-vault` MVP.

Inputs inspected:

- `docs/foundational-decisions.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-backup.md`
- `migrations/*.sql`
- `deploy/helm/password-vault`

Official sources checked on 2026-06-08 and refreshed on 2026-06-09:

- CloudNativePG 1.29 backup documentation: <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG 1.29 recovery documentation: <https://cloudnative-pg.io/docs/1.29/recovery/>
- CloudNativePG 1.29 replication documentation: <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG Barman Cloud Plugin 0.12.0 usage:
  <https://cloudnative-pg.io/plugin-barman-cloud/docs/usage/>
- PostgreSQL versioning policy: <https://www.postgresql.org/support/versioning/>
- PostgreSQL 18 table modification documentation: <https://www.postgresql.org/docs/18/ddl-alter.html>
- PostgreSQL 18 `ALTER TABLE` reference:
  <https://www.postgresql.org/docs/18/sql-altertable.html>
- PostgreSQL 18 `CREATE INDEX` reference:
  <https://www.postgresql.org/docs/18/sql-createindex.html>

Runtime state verified after the 2026-06-08 cutover:

- CloudNativePG CRDs and the shared CloudNativePG operator are present in the Kubernetes cluster.
- A product-owned `password-vault-cnpg` CloudNativePG `Cluster` exists with three PostgreSQL 18.4
  instances spread across the three worker nodes.
- The active primary reported `synchronous_commit=on` and
  `synchronous_standby_names=ANY 1 (...)`; `pg_stat_replication` reported both standbys in
  `streaming` state with `sync_state=quorum`.
- The live `password-vault` API is cut over to the `password-vault-cnpg` application Secret.
- The active `password-vault-cnpg` cluster is separate from HiringTrace. HiringTrace still uses a
  separate single PostgreSQL `StatefulSet` in a different namespace and must not be reused by
  Password Vault.
- The current default storage class is `local-path` with node-local RWO volumes and no volume
  expansion. This makes PostgreSQL replication useful for single-worker failure tolerance but does
  not replace off-node backups.
- The legacy single PostgreSQL `StatefulSet` may remain briefly as rollback debt, but it is no
  longer the active API database.
- There are currently no CloudNativePG `Backup` or `ScheduledBackup` resources for `password-vault`,
  and no usable base backup timestamp is visible in the live CNPG metrics.
- The Barman Cloud Plugin and cert-manager are deployed as platform foundation, but the Password
  Vault cluster does not yet have an object-store backup target, runtime object-store credentials,
  scheduled base backups, or restore evidence.
- The current `hiringtrace` PostgreSQL deployment, if present, is a separate product runtime and
  must not be reused as a `password-vault` database, schema, credential source, PVC, or backup
  target.

Additional runtime state refreshed on 2026-06-09:

- CloudNativePG operator version is `1.29.1`, and the Barman Cloud Plugin deployment version is
  `0.12.0`.
- `password-vault-cnpg` still reports three ready instances and a healthy phase.
- Live PostgreSQL state reports `synchronous_commit=on`,
  `synchronous_standby_names=ANY 1 (...)`, and both standbys in `streaming` / `quorum` state.
- No `ObjectStore`, `Backup`, or `ScheduledBackup` exists in the `password-vault` namespace.
- No backup/S3/Barman credential Secret exists in the `password-vault` namespace.
- The cluster API does not expose volume snapshot resources, so object-store backup through the
  Barman Cloud Plugin is the practical durability path for this environment.
- Grafana/VictoriaMetrics reported CNPG targets `3`, streaming replicas `2`, replication lag `0`,
  and backup availability `0`.

This brief is a product decision document. It does not change code, Helm manifests, infrastructure,
Kubernetes resources, or runtime secrets.

This is public-repository safe: it uses product and Kubernetes resource categories only. It does
not include secrets, private IP addresses, kubeconfigs, private hostnames, or live connection
strings.

## Decision Summary

For real password-vault data, the product needs clustered PostgreSQL. A single PostgreSQL
`StatefulSet` is acceptable only for bootstrap, preview, and disposable demos that must not receive
real user secrets.

Use one product-owned CloudNativePG `Cluster` per product and namespace. Sharing the CloudNativePG
operator is normal platform reuse; sharing another product's database, credentials, PVCs, backup
prefixes, or schema is not.

For a single-site Kubernetes cluster with three worker nodes, the recommended production-like mode
is three CloudNativePG instances with quorum synchronous replication:

```yaml
spec:
  instances: 3
  postgresql:
    synchronous:
      method: any
      number: 1
      dataDurability: required
```

When verifying this mode, prefer live PostgreSQL state over legacy-looking CRD defaults. In the
current CNPG API shape, `.spec.postgresql.synchronous` is the intended configuration. Seeing
`.spec.minSyncReplicas=0` and `.spec.maxSyncReplicas=0` is not by itself evidence that the database
is asynchronous. Verify with:

- `SHOW synchronous_commit`;
- `SHOW synchronous_standby_names`;
- `pg_stat_replication.state`;
- `pg_stat_replication.sync_state`.

Backups must include continuous WAL archiving plus physical base backups to an object-store-backed
backup target before any real user secrets are accepted. Restore must be drilled into a separate
namespace or separate `Cluster` object before the environment is considered production-like.

Live PostgreSQL archiver counters can show zero failures while the base-backup gate is still red.
That is expected: WAL archive health, base backup availability, PITR readiness, restore drills, and
failover drills are separate controls. Do not treat pod readiness, synchronous replication, or a
healthy archiver counter as proof that the database is recoverable after data corruption, operator
mistake, node-local volume loss, or a bad migration.

Schema migrations remain required even with stable PostgreSQL versions. PostgreSQL engine stability
does not remove the need to version application-owned tables, constraints, indexes, authentication
fields, sync metadata, and crypto/key-wrapping metadata. The goal is not "no migrations"; the goal
is fewer, deliberate, backward-compatible migrations that support live rollout.

For the current stabilization phase, treat the existing three SQL migrations as the bootstrap schema
history for the browser MVP and freeze the schema by default. Do not add another migration just
because an idea is plausible. A new migration must be tied to one of these reasons:

- a P0 security or data-integrity fix;
- a missing field or constraint required by already accepted browser-MVP behavior;
- a migration needed to complete backup/restore/failover validation safely;
- a backward-compatible prerequisite for a reviewed release that cannot be represented in the
  current schema.

The stable-software requirement applies to PostgreSQL and operators too: use supported PostgreSQL
major/minor releases and supported CloudNativePG/Barman plugin versions, but do not confuse engine
patching with application schema churn.

PostgreSQL version policy and application schema policy are separate:

- use a supported PostgreSQL major version and keep it on current minor releases;
- treat PostgreSQL major upgrades as platform/database projects;
- treat SQL migrations as product data-contract changes;
- do not add migrations for speculative ideas or cosmetic churn;
- do add migrations when persisted authentication, security, sync, audit, or encrypted-vault
  metadata must change.
- after real user data exists, do not edit already-applied migration files; add a new forward-only
  migration with an explicit rollout and rollback-compatibility note.

## Why Clustered PostgreSQL Is Required

The application is a password manager. A user saving a password expects an acknowledged write to
survive ordinary platform failure. Losing an acknowledged saved secret after a worker failure is a
product failure, not a minor availability event.

A single PostgreSQL `StatefulSet` has these limits:

- no database-level failover target;
- no replica that can be promoted after primary loss;
- one PostgreSQL pod is the write and availability bottleneck;
- with node-local storage, the volume is tied to the worker where it was provisioned;
- API replicas can continue running, but they cannot make a single database pod highly available.

CloudNativePG addresses the database layer through PostgreSQL application-level replication and
operator-managed failover. With three instances spread across workers, a single worker failure can
leave a promoted or existing primary plus at least one remaining replica, assuming scheduling and
storage placement are healthy.

This does not replace backups. On the current `local-path` storage class, PostgreSQL volumes are
node-local. CloudNativePG replication is necessary because it gives PostgreSQL a promotable data
copy on another worker, but it is still not a complete durability story. HA handles common database
instance, pod, and some worker-failure scenarios. Backups and PITR handle node-local storage loss,
data corruption, accidental deletes, bad migrations, credential mistakes, operator mistakes, and
disaster recovery.

## Path From Preview StatefulSet To CloudNativePG

The previous single PostgreSQL `StatefulSet` was a preview data source, not the final database
topology. There is no safe "turn the StatefulSet into a CloudNativePG cluster" shortcut. The staged
cutover path below remains the audit trail and template for future database moves.

Recommended path:

1. Confirm the shared CloudNativePG operator through infrastructure GitOps.
2. Keep the product-owned `password-vault-cnpg` CloudNativePG cluster healthy with three
   instances, separate PVCs, product-specific Services, product-specific Secrets, no public
   PostgreSQL exposure, monitoring, and NetworkPolicy.
3. Select and document the backup target, credentials path, retention, and restore-drill namespace.
4. Enable continuous WAL archiving and scheduled base backups.
5. Run and document restore and failover drills against a non-live target.
6. Run the product schema migrations against the new cluster using an explicit migration job, not API
   pod startup.
7. If preview data needs to be preserved, perform a reviewed dump/import or logical migration from
   the preview `StatefulSet`; otherwise treat preview data as disposable and initialize from
   migrations.
8. Cut the API over by changing the runtime database Secret/Service reference through GitOps, then
   roll API pods with `maxUnavailable: 0`.
9. Validate health, readiness, synthetic user journeys, backup status, replication state,
    failover behavior, and dashboards.
10. Keep the old preview `StatefulSet` quarantined for a short rollback window; remove it only after
    the cutover and restore evidence are recorded.

The API cutover is complete for the current preview, but real-secret use remains blocked until the
backup target, successful base backup, WAL archive health, restore drill, failover drill, and
runtime Secret handling are proven.

## No Conflict With HiringTrace Or Other Products

There is no inherent conflict with HiringTrace or another product using PostgreSQL if the boundary
is one product-owned database deployment per product and namespace.

Required isolation:

- separate namespace or clearly separated product ownership boundary;
- separate CloudNativePG `Cluster` resource;
- separate PostgreSQL database and role;
- separate Kubernetes Secrets for connection URLs and credentials;
- separate Services and NetworkPolicies;
- separate PVCs;
- separate object-store path or bucket prefix;
- separate restore drills;
- no cross-product application migrations;
- no password-vault tables inside another product's database.

The CloudNativePG operator itself can be shared cluster infrastructure. The database clusters it
manages should be product-owned. This is the same distinction as sharing Kubernetes but not sharing
application Secrets or tables.

A conflict would exist only if `password-vault` tried to reuse HiringTrace's PostgreSQL database,
role, connection Secret, PVCs, backup prefix, migration pipeline, or operational ownership. That is
not the recommended architecture. The safe shared layer is the operator and Kubernetes platform, not
the product data plane.

## Sync Versus Async Replication

### Recommendation

For real password-vault data in a single-site three-worker cluster, use synchronous quorum
replication with one synchronous standby and `dataDurability: required`.

Why:

- asynchronous replication can acknowledge a write before any standby has received the WAL;
- if the primary fails during replication lag, the product can lose a recently saved password or
  auth/security state;
- synchronous replication makes commits wait for the required standby acknowledgement;
- with three instances, requiring one synchronous standby is the practical durability baseline;
- if the cluster degrades too far, pausing writes is preferable to acknowledged data loss.

Expected tradeoff:

- one worker failure should be tolerated when instances are spread across nodes;
- if only the primary remains without a suitable standby, writes may pause;
- with `dataDurability: required`, this write pause is intentional: the system should prefer
  temporary write unavailability over acknowledging a saved secret that can be lost after primary
  failure;
- degraded write unavailability must alert loudly and be handled operationally;
- this is acceptable for real secrets because durability is the higher priority.

This choice is specifically for password-manager writes. A generic CRUD application may choose
availability over write durability more often; this product should not silently acknowledge saved
secrets that can disappear after a primary failure.

### Where Async Is Acceptable

Asynchronous replication or a single instance is acceptable for:

- local development;
- CI and disposable test environments;
- public demo environments explicitly labeled "no real secrets";
- load-test environments where data loss is expected and documented.

Asynchronous replication is not the default for a password manager that accepts real user secrets.

`dataDurability: preferred` can be evaluated later if we prove through failure testing that write
availability during degraded states is more important than strict acknowledged-write durability. That
would be a deliberate risk acceptance, not the baseline.

## Backup And PITR Requirements

Minimum production-like backup requirements:

- object-store backup target selected;
- CloudNativePG Barman Cloud Plugin or another reviewed CloudNativePG-supported backup path
  selected;
- continuous WAL archiving enabled;
- scheduled physical base backups enabled;
- backup retention policy defined;
- backup and restore alerts defined;
- backup credentials stored only as Kubernetes/runtime secrets, never in this public repository;
- the application database Secret, TOTP seed protection key, and synthetic metadata key have a
  documented restore path;
- restore drill completed into a separate namespace or separate `Cluster` object;
- restored cluster verified by a controlled application connection;
- RTO and RPO observations recorded.

WAL archive alone is not enough. PITR needs a recoverable base backup plus WAL replay. A base backup
without WAL archiving gives coarser recovery and cannot recover to an arbitrary point after the base
backup.

CloudNativePG recovery should be treated as creating a new recovered cluster from backup material,
not overwriting the live database during the first drills.

Backup/restore is a release gate, not documentation only. Real user secrets remain blocked until:

- at least one scheduled backup has completed successfully;
- WAL archiving health is observable;
- restore into a non-live target has been completed;
- the restored database can run the application schema and a controlled application connection;
- the restore result records observed RTO, observed RPO, and any missing Secrets or manual steps.

The current blocker is not a conflict with another product. The blockers are missing backup target,
missing backup credentials, missing scheduled base backups, missing restore drill, missing failover
drill, and node-local storage. Sharing the CloudNativePG operator is acceptable platform reuse;
sharing product databases, credentials, PVCs, backup prefixes, or migrations is not. Treat "three
CNPG instances are healthy" as an HA signal, not as proof that real password data is recoverable.

## Why Schema Migrations Are Still Required

Stable PostgreSQL versions and schema migrations solve different problems.

Stable PostgreSQL means the database engine behavior is predictable. It does not define the product
schema and it does not evolve application data contracts.

PostgreSQL's versioning policy covers engine releases: major versions introduce new features and
can require dump/reload or `pg_upgrade`, while minor releases contain fixes and should normally be
kept current for the selected major version. That is not the same lifecycle as changing
`password-vault` tables, indexes, constraints, auth state, sync state, or key-wrapping metadata.

This product already has real schema evolution pressure:

- `202606070001_initial_schema.sql` defines accounts, devices, sessions, vaults, item revisions,
  TOTP factors, recovery codes, and audit events.
- `202606070002_registration_key_material.sql` adds device client metadata, session expiry fields,
  account keysets, and vault key wraps.
- `202606070003_browser_pbkdf2_profile.sql` changes the supported browser KDF profile while keeping
  the database constraints explicit.

Those changes are application semantics, not PostgreSQL engine upgrades.

Migrations are needed for:

- deterministic schema creation from scratch in CI and new environments;
- constraints that enforce security boundaries;
- indexes required for latency and uniqueness guarantees;
- auth protocol changes;
- key-wrap and crypto metadata changes;
- multi-device sync metadata changes;
- controlled compatibility between old and new API replicas during live rollout;
- safe rollback and restore reasoning.

Removing migrations would not make the product more stable. It would make schema drift harder to
audit, harder to test, and harder to recover.

The stable target is therefore:

- one intentional, reviewed migration chain;
- migration files committed with product code;
- CI proving a clean database can be created from scratch;
- production-like rollout using explicit migration jobs;
- no implicit DDL from API pod startup;
- no unmanaged manual schema edits.

For MVP stabilization, the practical posture is a schema freeze by default: do not add new database
migrations unless they directly support a required security invariant, recovery/durability gate,
MVP user journey, or live-rollout safety fix. PostgreSQL minor-version maintenance remains a
platform/database patching concern and should not be confused with product schema churn.

This answers the common concern that "stable PostgreSQL should mean no migrations." Stable
PostgreSQL gives a supported engine and predictable operational behavior. It does not freeze the
application data contract. Password Vault still needs a versioned schema for authentication state,
MFA state, sessions, encrypted vault metadata, item revision chains, constraints, and indexes. The
anti-waste rule is to keep migrations rare and intentional, not to remove the migration system.

This means "rare migrations", not "no migrations". No-migration policy would force manual schema
drift or oversized up-front schema guesses. Frequent speculative migrations would create review and
rollback noise. The stable target is a short, immutable, reviewed migration chain with CI proof and
explicit GitOps execution for production-like environments.

## How To Minimize Migration Churn

The policy is deliberate migrations, not frequent schema churn.

Rules:

- prefer a well-reviewed initial schema for stable core concepts;
- avoid schema changes for cosmetic naming or speculative future features;
- keep encrypted item payloads flexible enough for client-side encrypted content evolution;
- keep security-critical server metadata explicit with constraints rather than hiding everything in
  unvalidated JSON;
- batch related backward-compatible additions into one release when practical;
- require a decision note for destructive or high-lock migrations;
- require load-test and restore evidence before large production data migrations;
- do not use database migrations as a substitute for unsettled product design.

Some migrations are still the safest path. For example, adding an explicit constraint or index can
be safer than trusting every API code path forever.

Do not add a migration when the change can be handled safely as:

- application-only validation with no persisted contract change;
- a feature flag over already-existing columns;
- encrypted client payload evolution that does not require new searchable/server-visible metadata;
- a documentation or API-shape clarification that does not change stored data.

Add a migration when the database must enforce a new invariant, store new security metadata, support
a new sync/authentication state, or provide an index/constraint required for reliable latency and
uniqueness.

## Live Rollout Migration Policy

Use expand/contract for real-user environments.

1. Expand: add backward-compatible columns, tables, constraints, or indexes.
2. Deploy code that works with both old and new schema.
3. Backfill with a controlled job if needed.
4. Observe API health, database health, latency, error rates, and business invariants.
5. Contract later: remove old columns or code paths only after the previous version is gone and
   rollback no longer needs the old shape.

Production application pods must not run schema migrations during normal startup.

The chart already reflects this direction:

- `config.runMigrationsOnStartup` defaults to `false`;
- a migration `Job` exists behind explicit `migrations.job.enabled`;
- the chart requires the migration job to be an Argo CD hook when rendered through that path;
- the migration job runs the same application image with `password-vault-api migrate`;
- API rollout defaults use rolling updates with `maxUnavailable: 0`.

Operational rules:

- use migration jobs or reviewed operator steps for production-like schema changes;
- run backups and confirm restore readiness before destructive or high-risk migrations;
- prefer additive nullable columns first, then backfill, then enforce `NOT NULL` later;
- use PostgreSQL concurrent index creation for large live tables where appropriate;
- avoid `DROP`, `RENAME`, type rewrites, and broad constraint validation in the same release that
  introduces the new code path;
- do not combine irreversible schema change with a new unproven application behavior;
- keep migration logs free of connection strings and secret values.
- treat a migration that only supports a speculative future feature as backlog waste until the
  feature becomes part of the accepted MVP scope.

PostgreSQL DDL can take locks. Some `ALTER TABLE` forms acquire strong locks, and normal index
creation can block writes. This is why online-compatible migrations must be reviewed as operational
changes, even on stable PostgreSQL.

## MVP Stabilization Implications

Blocking before real user secrets:

- keep the active product-owned CloudNativePG `Cluster` healthy with three PostgreSQL instances
  spread across workers;
- keep synchronous replication with one required synchronous standby verified through
  `pg_stat_replication`;
- configure scheduled base backups and keep WAL archive health observable;
- run and document restore and failover drills into non-live targets where appropriate;
- define alerts for replication health, backup failures, WAL archive failures, disk pressure,
  migration-job failures, and database unavailability;
- keep startup migrations disabled for real-user API pods.

Non-blocking for the current preview:

- keep the legacy single PostgreSQL `StatefulSet` only as a short rollback artifact while the
  environment is explicitly treated as preview/no-real-secrets;
- use asynchronous or disposable database modes for CI and load testing;
- defer destructive contract migrations until there is real production data and a mature rollout
  process;
- refine RTO/RPO targets after the first restore and failover drills produce measurements.

## Sources

Local sources:

- `docs/foundational-decisions.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-backup.md`
- `migrations/202606070001_initial_schema.sql`
- `migrations/202606070002_registration_key_material.sql`
- `migrations/202606070003_browser_pbkdf2_profile.sql`
- `deploy/helm/password-vault/README.md`
- `deploy/helm/password-vault/templates/migration-job.yaml`
- `deploy/helm/password-vault/values.yaml`

Official documentation:

- https://cloudnative-pg.io/docs/1.29/replication/
- https://cloudnative-pg.io/docs/1.29/backup/
- https://cloudnative-pg.io/docs/1.29/recovery/
- https://cloudnative-pg.io/plugin-barman-cloud/docs/usage/
- https://www.postgresql.org/support/versioning/
- https://www.postgresql.org/docs/current/sql-altertable.html
- https://www.postgresql.org/docs/current/sql-createindex.html
