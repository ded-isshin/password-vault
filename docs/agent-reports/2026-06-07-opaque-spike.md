# Session Report: OPAQUE Compatibility Spike

## Goal

Resolve whether OPAQUE should become the MVP auth default or remain a future preferred migration
path. Related issue: #24.

## Active Context

- Active repository: `password-vault`
- Branch: `docs/24-opaque-spike`
- Out of scope: product code, runtime secrets, infrastructure, deployment

## Work Completed

- Updated `docs/research/opaque-browser-compatibility-2026-06-07.md`.
- Updated `docs/development.md` with the required Rust Docker PATH behavior found during validation.
- Recorded package and container evidence.

## Decision

OPAQUE is not the MVP default.

The MVP default remains:

```text
derived-auth-key + account secret key + server-side slow hash + TOTP
```

OPAQUE remains the preferred future auth protocol and can still replace the default before
implementation only if a dedicated PoC proves browser/Rust interoperability, acceptable browser
performance, and operational handling.

## Evidence

- RFC 9807 makes OPAQUE attractive for avoiding password disclosure to the server.
- `opaque-ke@4.0.1` is a credible Rust server candidate, but current crates.io latest resolves to a
  pre-release line. MVP should not depend on the pre-release line without a separate decision.
- `@serenity-kit/opaque@1.1.0` is the main browser candidate, has Vite examples, and reports a
  security review, but Rust server interoperability must still be proven.
- The browser package README documents practical Argon2 memory constraints in browser environments.
- `opaque-wasm` appears older and is not the primary MVP candidate.
- Containerized Rust checks work when `/usr/local/cargo/bin` is added to `PATH`.

## Subagents Used

Yes. A report-only auth/crypto library reviewer completed.

Accepted suggestions:

- Keep derived-auth-key as MVP default.
- Treat OPAQUE as time-boxed PoC and future migration path.
- Keep protocol-neutral auth start/finish endpoints.
- Add explicit `auth_protocol` field.
- Keep vault unlock/wrapping separate from auth migration.

Deferred suggestions:

- Actual OPAQUE browser/Rust interoperability PoC.
- Choosing Ristretto255 versus P-256.
- OPAQUE server setup secret operational runbook.

## Claude Code Used?

Not for this narrow update. Claude Code already reviewed the broader MVP execution plan and flagged
OPAQUE default contradiction. This update follows that accepted recommendation.

## Commands Run

- `npm view @serenity-kit/opaque ... --json`
- `npm view @serenity-kit/opaque readme --json`
- `npm view opaque-wasm ... --json`
- `npm view @serenity-kit/opaque-p256 ... --json`
- `docker run --rm rust:1.96-bookworm ...`
- `docker run --rm rust:1.96-slim-bookworm ...`
- `docker run --rm rust:1.85-bookworm ...`
- `docker run --rm rust:1.85-bookworm sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo info opaque-ke@4.0.1'`

## Files Changed

- `docs/development.md`
- `docs/research/opaque-browser-compatibility-2026-06-07.md`
- `docs/agent-reports/2026-06-07-opaque-spike.md`

## Validation

Pending final local validation.

## Risks

- Derived-auth-key is weaker than OPAQUE against live backend compromise observing replayable
  password-equivalent auth material.
- OPAQUE migration will require explicit user re-authentication and OPAQUE credential enrollment.
- Silent downgrade between auth protocols would be a security bug.

## Approval Needed

No infrastructure approval is needed for this docs PR.

