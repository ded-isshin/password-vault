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

- Verified GitHub CLI now has `read:project` scope.
- Confirmed `project` write scope is still needed for GitHub Project creation.
- Created branch `docs/architecture-stack-baseline`.
- Drafted product whitepaper.
- Drafted architecture diagrams.
- Drafted backend stack ADR.
- Drafted auth and crypto direction ADR.
- Drafted Kubernetes data platform ADR.
- Drafted source baseline research note.
- Drafted GitHub control plane research note.
- Drafted Vault/OpenBao fit research note.
- Drafted auth and crypto v1 research note.
- Ran Claude Code as an independent architecture/security reviewer for auth, key derivation, and
  crypto v1.

## Files Created Or Changed

- `docs/whitepaper.md`
- `docs/diagrams.md`
- `docs/adr/0002-backend-stack-rust.md`
- `docs/adr/0003-auth-and-crypto-direction.md`
- `docs/adr/0004-kubernetes-data-platform-direction.md`
- `docs/research/source-baseline-2026-06-06.md`
- `docs/research/auth-crypto-v1-analysis.md`
- `docs/research/github-control-plane.md`
- `docs/research/cloudnativepg-platform-analysis.md`
- `docs/research/vault-openbao-platform-secrets.md`
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

## Research And Docs Consulted

- RFC 9807 OPAQUE.
- RFC 9106 Argon2.
- RFC 6238 TOTP.
- W3C WebCrypto.
- OWASP cryptographic storage and key management guidance.
- Axum, SQLx, RustCrypto Argon2 docs.
- CloudNativePG architecture, backup, and recovery docs.
- Kubernetes storage docs.
- GitHub Projects, rulesets, branch protection, CODEOWNERS, and Actions permission docs.

## Subagents Used?

Yes.

- Claude Code was used as an independent reviewer for auth/login/key-derivation and crypto v1.
- Sidecar research outputs from earlier planning were incorporated for Vault/OpenBao, GitHub
  workflow, and Rust vs Go stack direction.

## Claude Code Used?

Yes.

Purpose: independent architecture/security critique for auth, key derivation, and crypto v1.

Prompt/task given: review a Kubernetes-native zero-knowledge password manager MVP with TOTP and
future organizations; analyze PAKE/OPAQUE vs derived-auth-key vs simpler MVP, WebCrypto vs
Argon2id/WASM, key hierarchy, AEAD, nonce/versioning, and browser-delivered crypto risk.

Summary of output: Claude recommended a derived-auth-key MVP with OPAQUE as a future auth-layer
migration, Argon2id/WASM as the preferred KDF if supply-chain controls are ready, AES-GCM for the
web MVP, explicit versioning, and additional crypto tests.

Accepted suggestions: derived-auth-key as MVP candidate, OPAQUE migration path, explicit browser
residual risk, Argon2id/WASM review blocker, stronger test requirements.

Rejected suggestions: Claude described OPAQUE as a draft; official sources show OPAQUE is RFC 9807.

## Validation

- Documentation drafted.
- Validation pending after final doc pass.

## Risks

- OPAQUE implementation maturity is not yet validated.
- Browser Argon2id/WASM dependency risk is not yet resolved.
- Browser-delivered JavaScript remains an accepted residual risk.
- Backup target is unknown.
- GitHub Project write scope is not yet confirmed.

## Open Questions

- Which auth protocol should be selected?
- Which browser KDF path should be selected?
- Which TOTP seed custody model should be selected?
- Which backup target should be selected?
- Should synchronous PostgreSQL replication be required for acknowledged writes?

## Next Steps

1. Complete GitHub Project creation after `project` scope is confirmed.
2. Wait for subagent outputs and incorporate accepted findings.
3. Run Claude Code independent review.
4. Validate documentation and public-safety checks.
5. Commit branch and open a draft PR.
