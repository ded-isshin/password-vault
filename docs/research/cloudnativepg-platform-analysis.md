# Research Note: CloudNativePG Platform Fit

Status: bootstrap research note.

## Why This Matters

`password-vault` will store encrypted vault data and account state in PostgreSQL. Losing recent
writes, losing the database, or deploying a false-HA database would directly harm users. The product
also needs to fit the existing Kubernetes/GitOps operating model.

## Official Documentation Checked

- CloudNativePG 1.29 Architecture
- CloudNativePG 1.29 Replication
- CloudNativePG 1.29 Scheduling
- CloudNativePG 1.29 Backup
- CloudNativePG 1.29 Recovery
- Kubernetes Persistent Volumes
- Kubernetes Ingress
- Argo CD Application Specification and Automated Sync
- Vault Secrets Operator

## Current Behavior Relevant To Us

CloudNativePG relies on PostgreSQL application-level replication with WAL streaming and WAL archive
fallback. The operator exposes services such as the read-write service that follows the current
primary after failover.

CloudNativePG recommends shared-nothing PostgreSQL placement: instances on different worker nodes,
not sharing storage, preferably using local volumes attached to the nodes where the instances run.

CloudNativePG configures pod anti-affinity by default as preferred, and can be configured to require
anti-affinity. For a three-worker deployment, required anti-affinity is a better target if resources
are sufficient.

CloudNativePG supports synchronous replication. In the 1.29 documentation, quorum synchronous
replication is configured through `spec.postgresql.synchronous.method: any` and `number: 1` for a
three-instance cluster. Strict data durability can pause writes if the required standby is
unavailable, which is the expected PostgreSQL tradeoff.

CloudNativePG backup/recovery is moving toward CNPG-I plugins. The Barman Cloud Plugin path is the
new recommended object-store integration. WAL archiving plus physical base backups are the important
business-continuity path; WAL archive alone is not enough.

Kubernetes local persistent volumes require node affinity. This means local storage is tied to a
node, and a pod cannot simply restart on a different node with the same local data unless the data is
migrated outside Kubernetes.

## Recommended Deployment ADR Outline

Title: Kubernetes Data Platform Direction

Decision:

- use product repo for app source/CI/chart and infrastructure repo for environment-specific GitOps;
- use Argo CD as the reconciler;
- use CloudNativePG with three PostgreSQL instances;
- require pod distribution across workers where resources allow;
- use quorum synchronous replication with one synchronous standby for real user data;
- configure WAL archiving and physical base backups to object storage before real user secrets;
- test restore before production use;
- keep Vault/OpenBao as platform secret management only.

Consequences:

- stronger durability but possible write pauses in strict durability mode;
- local-path storage remains acceptable only because PostgreSQL replicates at the application layer;
- object storage becomes mandatory operational dependency;
- first public deployment should not accept real user secrets until restore is proven.

## Recommended Defaults For First Production-Like Deployment

- API/web replicas: 2 or 3.
- PostgreSQL instances: 3.
- PostgreSQL connection target: CloudNativePG read-write service.
- Replication: synchronous quorum, one standby, `required` versus `preferred` to be decided by
  failure testing.
- Storage: local-path per instance, no shared PVC.
- Anti-affinity: required if the cluster has enough worker capacity; otherwise preferred plus
  explicit risk acceptance.
- Backup: Barman Cloud Plugin/CNPG-I object store path where supported.
- Restore: separate namespace restore drill before public real-data use.
- Ingress: Kubernetes ingress or current cluster ingress pattern behind edge reverse proxy.
- Secrets: runtime Kubernetes Secrets from existing infrastructure path; evaluate Vault/OpenBao
  separately.

## Risks

- Synchronous replication can reduce write availability during failures.
- Asynchronous replication can lose newly saved vault changes on primary failure.
- Local-path storage does not survive full physical host loss.
- A single home physical host is not a multi-AZ disaster recovery environment.
- Backup object storage is not selected yet.
- Restore may be slower than expected and must be measured.
- Public internet exposure before auth/rate-limit hardening increases attack surface.
- Vault/OpenBao adds a critical stateful system if adopted too early.

## Open Questions

- What object storage target should be used for backups?
- What exact CloudNativePG/operator version exists or will be installed?
- Does the current storage class support snapshots, or should we rely only on object-store backups?
- Are there enough worker resources to make PostgreSQL anti-affinity required?
- Should PostgreSQL nodes be dedicated/tainted later?
- What edge route and TLS model will be used for the public endpoint?
- Which platform secret-management path is preferred for runtime app secrets?

## Claude Code Usage

Purpose: independent platform architecture review.

Prompt/task given: analyze CloudNativePG HA, sync vs async replication, local-path storage
limitations, backup/WAL archiving, restore testing, ingress/public routing, and Vault/OpenBao as a
platform layer only.

Summary of output: Claude Code agreed that CloudNativePG plus PostgreSQL replication is the right
direction, local-path means HA must come from PostgreSQL replicas rather than storage mobility, and
Vault/OpenBao must stay out of the user-vault decrypt path. It recommended quorum synchronous
replication and emphasized restore drills.

Accepted suggestions: local-path warning, off-cluster object storage requirement, restore testing,
no public PostgreSQL exposure, Vault/OpenBao as platform-only.

Rejected or qualified suggestions: `dataDurability: preferred` is not accepted as the default
without testing. For password-manager data, `required` may be more appropriate if latency and
failure behavior are acceptable.

## Sources

- https://cloudnative-pg.io/docs/1.29/architecture/
- https://cloudnative-pg.io/docs/1.29/replication/
- https://cloudnative-pg.io/docs/1.29/scheduling/
- https://cloudnative-pg.io/docs/1.29/backup/
- https://cloudnative-pg.io/docs/1.29/recovery/
- https://kubernetes.io/docs/concepts/storage/persistent-volumes/
- https://kubernetes.io/docs/concepts/services-networking/ingress/
- https://argo-cd.readthedocs.io/en/release-3.0/user-guide/application-specification/
- https://argo-cd.readthedocs.io/en/stable/user-guide/auto_sync/
- https://developer.hashicorp.com/vault/docs/deploy/kubernetes/vso
