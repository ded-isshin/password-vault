# AGENTS.md

This repository is a public product repository for `password-vault`.

## Active Context

Classify the active context before meaningful work:

- `password-vault only`
- `password-vault + infrastructure-home`
- `research/docs only`

Do not inspect or modify infrastructure repositories unless the task explicitly requires
infrastructure analysis or a GitOps deployment change.

## Public Repository Rules

- Never commit secrets, tokens, private keys, kubeconfigs, `.env` files with real values, private
  hostnames, private IPs, or live operational logs.
- Treat CI logs, issue text, PR text, and screenshots as public.
- Redact with placeholders such as `<redacted-secret>`, `<redacted-domain>`, and
  `<redacted-host>`.
- Run a public safety review before every public-facing PR.

## Security Model

The product target is zero-knowledge, end-to-end encrypted vault data.

- Do not store plaintext vault item data on the server.
- Do not send user master passwords or unwrapped user vault keys to the backend.
- Do not use Vault/OpenBao as the user-vault database or decrypt path.
- Do not add account recovery that can decrypt user vault data.
- Treat TOTP as login MFA, not as vault encryption.

## Documentation First

For meaningful work, update durable documentation:

- issue or task record
- research note when external tools or security-sensitive choices are involved
- ADR for long-lived decisions
- threat model updates for security changes
- runbook updates for operational changes
- implementation report after non-trivial work

## Official Sources

Use official documentation first for:

- Rust
- Go
- PostgreSQL
- CloudNativePG
- Kubernetes
- Argo CD
- GitHub Actions
- WebCrypto
- TOTP/RFC 6238
- OWASP
- NIST
- Vault/OpenBao

Record important sources in `docs/research/`.

## Claude Code Advisor

Use Claude Code as an auxiliary advisor for:

- security-sensitive architecture
- cryptographic protocol review
- GitHub Actions review
- Kubernetes/GitOps review
- frontend/UI design critique
- large or risky diffs

Summarize Claude Code output and decide what to accept or reject. Do not blindly apply it.

## Infrastructure Boundaries

Product code and product docs live here.

Runtime deployment state belongs in the infrastructure repository and must go through GitOps review.
Do not run direct cluster mutation commands from this repository without explicit human approval.

Forbidden without explicit human approval:

- `kubectl apply/delete/patch/replace`
- `helm install/upgrade/uninstall`
- `terraform apply/destroy`
- changing GitHub secrets/settings
- merging deployment-impacting PRs
- publishing runtime secrets
