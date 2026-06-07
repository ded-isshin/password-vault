# Decision Brief: PostgreSQL HA And Backup

Status: draft.

## Question

Which PostgreSQL HA and backup direction should the Kubernetes-native MVP use with CloudNativePG,
node-local local-path storage, and a future S3/object-storage backup target?

## Short Answer

Use CloudNativePG for production-like PostgreSQL with three instances spread across worker nodes.

For real user data, prefer synchronous replication with one synchronous standby and
`dataDurability: required` unless failure testing shows that write pauses are unacceptable. Use
asynchronous replication only for development or explicitly accepted throwaway environments.

No real user secrets should be accepted until object-store WAL archiving, scheduled base backups,
and a restore drill are working.

## Environment Constraint

The current platform uses node-local storage for persistent volumes. Local-path storage is not
distributed storage. It does not make a PVC portable across worker nodes.

Therefore, single-worker failure tolerance must come from PostgreSQL replication and CloudNativePG
failover, not from storage mobility.

## Recommended Modes

| Environment | PostgreSQL mode | Backup mode | Accept real data? |
| --- | --- | --- | --- |
| Local dev | local Postgres container or single instance | disposable | No |
| Kubernetes dev | CloudNativePG 1 instance or 3 async instances | optional | No |
| Prod-like test | CloudNativePG 3 instances | WAL + base backups to object storage | No, until restore passes |
| Public real-data | CloudNativePG 3 instances, sync replication | WAL + scheduled base backups + restore runbook | Yes |

## Sync vs Async

Asynchronous replication keeps writes available when replicas are behind or unavailable, but a
primary failure can lose acknowledged recent writes.

Synchronous replication makes commits wait until WAL is replicated to the required standby count.
With CloudNativePG `dataDurability: required`, this targets zero data loss for acknowledged writes
but can pause writes if the required standby is unavailable.

For a password manager, losing a newly saved password is usually worse than temporarily pausing
writes. The MVP production-like recommendation is therefore:

```yaml
spec:
  instances: 3
  postgresql:
    synchronous:
      method: any
      number: 1
      dataDurability: required
```

`dataDurability: preferred` can be considered after testing if availability during degraded states
is more important than strict RPO=0 for acknowledged writes. That would need explicit risk
acceptance.

## Backup Direction

Use object storage for backups. The likely future target is S3-compatible storage.

The backup design must include both:

- continuous WAL archiving;
- physical base backups.

WAL alone is not enough. Restore needs a base backup plus WAL replay. The preferred CloudNativePG
new-design path is the Barman Cloud Plugin when the installed CloudNativePG version supports it.

Each PostgreSQL cluster should have a dedicated object-store backup configuration for isolation and
operational clarity.

## Restore Requirement

Restore must be tested before real data:

1. create a test item;
2. confirm WAL archive and scheduled base backup exist;
3. restore into a separate namespace or separate cluster object;
4. verify data is present;
5. verify app can connect to restored database in a controlled test;
6. document RTO/RPO observations.

Recovery must not overwrite the live cluster during the first drills.

## GitOps Boundary

Product repository owns:

- app source;
- product docs;
- tests;
- Dockerfile;
- Helm chart or app deployment template;
- non-secret default values.

Infrastructure repository owns:

- namespace;
- Argo CD Application;
- production values;
- ingress/routing;
- CloudNativePG Cluster and backup resources if they are environment-owned;
- object-store references;
- runtime secret references.

Git must not contain:

- S3 credentials;
- database passwords;
- kubeconfigs;
- private home-network details;
- plaintext TOTP seed protection keys.

## Hard Gates Before Public Real-Data Use

- backup target selected;
- CloudNativePG/operator version selected;
- WAL archiving enabled;
- scheduled physical backups enabled;
- restore drill passed;
- failover drill passed;
- local-path storage limitations documented in the runbook;
- no public PostgreSQL exposure;
- app ingress and TLS design approved;
- secrets management path approved.

## Sources

- https://cloudnative-pg.io/docs/1.29/architecture/
- https://cloudnative-pg.io/docs/1.27/replication/
- https://cloudnative-pg.io/plugin-barman-cloud/docs/concepts/
- https://cloudnative-pg.io/docs/1.29/backup/
- https://cloudnative-pg.io/docs/1.29/recovery/
- https://kubernetes.io/docs/concepts/storage/persistent-volumes/
