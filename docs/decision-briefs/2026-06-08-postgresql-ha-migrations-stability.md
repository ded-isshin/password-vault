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

Runtime assumption for this decision:

- CloudNativePG CRDs are present in the Kubernetes cluster.
- There are currently no CloudNativePG `Cluster` resources for `password-vault`.
- The current `password-vault` database is a single PostgreSQL `StatefulSet`.
- The current `hiringtrace` PostgreSQL deployment, if present, is a separate product runtime and
  must not be reused as a `password-vault` database, schema, credential source, PVC, or backup
  target.

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

Backups must include continuous WAL archiving plus physical base backups to an object-store-backed
backup target before any real user secrets are accepted. Restore must be drilled into a separate
namespace or separate `Cluster` object before the environment is considered production-like.

Schema migrations remain required even with stable PostgreSQL versions. PostgreSQL engine stability
does not remove the need to version application-owned tables, constraints, indexes, authentication
fields, sync metadata, and crypto/key-wrapping metadata. The goal is not "no migrations"; the goal
is fewer, deliberate, backward-compatible migrations that support live rollout.

PostgreSQL version policy and application schema policy are separate:

- use a supported PostgreSQL major version and keep it on current minor releases;
- treat PostgreSQL major upgrades as platform/database projects;
- treat SQL migrations as product data-contract changes;
- do not add migrations for speculative ideas or cosmetic churn;
- do add migrations when persisted authentication, security, sync, audit, or encrypted-vault
  metadata must change.

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

This does not replace backups. HA handles common instance or node failures. Backups and PITR handle
data corruption, accidental deletes, bad migrations, credential mistakes, operator mistakes, and
disaster recovery.

## Path From Preview StatefulSet To CloudNativePG

The current single PostgreSQL `StatefulSet` should be treated as a preview data source, not as the
final database topology. There is no safe "turn the StatefulSet into a CloudNativePG cluster"
shortcut. The migration should be staged and reversible.

Recommended path:

1. Install or confirm the shared CloudNativePG operator through infrastructure GitOps.
2. Select and document the backup target, credentials path, retention, and restore-drill namespace.
3. Create a product-owned `password-vault` CloudNativePG `Cluster` with three instances, separate
   PVCs, product-specific Services, product-specific Secrets, and no public PostgreSQL exposure.
4. Configure synchronous replication, node spread/anti-affinity, resource requests, monitoring, and
   NetworkPolicies before accepting real user secrets.
5. Enable continuous WAL archiving and scheduled base backups.
6. Run the product schema migrations against the new cluster using an explicit migration job, not API
   pod startup.
7. If preview data needs to be preserved, perform a reviewed dump/import or logical migration from
   the preview `StatefulSet`; otherwise treat preview data as disposable and initialize from
   migrations.
8. Run a restore drill into a separate namespace or separate `Cluster` object before cutover.
9. Cut the API over by changing the runtime database Secret/Service reference through GitOps, then
   roll API pods with `maxUnavailable: 0`.
10. Validate health, readiness, synthetic user journeys, backup status, replication state,
    failover behavior, and dashboards.
11. Keep the old preview `StatefulSet` quarantined for a short rollback window if data was migrated;
    remove it only after the cutover and restore evidence are recorded.

The cutover must be blocked if the backup target, WAL archiving, restore drill, or runtime Secret
handling is incomplete.

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

PostgreSQL DDL can take locks. Some `ALTER TABLE` forms acquire strong locks, and normal index
creation can block writes. This is why online-compatible migrations must be reviewed as operational
changes, even on stable PostgreSQL.

## MVP Stabilization Implications

Blocking before real user secrets:

- replace the single PostgreSQL `StatefulSet` with a product-owned CloudNativePG `Cluster`;
- spread three PostgreSQL instances across workers where capacity allows;
- configure synchronous replication with one required synchronous standby;
- configure continuous WAL archiving and scheduled base backups;
- run and document a restore drill into a separate namespace or cluster object;
- define alerts for replication health, backup failures, WAL archive failures, disk pressure,
  migration-job failures, and database unavailability;
- keep startup migrations disabled for real-user API pods.

Non-blocking for the current preview:

- keep the single PostgreSQL `StatefulSet` only if the environment is explicitly treated as
  bootstrap/demo and no real secrets are stored;
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
- https://cloudnative-pg.io/plugin-barman-cloud/docs/intro/
- https://www.postgresql.org/support/versioning/
- https://www.postgresql.org/docs/current/sql-altertable.html
- https://www.postgresql.org/docs/current/sql-createindex.html
