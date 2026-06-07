# ADR 0004: Kubernetes Data Platform Direction

Status: proposed.

## Context

`password-vault` must be Kubernetes-native and GitOps-compatible. The target home platform has a
multi-worker Kubernetes cluster and node-local persistent storage. The product should tolerate a
single worker failure for production-like operation, while recognizing that physical host failure is
outside the current cluster's availability boundary.

## Options Considered

### Single PostgreSQL Pod

Pros:

- simple.

Cons:

- not acceptable for real vault data;
- no single-worker failure tolerance;
- weak backup and recovery posture unless heavily extended.

### PostgreSQL Operator With Replicas

Use CloudNativePG or another mature PostgreSQL operator.

Pros:

- Kubernetes-native lifecycle.
- primary/replica topology.
- failover support.
- backup/WAL integration.
- works with GitOps manifests.

Cons:

- more platform complexity.
- requires backup target and restore runbook.
- local-path storage still means data is node-local.

### Distributed Storage First

Introduce a distributed storage layer before product deployment.

Pros:

- may improve volume mobility and storage resilience.

Cons:

- larger infrastructure project;
- not necessary for initial product architecture;
- must not be rushed for a password manager.

## Proposed Direction

Use CloudNativePG for production-like PostgreSQL design, with three instances spread across worker
nodes where possible.

Use PostgreSQL replication for data availability. Do not rely on local PV mobility.

## Replication Direction

Evaluate quorum synchronous replication with one synchronous standby for vault data durability.
CloudNativePG 1.29 documents this through `spec.postgresql.synchronous.method: any` and
`number: 1`.

MVP recommendation:

- `dataDurability: required` favors acknowledged-write durability and may pause writes when the
  required standby is unavailable.
- `dataDurability: preferred` favors write availability and self-healing during degraded states, but
  may temporarily accept asynchronous behavior.
- asynchronous replication alone is acceptable only for development or throwaway data unless there is
  explicit risk acceptance.

The default product recommendation is synchronous replication with one synchronous standby and
`dataDurability: required` for real user data. This favors acknowledged-write durability over write
availability during degraded states.

`dataDurability: preferred` can be considered after failure-mode testing if write availability
during degraded states is more important than strict RPO=0 for acknowledged writes. That tradeoff
requires explicit risk acceptance.

Asynchronous replication is acceptable for development or throwaway data, not for public real-data
use.

Failure-mode testing in the target cluster is still required before production-like use. The test
must prove how the application behaves during one worker failure, standby loss, primary failover,
and backup restore.

## Backup Direction

No real vault data should be stored until off-node backups and restore testing exist.

The expected direction is S3-compatible object storage or another object-store target supported by
the CloudNativePG backup path. If the first public deployment happens before this exists, it must be
treated as a public demo only and must clearly forbid real secrets.

CloudNativePG 1.29 documents WAL archiving and physical base backups as the core backup building
blocks. Native backup/recovery is being phased toward CNPG-I plugins; the Barman Cloud Plugin should
be the preferred new-design path if it matches the installed operator version. A backup target must
be selected before production-like deployment.

WAL archive alone is not enough. Restore requires a physical base backup plus WAL. Restore must be
tested into a separate namespace or cluster before real user secrets are accepted.

## Local-Path Storage Direction

Local-path storage is node-local. It does not provide storage replication, volume mobility, or a
cross-node snapshot story by itself.

Consequences:

- PostgreSQL pods must be spread across workers.
- Single-worker tolerance comes from PostgreSQL replication and CloudNativePG failover.
- A failed worker's local PV should be treated as unavailable until that worker returns or the
  instance is rebuilt from another source.
- Full physical host failure is outside the single-worker-failure target and requires off-host
  backups.

## GitOps Direction

Product repository owns:

- source code;
- product docs;
- tests;
- Dockerfile;
- chart or deployment template.

Infrastructure repository owns:

- Argo CD Application;
- namespace;
- production values;
- ingress/routing;
- CloudNativePG cluster resources if chosen as shared platform state;
- runtime secret references.

## Vault/OpenBao Direction

Vault/OpenBao may be evaluated for platform runtime secrets, dynamic database credentials, PKI, or
server-owned TOTP seed encryption. It must not decrypt user vault item payloads.

## Consequences

- A backup target is a hard blocker for real data.
- Restore test runbook is required before production-like use.
- Public deployment routing requires a separate infrastructure ADR and explicit human approval.
- PostgreSQL is never exposed publicly.
- Application ingress/routing details stay in the infrastructure repository.
- Direct `kubectl apply` from product work is out of scope.

## Sources

- https://cloudnative-pg.io/docs/1.29/architecture/
- https://cloudnative-pg.io/docs/1.27/replication/
- https://cloudnative-pg.io/docs/1.29/scheduling/
- https://cloudnative-pg.io/docs/1.29/backup/
- https://cloudnative-pg.io/docs/1.29/recovery/
- https://cloudnative-pg.io/plugin-barman-cloud/docs/intro/
- https://kubernetes.io/docs/concepts/storage/persistent-volumes/
- https://kubernetes.io/docs/concepts/services-networking/ingress/
- https://argo-cd.readthedocs.io/en/release-3.0/user-guide/application-specification/
- https://argo-cd.readthedocs.io/en/stable/user-guide/auto_sync/
