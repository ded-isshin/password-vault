# Session Report: Threat Model V1

## Goal

Create the first complete threat model for the public web MVP.

## Active Context

- Active repository: `password-vault`
- Issue: #1
- Branch: `docs/1-threat-model-v1`
- Out of scope: infrastructure changes, product code, deployment, GitHub settings

## Work Completed

- Replaced the bootstrap threat model with a v1 draft.
- Added explicit scope, security goals, assets, actors, trust boundaries, data flows, threat matrix,
  residual risks, test evidence requirements, open decisions, and sources.
- Added live-backend compromise, CSRF, stale revision/rollback, key-wrap substitution, crypto
  downgrade, GitHub/GHCR/GitOps, Kubernetes/RBAC/NetworkPolicy, backup/restore, and observability
  risks.
- Added key hierarchy and custody section.
- Linked open risks to issues #2, #3, #4, #5, #7, and #9.

## Agent Coordination

Codex was the only writer for `docs/threat-model.md`.

Subagents were report-only:

- Curie: auth/crypto/MFA/device threat review.
- Gauss: platform/CI/data threat review.

Maximum runtime was set to 2 hours. Both subagents completed within the runtime. No subagent edited
files.

## Claude Code Used?

Yes.

Purpose: independent security architecture review of the threat model draft.

Summary of output:

- The draft had a good zero-knowledge boundary and honest browser-delivered crypto residual risk.
- Missing blockers included stale revision rollback, CSRF, live compromised backend auth-channel
  ambiguity, key hierarchy/custody, and stronger supply-chain/platform backup coverage.

Accepted suggestions:

- Added stale/rolled-back revision threats and tests.
- Added CSRF and session fixation threats and tests.
- Added live backend replay risk as an open blocker for #2.
- Added key hierarchy and AAD direction.
- Added supply-chain, GHCR, GitOps, Kubernetes, backup, and observability risks.
- Clarified CSP/SRI as partial controls, not complete same-origin protection.

Rejected suggestions:

- None.

Deferred suggestions:

- Full OpenAPI contract and exact endpoint schemas.
- Exact PAKE/OPAQUE versus derived-auth message shape.
- Full backup and Kubernetes runbooks.
- Strong device enrollment and browser-extension threat model.

## Commands Run

- `git switch -c docs/1-threat-model-v1`
- `gh issue view 1 --json ...`
- `sed -n ... docs/threat-model.md`
- `sed -n ... docs/api-contract.md docs/foundational-decisions.md`
- `claude -p --permission-mode plan --tools "" --no-session-persistence --model opus --effort high ...`
- `rg ... docs/threat-model.md`
- `git diff --check`

## Research Consulted

- OWASP Threat Modeling Cheat Sheet
- OWASP Application Security Verification Standard
- NIST SP 800-63B
- OWASP Secrets Management Cheat Sheet
- RFC 6238 TOTP
- RFC 9106 Argon2
- W3C WebCrypto

## Validation

Pending final local validation and GitHub checks.

## Risks

- Threat model is still documentation, not implementation.
- Exact auth, crypto, TOTP, API, backup, and GitHub ruleset decisions remain open.
- Browser-delivered JavaScript risk is accepted only as a documented MVP residual risk, not solved.

## Approval Needed

- No infrastructure or GitHub settings approval needed for this docs PR.
- Future merge can proceed when checks pass.

