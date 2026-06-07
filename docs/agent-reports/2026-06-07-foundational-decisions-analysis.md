# Session Report: Foundational Decisions Analysis

## Goal

Clarify the initial architecture discussion for `password-vault` before product code starts.

## Active Context

- Active repository: `password-vault`
- Supporting context: public GitHub Project state and current draft PR
- Out of scope: `infrastructure-home` changes, Kubernetes changes, deployment, product code

## Work Completed

- Explained and documented the difference between login, session, and vault unlock.
- Reconfirmed derived-auth-key flow as the MVP authentication candidate.
- Reconfirmed account secret key / two-secret key derivation as the recommended MVP baseline, with
  UX, recovery, and new-device behavior still requiring a dedicated ADR before code.
- Kept OPAQUE as a long-term candidate, not the immediate MVP default.
- Reconfirmed Argon2id/WASM as the browser KDF target and PBKDF2 only as an explicitly approved
  prototype/degraded mode, not a silent fallback.
- Reconfirmed HKDF domain separation after one expensive password KDF.
- Reconfirmed AES-256-GCM as the browser MVP AEAD candidate, with per-revision content keys as the
  recommended nonce-risk reduction path.
- Reconfirmed TOTP as login MFA only, with seed custody as a server-owned secret-management
  decision.
- Added pre-login KDF metadata and account enumeration as a design blocker.
- Added server-side slow-hash denial-of-service as a design blocker.
- Clarified that MVP is web-client first but must be multi-device in protocol and data model.
- Recommended CloudNativePG quorum synchronous replication with `dataDurability: required` as the
  initial real-data recommendation.
- Clarified GitHub Project views and GitHub Flow usage.
- Added API-first as an explicit product architecture requirement.
- Added single-writer agent coordination rules to avoid subagent/Claude/Codex documentation
  collisions.
- Added a canonical API contract draft for the initial `/v1` surface.
- Created GitHub issue #9 for multi-device client and browser-extension roadmap.
- Added issue #9 to the public GitHub Project.
- Moved closed issue #6 to `Done` in the public GitHub Project.
- Added decision briefs for auth/crypto, clients, GitHub workflow, and PostgreSQL HA/backup.
- Evaluated Claude Code's independent review and recorded accepted/rejected findings.

## Files Changed

- `AGENTS.md`
- `README.md`
- `docs/architecture.md`
- `docs/api-contract.md`
- `docs/product-brief.md`
- `docs/foundational-decisions.md`
- `docs/research/github-control-plane.md`
- `docs/adr/0004-kubernetes-data-platform-direction.md`
- `docs/data-model.md`
- `docs/feature-map.md`
- `docs/decision-briefs/README.md`
- `docs/decision-briefs/2026-06-07-auth-crypto-mvp.md`
- `docs/decision-briefs/2026-06-07-client-roadmap.md`
- `docs/decision-briefs/2026-06-07-github-workflow.md`
- `docs/decision-briefs/2026-06-07-postgresql-ha-backup.md`
- `docs/agent-reports/2026-06-07-foundational-decisions-analysis.md`

## Commands Run

- `rg -n "password-vault|agent-platform|CloudNativePG|OPAQUE|Argon2id" <redacted-path>/.codex/memories/MEMORY.md || true`
- `git status --short --branch`
- `gh pr view --json number,title,state,isDraft,headRefName,baseRefName,url,statusCheckRollup`
- `gh project list --owner ded-isshin --format json`
- `rg --files docs .github | sort`
- `sed -n ...` for existing docs review
- `gh issue list --state open --json number,title,labels,milestone,url --limit 20`
- `gh issue create --title "[ADR]: Multi-device client and browser extension roadmap" ...`
- `gh project item-add 2 --owner ded-isshin --url https://github.com/ded-isshin/password-vault/issues/9`
- `gh project item-list 2 --owner ded-isshin --format json --limit 20`
- `gh project field-list 2 --owner ded-isshin --format json`
- `claude -p --permission-mode plan --tools Read --no-session-persistence --model opus --effort high ...`
- `gh project item-edit ... --single-select-option-id 98236657`
- `git diff --check`
- `find docs -name '*.md' -type f -empty -print`
- `python3 - <<'PY' ... yaml.safe_load ...`
- `bash -n .github/workflows/docs.yml`
- `bash -n .github/workflows/security.yml`
- `grep -RInE ...`
- `test -f README.md && test -f AGENTS.md && ...`

## Research Consulted

- RFC 9807 OPAQUE
- RFC 9106 Argon2
- W3C WebCrypto
- RFC 6238 TOTP
- OWASP Password Storage Cheat Sheet
- OWASP Cryptographic Storage Cheat Sheet
- OWASP Key Management Cheat Sheet
- CloudNativePG replication, backup, recovery, and scheduling docs
- GitHub Flow docs
- GitHub Project views docs
- Pro Git distributed workflows

## Claude Code Used?

Yes. Claude Code was invoked as an independent architecture/security reviewer for auth/login,
browser crypto, CloudNativePG replication, multi-device design, and GitHub workflow.

Accepted findings:

- Auth code must wait for a precise implementation spec, not only ADR 0003.
- Pre-login salt and KDF-parameter delivery needs explicit user-enumeration and offline-attack
  analysis.
- Argon2id parameters, PBKDF2 fallback thresholds, TOTP seed custody, rate limits, and session
  lifetimes are blockers before auth/MFA implementation.
- PBKDF2 must not become a silent production fallback.
- Per-revision content keys are preferred to reduce AES-GCM nonce-budget risk.
- CloudNativePG with three instances, anti-affinity, one synchronous standby, and
  `dataDurability: required` is the right initial real-data recommendation.
- Restore testing remains a hard gate before accepting real user secrets.

Rejected or qualified findings:

- "MVP equals one device" is rejected. The first client is browser-only, but the protocol and data
  model must be multi-device-capable from day one.
- "SECURITY.md/LICENSE/CONTRIBUTING/CODEOWNERS are missing" is rejected; these files exist in the
  repository. Claude Code was asked to focus on selected docs and did not inspect the full root.
- "Server-side slow hash may be unnecessary for a client-derived auth secret" is not accepted yet.
  The current conservative direction keeps slow server-side hashing until the auth protocol spec
  proves a cheaper verifier is safe.

## Validation

Tested:

- `git diff --check` passed.
- No empty Markdown files were found under `docs/`.
- `.github/**/*.yml` parsed successfully with PyYAML.
- `bash -n` passed for workflow files.
- Required docs from the `docs` workflow exist.
- Local public-safety grep found no matching secret/private-key patterns.

Not tested:

- GitHub Actions after push.
- Product code, because no product code exists.
- Kubernetes, database, Argo CD, and infrastructure changes.

## Risks

- Auth protocol is still not final.
- Browser Argon2id requires WASM dependency review.
- AES-GCM nonce and rekey policy is still a hard blocker.
- TOTP seed custody is not final.
- Backup target is not selected.
- GitHub repository ruleset is not yet configured.

## Open Questions

- Which Argon2id browser dependency, if any, is acceptable?
- How should account secret key UX, recovery, emergency-kit, and new-device onboarding work?
- Should OPAQUE be delayed until after MVP or implemented earlier if libraries look strong enough?
- Which object storage target will be used for backups?
- Should Vault/OpenBao be adopted for app/TOTP secrets before or after MVP?
- Which GitHub Project views should be created in the UI/API first?

## Next Steps

1. Finish PR #8 review and merge after human approval.
2. Write threat model v1.
3. Write auth/login protocol ADR with exact message shapes.
4. Write API contract draft for `/v1` registration, auth, TOTP, devices, vault sync, and item
   revisions.
5. Write crypto v1 spec.
6. Write PostgreSQL HA/backup/restore ADR.

## Approval Needed

- Merging PR #8.
- Changing GitHub repository rulesets/settings.
- Any infrastructure or Kubernetes changes.
