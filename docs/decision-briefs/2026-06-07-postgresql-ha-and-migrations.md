# Decision Brief: PostgreSQL HA And Migrations

Status: draft.

## Scope

Analyze the MVP PostgreSQL strategy for `password-vault`: temporary single-instance posture,
relationship to the existing product database footprint, CloudNativePG-style HA direction,
synchronous versus asynchronous replication, backup and restore gates, and application schema
migration policy.

Active context: `password-vault + infrastructure-home`, read-only for infrastructure.

Public-repository safety: this brief intentionally uses public product names, generic Kubernetes
resource patterns, and placeholder-style names only. It includes no secrets, private IPs,
kubeconfigs, live logs, or private hostnames.

## Short Answer

The MVP should treat any single PostgreSQL `StatefulSet` as temporary bootstrap or demo
infrastructure only. It is not acceptable for real password-vault user secrets because node-local
storage and a single database pod provide no single-worker failure tolerance.

There is no required sharing with another product database. The existing observed product database
footprint is app-specific to another product and should remain isolated. `password-vault` should use
its own namespace, database identity, Kubernetes secrets, service names, CloudNativePG cluster, and
backup object-store prefix or bucket path.

For production-like use, move to a CloudNativePG-style three-instance PostgreSQL cluster with
instances spread across workers. For real password-vault data, prefer quorum synchronous replication
with one synchronous standby and required durability. Asynchronous replication is acceptable only
for local development, disposable demos, or a clearly labeled public demo that forbids real secrets.

Application schema migrations remain mandatory even when the PostgreSQL engine version is stable.
Stable PostgreSQL means the database server behavior is predictable; it does not create or evolve
product tables, constraints, indexes, auth fields, encrypted sync metadata, or tenant-boundary
relationships.

## Current State

Implemented:

- Product direction is PostgreSQL as the source of truth for server-side product state.
- Product direction is CloudNativePG for production-like Kubernetes PostgreSQL.
- The product Helm chart deploys the API only. PostgreSQL, backup resources, runtime secret
  creation, and production values remain infrastructure responsibilities.
- The Helm default is `config.runMigrationsOnStartup: false`.
- The API supports SQLx migrations and a `PV_RUN_MIGRATIONS_ON_STARTUP` switch for local or
  bootstrap use.
- Root SQL migrations exist under `migrations/`.
- CI and local database-backed tests can validate migrations against disposable PostgreSQL when
  `PV_TEST_DATABASE_URL` is configured.
- Current CI uses a disposable `postgres:18-alpine` service for database-backed tests. Some older
  development text may still mention earlier PostgreSQL 17 test images and should be cleaned up in a
  documentation consistency pass.
- Infrastructure read-only review found an existing single PostgreSQL `StatefulSet` pattern for
  `hiringtrace`, scoped to that product's namespace and secret contract.
- Infrastructure read-only review found no shared data operator currently enabled from the platform
  data bootstrap directory.
- Live cluster review on 2026-06-07 found CloudNativePG CRDs installed, but no active
  `clusters.postgresql.cnpg.io` resources.
- Live cluster review on 2026-06-07 found the password-vault preview database running as one
  `postgres:17-bookworm` StatefulSet replica with a `local-path` PVC. This is bootstrap/demo
  infrastructure, not HA.
- Live infrastructure values currently set startup migrations off for the password-vault preview.
  That should stay true for real-user data; future schema-changing releases need a controlled
  migration job or another reviewed operator step.

Planned:

- No password-vault CloudNativePG `Cluster` is implemented in this product repository.
- No password-vault production database, backup target, WAL archiving, scheduled base backups, or
  restore drill is implemented here.
- No controlled production migration `Job` or GitOps runbook has been implemented for
  password-vault real-user rollouts.
- CloudNativePG operator installation and environment-specific password-vault database resources
  remain infrastructure work.

## Why A Single StatefulSet Is Temporary

A single PostgreSQL `StatefulSet` is useful for fast bootstrap because it is easy to understand and
requires little platform machinery. It is also a reasonable local, disposable, or non-real-data demo
shape.

It is temporary for password-vault real data because:

- one database pod has no database-level failover target;
- with `local-path` storage, the PVC is node-local and does not become portable storage;
- if the worker hosting the database volume is unavailable, another worker cannot simply remount the
  same volume and continue;
- a single pod plus local storage makes backup and restore the only recovery path for worker-loss or
  volume-loss events;
- password-manager writes are user-visible saved secrets, so acknowledged write loss is a product
  failure, not just a metrics gap.

The temporary label applies even if the application layer has multiple replicas. API replicas can
reschedule, but a single node-local PostgreSQL primary remains the availability bottleneck.

## Other Product Database Conflict

The current infrastructure footprint includes the `hiringtrace` PostgreSQL deployment pattern. That
does not create a schema or runtime conflict for password-vault if the boundary stays strict:

- do not reuse the other product's namespace;
- do not reuse its PostgreSQL `StatefulSet`, service, secret, database name, user, or PVCs;
- do not place password-vault tables into another product database;
- do not run password-vault migrations against another product database;
- do not share object-store backup prefixes between products;
- do not expose PostgreSQL publicly for either product.

The real risk is not direct conflict; it is copying a temporary single-`StatefulSet` pattern into a
password manager and treating it as production-ready. The `hiringtrace` database can remain an
app-specific implementation detail. Password-vault should use a separate CloudNativePG-managed
cluster for production-like operation.

CloudNativePG itself can be a shared platform operator. The password-vault database should still be a
separate product-owned cluster resource with its own credentials, services, backup target prefix, and
restore drills. Sharing the operator is normal; sharing another product's database is not.

## Recommended HA Direction

Use a CloudNativePG-style PostgreSQL cluster for production-like password-vault environments:

- three PostgreSQL instances;
- pod scheduling spread across worker hostnames where capacity allows;
- one primary and at least two replicas;
- failover managed by the PostgreSQL operator;
- each PostgreSQL instance has its own PVC;
- local-path storage is acceptable only as node-local instance storage, not as replicated storage;
- no public PostgreSQL exposure;
- app connects through the operator-provided read-write service or an environment-owned service
  contract;
- database secrets and production values stay out of this public product repository.

The platform-level operator can be a shared infrastructure dependency. The application database
cluster should still be product-specific unless the platform later defines a managed database service
with strong isolation, backup boundaries, and migration ownership.

## Sync Versus Async Replication

For password-vault real user data, prefer synchronous replication with one synchronous standby and
required durability:

```yaml
spec:
  instances: 3
  postgresql:
    synchronous:
      method: any
      number: 1
      dataDurability: required
```

Rationale:

- asynchronous replication can acknowledge a write on the primary before a standby has received it;
- if the primary fails during that lag window, a saved password, TOTP enrollment state, session
  mutation, or sync revision can be lost;
- synchronous replication with required durability can pause writes when the required standby is not
  available, but it better matches the product expectation that acknowledged saved secrets survive
  failover;
- temporary write unavailability during degraded operation is easier to explain than acknowledged
  data loss.

Allowed modes:

| Environment | Replication mode | Real secrets allowed? |
| --- | --- | --- |
| Local dev | local PostgreSQL or single disposable instance | No |
| Kubernetes dev | one instance or asynchronous replicas | No |
| Public demo | asynchronous or single instance only with explicit no-real-secrets labeling | No |
| Production-like test | three instances, synchronous preferred | No, until gates pass |
| Public real-data | three instances, synchronous with required durability | Yes, after gates pass |

`dataDurability: preferred` can be evaluated after failure testing if write availability during
degraded states is more important than strict acknowledged-write durability. That would be a product
risk acceptance, not a default.

## Backup And Restore Gates

No real password-vault user secrets should be accepted until these gates pass:

- CloudNativePG/operator version selected and documented.
- Password-vault database resources are product-specific and not shared with another product.
- Object-store backup target selected.
- Continuous WAL archiving enabled.
- Scheduled physical base backups enabled.
- Backup credentials and runtime secret custody documented without committing secret values.
- TOTP seed encryption key and synthetic metadata key restore path documented and tested.
- Restore drill completed into a separate namespace or separate cluster object.
- Restored database verified by controlled application connection.
- Failover drill completed for primary loss and standby loss.
- RTO and RPO observations recorded.
- Alerts exist for backup failure, WAL archive failure, replication health, disk pressure, and
  failing readiness.
- Public ingress exposes only the application, never PostgreSQL.

WAL archiving alone is not enough. Restore needs a physical base backup plus WAL replay. Restore
drills must not overwrite the live cluster during initial validation.

## Why Schema Migrations Still Matter

Stable PostgreSQL versions reduce engine drift. They do not remove the need for application schema
migrations.

Password-vault still needs migrations because:

- product tables and constraints are application-owned, not PostgreSQL-owned;
- auth protocol fields, MFA state, session policy, account keysets, vault key wraps, and sync
  revision metadata will evolve;
- new indexes may be required for API latency or uniqueness guarantees;
- security boundaries often live in database constraints, not only application code;
- compatibility windows are needed while old and new API versions overlap during rollout;
- CI needs deterministic proof that the schema can be created from scratch.

Database engine upgrades and application schema migrations are separate change types. A stable
PostgreSQL major version is compatible with frequent, reviewed application schema migrations.

## Migration Policy

Use expand/contract migrations for real-user environments:

1. Expand: add backward-compatible tables, columns, constraints, or indexes.
2. Deploy application code that works with both old and new schema.
3. Backfill with a controlled job when needed.
4. Verify readiness, traffic, error rates, migration status, and data invariants.
5. Contract in a later release after old application versions and old code paths are gone.

Do not drop or rename columns in the same release that first requires the new shape. Do not combine
irreversible schema changes with unproven application behavior.

## Controlled Migration Job

Production migrations should run as an explicit, controlled job or equivalent GitOps-approved
operator action:

- run the migration using the exact application image or reviewed migration image for the release;
- read database credentials from the password-vault database secret;
- run before the application rollout when the expand step is required by the new code;
- serialize migration execution through the migration framework or database locking;
- fail closed on migration error;
- emit logs that are safe to publish and do not include connection URLs or secret values;
- keep the migration job separate from normal API replica startup;
- record which migration versions were applied.

`PV_RUN_MIGRATIONS_ON_STARTUP` is acceptable for local development, disposable environments, or a
human-approved bootstrap. It should remain disabled for real users.

## No Startup Migrations For Real Users

Application pods should not run production migrations during normal startup because:

- multiple API replicas can race or amplify a migration failure;
- a rollout can turn a schema issue into an application crash loop;
- startup migrations blur deployment health with data-plane mutation;
- rollback becomes ambiguous if the app and schema changed at the same time;
- migrations need prechecks, backups, observability, and operator attention.

Real-user rollouts should make migration a deliberate release step, not a side effect of starting an
API pod.

## Rollback And Restore Considerations

Every migration that touches real data needs a rollback or restore note before it runs:

- verify the latest backup and WAL archive status before the migration;
- prefer reversible expand steps and delayed contract steps;
- rollback the app only if the previous app version is compatible with the migrated schema;
- if a migration corrupts data or applies an irreversible change, treat point-in-time restore as the
  recovery path;
- restore drills must include runtime secrets needed to operate encrypted server-owned fields such
  as TOTP seed ciphertext;
- after database restore, clients may hold newer sync checkpoints than the restored server state, so
  the app must surface restore or fork conflicts rather than silently accepting stale state;
- do not restore over the live cluster during drills.

The default rollback path should be application rollback within a backward-compatible schema window.
The default data recovery path should be point-in-time restore into an isolated target, followed by
operator review before any production cutover.

## Decision

For the password-vault MVP:

- keep single-instance PostgreSQL as temporary, non-real-data infrastructure only;
- do not share another product database;
- use a product-specific CloudNativePG-style three-instance HA direction for production-like use;
- prefer synchronous replication with required durability for real user data;
- block real secrets until backup, restore, and failover gates pass;
- keep application schema migrations as a first-class release process;
- use controlled migration jobs and expand/contract policy;
- keep startup migrations disabled for real users.

## Validation Commands

Docs-only validation for this brief:

```bash
awk '/[ \t]$/ { print FNR ": trailing whitespace"; bad=1 } END { exit bad }' docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md
scan_pattern='([0-9]{1,3}[.]){3}[0-9]{1,3}'
scan_pattern="${scan_pattern}|KUBE""CONFIG|BEGIN .*PRIV""ATE|--from-""literal|:""//[^<][^[:space:]]+:[^@[:space:]]+@"
rg -n "$scan_pattern" docs/decision-briefs/2026-06-07-postgresql-ha-and-migrations.md
```

Expected result: `awk` exits cleanly, and the `rg` command prints no matches.

## Related Documents

- `docs/decision-briefs/2026-06-07-postgresql-ha-backup.md`
- `docs/adr/0004-kubernetes-data-platform-direction.md`
- `docs/runbooks/release-and-rollout.md`
- `deploy/helm/password-vault/README.md`
- `docs/development.md`

## Sources

- CloudNativePG 1.29 documentation:
  <https://cloudnative-pg.io/docs/1.29/>
- CloudNativePG architecture:
  <https://cloudnative-pg.io/docs/1.29/architecture/>
- CloudNativePG replication:
  <https://cloudnative-pg.io/docs/1.29/replication/>
- CloudNativePG backup:
  <https://cloudnative-pg.io/docs/1.29/backup/>
- CloudNativePG recovery:
  <https://cloudnative-pg.io/docs/1.29/recovery/>
- PostgreSQL Versioning Policy:
  <https://www.postgresql.org/support/versioning/>
