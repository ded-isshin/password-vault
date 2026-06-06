# Session Report: Initial Architecture Analysis

Status: draft.

## Goal

Start the baseline architecture analysis for `password-vault`, create durable documentation, and
prepare GitHub Project tracking.

## Active Context

- Active repository: `password-vault`.
- Supporting context: public official documentation and previous `agent-platform` bootstrap docs.
- Out of scope: product code, infrastructure repository changes, Kubernetes changes, deployments,
  repository settings changes.

## Work Completed

- Verified GitHub CLI has `read:project` scope.
- Confirmed `project` write scope is still needed for GitHub Project creation.
- Continued on branch `docs/architecture-stack-baseline`.
- Drafted product whitepaper.
- Drafted architecture diagrams.
- Drafted backend stack ADR.
- Drafted auth and crypto direction ADR.
- Drafted Kubernetes data platform ADR.
- Drafted source baseline research note.
- Drafted GitHub control-plane research note.
- Drafted Vault/OpenBao fit research note.
- Drafted auth and crypto v1 research note.
- Added product feature map.
- Added data model and plaintext/ciphertext boundary draft.
- Added sync protocol draft.
- Added auth and MFA lifecycle draft.
- Added lock/unlock state model.
- Captured product architecture and UX review findings.
- Captured CloudNativePG platform review findings.

## Files Created Or Changed

- `README.md`
- `docs/product-brief.md`
- `docs/architecture.md`
- `docs/threat-model.md`
- `docs/security/crypto-design-draft.md`
- `docs/whitepaper.md`
- `docs/diagrams.md`
- `docs/feature-map.md`
- `docs/data-model.md`
- `docs/sync-protocol.md`
- `docs/auth-mfa-lifecycle.md`
- `docs/lock-unlock-state.md`
- `docs/adr/0002-backend-stack-rust.md`
- `docs/adr/0003-auth-and-crypto-direction.md`
- `docs/adr/0004-kubernetes-data-platform-direction.md`
- `docs/research/source-baseline-2026-06-06.md`
- `docs/research/auth-crypto-v1-analysis.md`
- `docs/research/github-control-plane.md`
- `docs/research/vault-openbao-platform-secrets.md`
- `docs/research/cloudnativepg-platform-analysis.md`
- `docs/research/product-architecture-ux-subagent-2026-06-06.md`
- `docs/agent-reports/2026-06-06-initial-architecture-analysis.md`

## Commands Run

- `git status --short --branch`
- `git remote -v`
- `gh auth status`
- `gh project list --format json`
- `gh issue list --repo ded-isshin/password-vault --limit 20`
- `gh project create --owner @me --title "Password Vault MVP" --format json`
- `git switch -c docs/architecture-stack-baseline`
- `claude -p --permission-mode plan --tools Read --no-session-persistence --model opus --effort high ...`
- `gh repo view ded-isshin/password-vault --json nameWithOwner,visibility,url,defaultBranchRef,hasIssuesEnabled,hasProjectsEnabled,hasWikiEnabled,viewerPermission,securityPolicyUrl`
- `gh project list --owner ded-isshin --format json`
- `test -f README.md && test -f AGENTS.md && test -f SECURITY.md && test -f CONTRIBUTING.md && test -f docs/product-brief.md && test -f docs/architecture.md && test -f docs/threat-model.md`
- GitHub workflow-style public-safety grep for common secret material.
- Additional local public-safety scan for private network and token-like patterns.
- `git diff --check`
- `bash -lc 'set -euo pipefail; grep -oP "\]\(\Kdocs/[^)]*" README.md | sort -u | while read -r path; do test -f "$path" || { echo "missing $path"; exit 1; }; done'`

## Research And Docs Consulted

- RFC 9807 OPAQUE.
- RFC 9106 Argon2.
- RFC 6238 TOTP.
- W3C WebCrypto.
- OWASP cryptographic storage and key management guidance.
- 1Password Security Design.
- Bitwarden Security Whitepaper.
- Axum, SQLx, RustCrypto Argon2 docs.
- CloudNativePG architecture, backup, replication, and recovery docs.
- CloudNativePG 1.29 scheduling docs.
- Kubernetes persistent volume and ingress docs.
- GitHub Projects, rulesets, branch protection, CODEOWNERS, and Actions permission docs.
- Argo CD Application and automated sync docs.
- Vault and OpenBao documentation.

## Subagents Used?

Yes.

- Claude Code was used as an independent reviewer for auth/login/key-derivation and crypto v1.
- Claude Code was used as an independent reviewer for product architecture, UX flows, sync, and
  recovery expectations.
- Claude Code was used as an independent platform reviewer for CloudNativePG HA, local-path storage,
  backup/restore, public routing, and Vault/OpenBao platform usage.
- Sidecar research outputs from earlier planning were incorporated for Vault/OpenBao, GitHub
  workflow, and Rust vs Go stack direction.

## Claude Code Used?

Yes.

First review purpose: independent architecture/security critique for auth, key derivation, and
crypto v1.

Accepted suggestions:

- derived-auth-key as MVP candidate;
- OPAQUE migration path;
- explicit browser residual risk;
- Argon2id/WASM review blocker;
- one-pass Argon2id plus HKDF domain separation;
- server-side slow hash for client-derived auth secret;
- AES-GCM nonce budget and rekey requirement;
- stronger test requirements.

Rejected or corrected suggestions:

- Claude described OPAQUE as a draft; official sources show OPAQUE is RFC 9807.

Second review purpose: independent product architecture and UX critique.

Accepted suggestions:

- distinguish server session from vault unlock state;
- make recovery-code wording explicit because account MFA recovery is not vault recovery;
- make plaintext metadata boundary explicit;
- define revision and delta-sync protocol before code;
- postpone plugin/marketplace work while keeping payload and API versions.

Rejected suggestions: none outright. Multi-device enrollment, recovery-key design, and plugin model
remain deferred decisions.

Third review purpose: independent platform critique for CloudNativePG, local-path storage,
backup/restore, ingress, and Vault/OpenBao.

Accepted suggestions:

- local-path storage must be documented as node-local and non-portable;
- PostgreSQL HA comes from replication, not distributed local-path storage;
- object-store backup and restore drills are mandatory before real user secrets;
- public routing should expose only the application, never PostgreSQL;
- Vault/OpenBao must stay outside the user-vault decrypt path.

Rejected or qualified suggestions:

- `dataDurability: preferred` is not accepted as a default without target-cluster failure testing.
  The ADR records `required` versus `preferred` as an explicit durability/availability tradeoff.

## Validation

- Required docs check: passed.
- GitHub workflow-style public-safety grep: passed.
- Additional local public-safety pattern scan: only matched the checked-in workflow regex itself.
- `git diff --check`: passed.
- README local documentation links: passed.

## Risks

- OPAQUE implementation maturity is not yet validated.
- Browser Argon2id/WASM dependency risk is not yet resolved.
- Browser-delivered JavaScript remains an accepted residual risk.
- Backup target is unknown.
- GitHub Project write scope is not yet confirmed; current token still reports `read:project`.
- CloudNativePG `required` versus `preferred` synchronous durability mode is not selected.
- Public routing details are intentionally not documented in this public repository.
- Multi-device support is not decided.
- Account recovery codes may be misunderstood as vault recovery unless the UX is explicit.

## Open Questions

- Which auth protocol should be selected?
- Which browser KDF path should be selected?
- Which TOTP seed custody model should be selected?
- Which backup target should be selected?
- Should synchronous PostgreSQL replication be required for acknowledged writes?
- Should the MVP support multiple devices immediately?
- Should a zero-knowledge recovery key be part of MVP?
- Which metadata fields, if any, may be plaintext?

## Next Steps

1. Complete GitHub Project creation after `project` scope is confirmed.
2. Commit this documentation branch and open a draft PR.
3. Create ADR for single-device vs multi-device MVP.
4. Create ADR/spec for auth/login and key-derivation protocol.
5. Create ADR/spec for crypto v1 payload, key hierarchy, and recovery-key decision.
6. Create ADR/spec for PostgreSQL HA, backup, and restore.
