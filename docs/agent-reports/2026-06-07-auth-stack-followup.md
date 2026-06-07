# Session Report: Auth And Stack Follow-Up

Date: 2026-06-07.

Status: documentation and architecture follow-up for draft PR #8.

Superseded note: later review reaffirmed `account secret key / two-secret key derivation` as the
recommended MVP baseline, while requiring a dedicated ADR for UX, recovery, and new-device behavior
before code.

## Goal

Clarify authentication/login options, multi-device direction, PostgreSQL replication recommendation,
and GitHub Project views for `password-vault` before product implementation starts.

## Active Context

- Active repository: `password-vault`.
- Read/write scope: documentation only.
- Out of scope: product code, Kubernetes changes, Argo CD changes, Terraform, runtime deployment.

## Work Completed

- Added a dedicated research note comparing auth/login options:
  - traditional password-over-TLS;
  - derived-auth-key;
  - OPAQUE / PAKE;
  - WebAuthn / passkeys.
- Updated ADR 0003 with the current recommended MVP direction:
  - derived-auth-key for MVP;
  - account secret key / two-secret key derivation as recommended strengthening;
  - OPAQUE long-term;
  - WebAuthn/passkeys post-MVP as phishing-resistant MFA/login path;
  - traditional password-over-TLS rejected for public MVP.
- Updated architecture, whitepaper, diagrams, auth lifecycle, crypto draft, feature map, and data
  model for multi-device-capable web MVP.
- Updated PostgreSQL direction to recommend synchronous quorum replication with
  `dataDurability: required` for real user data.
- Clarified GitHub Project views as saved views over the same project items.

## Files Changed

- `docs/research/auth-login-protocol-options.md`
- `docs/research/auth-crypto-v1-analysis.md`
- `docs/research/github-control-plane.md`
- `docs/adr/0003-auth-and-crypto-direction.md`
- `docs/adr/0004-kubernetes-data-platform-direction.md`
- `docs/architecture.md`
- `docs/auth-mfa-lifecycle.md`
- `docs/data-model.md`
- `docs/diagrams.md`
- `docs/feature-map.md`
- `docs/security/crypto-design-draft.md`
- `docs/whitepaper.md`
- `docs/agent-reports/2026-06-07-auth-stack-followup.md`

## Claude Code Usage

Purpose: independent architecture/security review for auth/login options.

Prompt/task given: compare derived-auth-key, OPAQUE/PAKE, password-over-TLS, and WebAuthn/passkeys
for a zero-knowledge Kubernetes-native password manager, using standards and primary sources.

Summary of output:

- MVP should use derived-auth-key plus TOTP.
- Account secret key / two-secret key derivation is a strong low-cost improvement for MVP.
- OPAQUE is the long-term password-login target after library and browser interop review.
- WebAuthn/passkeys should be added first as phishing-resistant MFA and later considered for login.
- Password-over-TLS should not be used for the public MVP.

Accepted suggestions:

- Add account secret key as recommended MVP direction.
- Keep OPAQUE long-term rather than first blocker.
- Keep WebAuthn/passkeys post-MVP.
- Make multi-device protocol support explicit.

Rejected or deferred suggestions:

- Do not implement OPAQUE immediately.
- Do not design passkey unlock around WebAuthn PRF until support and product UX are reviewed.

## Sources Consulted

- RFC 9807 OPAQUE: https://www.rfc-editor.org/info/rfc9807/
- NIST SP 800-63B: https://pages.nist.gov/800-63-4/sp800-63b.html
- OWASP ASVS: https://owasp.org/www-project-application-security-verification-standard/
- OWASP Authentication Cheat Sheet:
  https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- OWASP MFA Cheat Sheet:
  https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html
- W3C WebAuthn Level 3: https://www.w3.org/TR/webauthn-3/
- 1Password Security Design:
  https://agilebits.github.io/security-design/key-security-features.html
- Bitwarden Security Whitepaper:
  https://bitwarden.com/help/bitwarden-security-white-paper/
- CloudNativePG replication: https://cloudnative-pg.io/docs/1.29/replication/
- CloudNativePG recovery: https://cloudnative-pg.io/docs/1.29/recovery/
- Barman Cloud Plugin: https://cloudnative-pg.io/plugin-barman-cloud/docs/intro/
- GitHub Flow: https://docs.github.com/en/get-started/using-github/github-flow
- GitHub Projects:
  https://docs.github.com/en/issues/planning-and-tracking-with-projects/learning-about-projects/about-projects
- Pro Git branching workflows:
  https://git-scm.com/book/en/v2/Git-Branching-Branching-Workflows.html

## Validation

Tested after this report was drafted:

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

- Account secret key improves database-compromise resistance but adds UX and recovery complexity.
- Browser-delivered JavaScript remains a structural residual risk for the web MVP.
- OPAQUE and WebAuthn/passkey unlock are deferred and must not be silently implied.
- Public deployment before off-node backups must be treated as demo-only and must not accept real
  user secrets.

## Open Questions

- Final acceptance of account secret key / two-secret key derivation for MVP.
- Exact Argon2id parameters and browser WASM dependency.
- TOTP seed custody: app-level encryption, Vault/OpenBao Transit, or another KMS path.
- Backup target selection.
- GitHub branch ruleset implementation.

## Next Steps

1. Run local validation.
2. Push the updated draft PR branch.
3. Watch GitHub Actions for PR #8.
4. Continue with a dedicated threat model v1 PR or convert PR #8 to review-ready after human
   acceptance.
