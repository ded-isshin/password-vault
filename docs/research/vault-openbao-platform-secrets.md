# Research Note: Vault/OpenBao Platform Secrets Fit

Status: bootstrap research note.

## Why This Matters

Vault/OpenBao is attractive because it is designed for secrets, dynamic credentials, encryption
services, leasing, audit, and Kubernetes integrations. For `password-vault`, it can help platform
operations, but it can also destroy the zero-knowledge boundary if used in the wrong place.

## Official Documentation Checked

- HashiCorp Vault overview.
- Vault Transit secrets engine.
- Vault Secrets Operator.
- Vault database secrets engine.
- Vault on Kubernetes deployment documentation.
- OpenBao Kubernetes secrets engine API documentation.
- Kubernetes Secrets documentation.

## Recommended Position

Vault/OpenBao should be treated as a platform secret manager, not as the password-manager product
core.

Good uses:

- application runtime secrets;
- PostgreSQL credentials and future rotation;
- object storage credentials for backups;
- TLS/PKI material where appropriate;
- server-owned encryption keys for TOTP seed custody;
- future dynamic database credentials;
- audit of platform-secret access.

Bad uses:

- storing user vault item plaintext;
- storing user vault keys;
- decrypting user vault item payloads with Transit;
- making the backend or platform able to recover user vault contents;
- exposing a home-cluster Vault endpoint to public GitHub Actions just to deploy.

## Kubernetes Integration Direction

For MVP, use the existing infrastructure runtime-secret path and document secret names only with
placeholders in the public product repository.

Post-MVP, evaluate:

- Vault/OpenBao plus Vault Secrets Operator;
- External Secrets Operator with Vault/OpenBao or another provider;
- SOPS;
- Sealed Secrets.

The chosen path should be a platform ADR in the infrastructure repository because it affects the
cluster, runtime secrets, backup credentials, unseal/key custody, RBAC, audit, and recovery.

## TOTP Seed Custody

TOTP seeds are server-owned authentication secrets because the server must verify codes. Encrypting
them at rest with a platform key is compatible with the product model.

This is different from user vault item payloads. The platform must never hold keys that can decrypt
user vault items.

## GitHub Actions And Vault

GitHub officially documents OIDC authentication to Vault. That may be useful later for cloud or
platform workflows. For this home-cluster product, do not open Vault to the public internet or add a
public self-hosted runner just so GitHub Actions can fetch secrets.

Product CI should build and test on GitHub-hosted runners. Deployment should happen through GitOps
PRs and Argo CD.

## Risks

- Vault/OpenBao becomes another critical stateful system with its own HA, backup, unseal, and audit
  requirements.
- Transit can accidentally become a decrypt oracle for user data.
- Kubernetes Secrets synchronized from Vault are still Kubernetes Secrets and must be protected by
  RBAC, etcd encryption where available, and least privilege.
- Dynamic credentials add operational complexity before the product needs them.
- License and project-governance differences between Vault and OpenBao need a separate platform
  decision.

## Open Questions

- Vault or OpenBao?
- Vault Secrets Operator or External Secrets Operator?
- Where are unseal keys or recovery keys stored?
- What off-host backup target protects Vault/OpenBao state if it is deployed?
- Should TOTP seed encryption use application-level AEAD or platform Transit?
- What is the first secret-management step that improves safety without overcomplicating MVP?

## Sources

- https://docs.hashicorp.com/vault/docs/about-vault/how-vault-works
- https://developer.hashicorp.com/vault/docs/secrets/transit
- https://developer.hashicorp.com/vault/docs/deploy/kubernetes/vso
- https://developer.hashicorp.com/vault/docs/secrets/databases
- https://developer.hashicorp.com/vault/docs/deploy/kubernetes
- https://openbao.org/api-docs/secret/kubernetes/
- https://kubernetes.io/docs/concepts/configuration/secret/
- https://docs.github.com/en/actions/how-tos/secure-your-work/security-harden-deployments/oidc-in-hashicorp-vault
