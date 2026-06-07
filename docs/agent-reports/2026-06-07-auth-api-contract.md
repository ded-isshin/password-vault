# Session Report: Auth Protocol And API Contract

## Goal

Define the MVP authentication protocol direction and update the `/v1` API contract enough for backend
and frontend implementation planning. Related issues: #2 and #13.

## Active Context

- Active repository: `password-vault`
- Branch: `docs/2-auth-api-contract`
- Out of scope: product code, runtime secrets, infrastructure, deployment

## Work Completed

- Added `docs/security/auth-protocol-v1.md`.
- Updated `docs/api-contract.md` with protocol-neutral auth start/finish endpoints.
- Updated `docs/auth-mfa-lifecycle.md` with `derived-auth-v1`, account secret key, pre-MFA state, and
  login/unlock separation.
- Updated ADR 0003 status and decision for MVP planning.
- Linked the new auth protocol doc from `README.md`.

## Decision

Use `derived-auth-v1` for MVP planning and implementation.

OPAQUE remains the preferred future auth protocol, but #24 did not prove enough browser/Rust
interoperability to make it the MVP default.

## Security Boundaries

- Raw master password: never sent to backend.
- Account secret key: never sent to backend.
- Account unlock key: never sent to backend.
- Unwrapped vault keys: never sent to backend.
- `client_auth_secret`: password-equivalent; backend receives it only through the documented
  `derived-auth-v1` proof/material path and stores only a slow server-side hash.
- TOTP: login MFA only, never vault encryption.

## Subagents Used

No new subagent for this branch. This work uses the completed #24 OPAQUE library reviewer and prior
Claude Code architecture review.

## Claude Code Used?

Yes.

Purpose: independent security/API review for issues #2 and #13.

Summary of output:

- The direction is adequate as an MVP planning spec.
- Blocking issues before implementation spec: pre-auth MFA hint leaked enrollment state,
  `proof/material` was undefined, and registration enumeration was not addressed.

Accepted suggestions:

- Removed pre-auth `mfa_required_hint`.
- Clarified that `derived-auth-v1` must use challenge-bound proof material, not raw reusable
  `client_auth_secret`.
- Added non-enumerating registration behavior as a requirement.

Deferred suggestions:

- Exact proof construction and test vectors.
- Server-side slow hash algorithm and parameter names.
- Timing-enumeration mitigation.
- Account-secret-key new-device/lost-secret UX.

## Commands Run

- `gh issue view 2`
- `gh issue view 13`
- `sed -n ... docs/auth-mfa-lifecycle.md`
- `sed -n ... docs/security/crypto-design-draft.md`
- `sed -n ... docs/api-contract.md`
- `sed -n ... docs/adr/0003-auth-and-crypto-direction.md`
- `sed -n ... docs/decision-briefs/2026-06-07-auth-crypto-mvp.md`
- `git switch -c docs/2-auth-api-contract`
- `claude -p --permission-mode plan --tools "Read,Glob,Grep" --no-session-persistence --model opus --effort high ...`

## Files Changed

- `README.md`
- `docs/adr/0003-auth-and-crypto-direction.md`
- `docs/api-contract.md`
- `docs/auth-mfa-lifecycle.md`
- `docs/security/auth-protocol-v1.md`
- `docs/whitepaper.md`
- `docs/agent-reports/2026-06-07-auth-api-contract.md`

## Validation

Pending final local validation.

## Risks

- `derived-auth-v1` is weaker than OPAQUE against a live backend that can observe replayable
  password-equivalent auth material.
- Account secret key UX and new-device behavior still need concrete implementation detail.
- Server-side slow hash algorithm and parameters must be selected before code.
- TOTP seed custody remains in #4.

## Approval Needed

No infrastructure approval is needed for this docs PR.
