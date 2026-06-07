# password-vault

Status: product bootstrap draft. No production code exists yet.

`password-vault` is a planned Kubernetes-native, zero-knowledge password manager.
The first milestone is a personal vault MVP with registration, login, TOTP MFA,
client-side encryption, encrypted item storage, and GitOps-compatible deployment.

This repository is public from the start. Treat all issues, pull requests,
documentation, logs, and CI output as public.

## Initial Direction

- Product type: password manager for personal use first, with a path to organizations.
- Security model: zero-knowledge, end-to-end encrypted vault data.
- Backend direction: Rust, Axum, SQLx, PostgreSQL.
- Frontend direction: TypeScript, React, Vite, WebCrypto.
- Database direction: PostgreSQL, with CloudNativePG for production-like Kubernetes.
- Deployment direction: GitHub Actions, GHCR, Helm, GitOps PR, Argo CD.
- MFA direction: TOTP first, WebAuthn/passkeys later.
- Secret-management direction: Vault/OpenBao may help platform secrets later, but it is not the
  user-vault core.
- Public routing direction: edge reverse proxy to Kubernetes ingress/service; exact host, port,
  TLS, and network details belong in the infrastructure repository.
- Current blocker: login/key-derivation and cryptographic design are not finalized.

## MVP Scope

In scope:

- personal user account
- login
- TOTP enrollment and verification
- recovery codes
- personal vault
- encrypted vault items
- item revisions
- basic audit events without secret values
- CI and documentation

Out of scope for the first MVP:

- organizations
- shared vaults
- browser extension
- mobile app
- desktop app
- KeePass/KDBX import
- plugin marketplace
- billing
- admin recovery that can decrypt user vault data

## Repository Safety

Do not commit:

- passwords or vault contents
- TOTP seeds
- recovery codes
- private keys
- tokens
- kubeconfigs
- `.env` files with real values
- private hostnames, IPs, or home-network details
- live infrastructure logs

Use placeholders such as `<redacted-secret>`, `<redacted-domain>`, and
`<redacted-host>` when documentation needs an example.

## Start Here

- [AGENTS.md](AGENTS.md)
- [docs/product-brief.md](docs/product-brief.md)
- [docs/whitepaper.md](docs/whitepaper.md)
- [docs/foundational-decisions.md](docs/foundational-decisions.md)
- [docs/decision-briefs/README.md](docs/decision-briefs/README.md)
- [docs/feature-map.md](docs/feature-map.md)
- [docs/architecture.md](docs/architecture.md)
- [docs/development.md](docs/development.md)
- [docs/api-contract.md](docs/api-contract.md)
- [docs/diagrams.md](docs/diagrams.md)
- [docs/data-model.md](docs/data-model.md)
- [docs/sync-protocol.md](docs/sync-protocol.md)
- [docs/auth-mfa-lifecycle.md](docs/auth-mfa-lifecycle.md)
- [docs/lock-unlock-state.md](docs/lock-unlock-state.md)
- [docs/threat-model.md](docs/threat-model.md)
- [docs/security/auth-protocol-v1.md](docs/security/auth-protocol-v1.md)
- [docs/security/crypto-design-draft.md](docs/security/crypto-design-draft.md)
- [docs/security/revision-freshness.md](docs/security/revision-freshness.md)
- [docs/adr/0001-initial-product-direction.md](docs/adr/0001-initial-product-direction.md)
- [docs/adr/0002-backend-stack-rust.md](docs/adr/0002-backend-stack-rust.md)
- [docs/adr/0003-auth-and-crypto-direction.md](docs/adr/0003-auth-and-crypto-direction.md)
- [docs/adr/0004-kubernetes-data-platform-direction.md](docs/adr/0004-kubernetes-data-platform-direction.md)
- [docs/research/initial-stack-analysis.md](docs/research/initial-stack-analysis.md)
- [docs/research/auth-crypto-v1-analysis.md](docs/research/auth-crypto-v1-analysis.md)
- [docs/research/vault-openbao-platform-secrets.md](docs/research/vault-openbao-platform-secrets.md)
- [docs/research/cloudnativepg-platform-analysis.md](docs/research/cloudnativepg-platform-analysis.md)
- [docs/research/github-control-plane.md](docs/research/github-control-plane.md)
- [docs/research/source-baseline-2026-06-06.md](docs/research/source-baseline-2026-06-06.md)
- [docs/research/product-architecture-ux-subagent-2026-06-06.md](docs/research/product-architecture-ux-subagent-2026-06-06.md)
