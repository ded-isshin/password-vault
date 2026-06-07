# Research Note: MVP Stack Official Documentation

Status: draft. Date: 2026-06-07.

## Why This Matters

`password-vault` is security-sensitive and Kubernetes-native. Stack choices must be based on current
official documentation, standards, and clearly recorded implementation constraints rather than
memory or convenience.

## Official Documentation Checked

Rust and backend:

- Rust Cargo workspaces: <https://doc.rust-lang.org/stable/cargo/reference/workspaces.html>
- Axum docs.rs: <https://docs.rs/axum/latest/axum/>
- SQLx docs.rs: <https://docs.rs/sqlx/latest/sqlx/>
- SQLx migrations: <https://docs.rs/sqlx/latest/sqlx/macro.migrate.html>

Frontend and browser crypto:

- React start-a-new-project guidance: <https://react.dev/learn/start-a-new-react-project>
- React installation guidance: <https://react.dev/learn/installation>
- Vite guide: <https://vite.dev/guide/>
- Vite build guide: <https://vite.dev/guide/build>
- Vite env and modes: <https://vite.dev/guide/env-and-mode.html>
- MDN Web Crypto API: <https://developer.mozilla.org/en-US/docs/Web/API/Web_Crypto_API>
- MDN SubtleCrypto: <https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto>

Crypto/auth standards and security guidance:

- RFC 9807 OPAQUE: <https://www.rfc-editor.org/rfc/rfc9807.html>
- RFC 9106 Argon2: <https://www.ietf.org/rfc/rfc9106.html>
- RFC 6238 TOTP: <https://www.rfc-editor.org/rfc/rfc6238>
- RFC 5869 HKDF: <https://www.rfc-editor.org/rfc/rfc5869>
- OWASP Password Storage Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html>
- OWASP Session Management Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html>
- OWASP CSRF Prevention Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html>

Kubernetes, GitOps, database, and CI:

- Kubernetes probes:
  <https://kubernetes.io/docs/concepts/workloads/pods/probes/>
- Argo CD multiple sources:
  <https://argo-cd.readthedocs.io/en/release-2.10/user-guide/multiple_sources/>
- Helm chart best practices:
  <https://docs.helm.sh/docs/chart_best_practices/>
- Helm values best practices:
  <https://docs.helm.sh/docs/chart_best_practices/values/>
- CloudNativePG current docs: <https://cloudnative-pg.io/documentation/current/>
- CloudNativePG replication: <https://cloudnative-pg.io/docs/1.27/replication/>
- GitHub Actions `GITHUB_TOKEN`:
  <https://docs.github.com/en/actions/concepts/security/github_token>
- GitHub Actions token permissions:
  <https://docs.github.com/actions/using-jobs/assigning-permissions-to-jobs>
- GitHub Actions workflow syntax:
  <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>
- GitHub publishing Docker images:
  <https://docs.github.com/en/actions/tutorials/publish-packages/publish-docker-images>

OPAQUE library evidence:

- `opaque-ke` docs.rs: <https://docs.rs/opaque-ke/latest/opaque_ke/>

## Current Behavior Relevant To Us

- Cargo workspaces support shared `Cargo.lock`, common commands across members, shared target
  directory, and workspace metadata. This fits a Rust backend with potential shared crates.
- Axum is Tokio/Hyper based and composes routing, extractors, responses, and Tower middleware.
- SQLx supports PostgreSQL and migrations; embedded migrations need care so migration changes
  trigger rebuilds.
- React recommends frameworks for production apps, but Vite remains a documented path for building
  from scratch when the app constraints do not need a full-stack React framework.
- Vite exposes frontend environment variables with `VITE_` prefix; those values are public client
  build data and must not be treated as secrets.
- WebCrypto/SubtleCrypto is available only in secure contexts and exposes low-level primitives that
  are easy to misuse. It supports AES-GCM and HKDF, but not Argon2id.
- OPAQUE is now specified in RFC 9807. `opaque-ke` advertises RFC 9807 support and Argon2 feature
  support, but browser integration and package maturity still need a dedicated spike.
- Kubernetes readiness probes should remove unready pods from service endpoints; liveness/startup
  probes have different restart semantics.
- Argo CD supports multiple sources, including Helm chart source plus values from another Git
  source. This matches the existing infrastructure handoff pattern.
- CloudNativePG supports PostgreSQL streaming replication and synchronous replication configuration.
- GitHub recommends least required `GITHUB_TOKEN` permissions; `pull_request_target` has elevated
  permission implications and should not be used for untrusted PR logic.

## Best Practices For This MVP

- Keep the backend as a small Rust workspace with explicit crates only when needed.
- Use Axum for the API service and Tower middleware for tracing, timeouts, request limits, and
  security controls.
- Use SQLx migrations with deterministic SQL files and CI validation.
- Use React/Vite for a browser-first SPA, but do not put runtime secrets in Vite environment
  variables.
- Use WebCrypto for browser-native AES-GCM and HKDF.
- Treat Argon2id and OPAQUE browser support as research/spike gates before implementation. OPAQUE is
  the preferred security direction, but the existing derived-auth-key direction remains the MVP
  default unless the OPAQUE spike proves practical.
- Use server-side sessions with secure host-prefixed cookies, CSRF tokens, origin/fetch-metadata
  checks, and generic auth errors.
- Use TOTP as login MFA only; do not mix it into vault encryption.
- Use GitHub-hosted runners, least-privilege workflow permissions, and no self-hosted runner for
  public CI.
- Use a product-owned Helm chart and infrastructure-owned production values.
- Use GitOps for deployment handoff; do not mutate the cluster directly from product work.

## Security Considerations

- The server must never receive the master password, plaintext vault item data, or unwrapped vault
  data keys.
- Browser-delivered JavaScript is a residual risk. Strict CSP, no third-party scripts, dependency
  review, and reproducible builds reduce but do not remove that risk.
- OPAQUE is attractive because it avoids sending passwords to the server, but implementation quality
  and browser package maturity matter more than protocol attractiveness.
- TOTP seeds are server-owned secrets; recovery codes are login-factor recovery only and must not
  decrypt user vaults.
- Database backups can contain encrypted vault data and auth/MFA records; backup credentials and
  restore drills are part of the security boundary.
- GitHub Actions workflows in a public repository must not expose secrets to untrusted PRs.

## How We Should Use It

- Proceed with Rust/Axum/SQLx and React/Vite as the MVP scaffold after the Rust build environment
  is resolved.
- Close #24 before accepting #2. If #24 is inconclusive, #2 should use the documented
  derived-auth-key MVP default with explicit risks and a replacement path.
- Close #3 before implementing browser crypto.
- Close #4 before implementing TOTP.
- Close #5 before real user data and before final GitOps deployment approval.
- Create CI early, but keep publish/deployment permissions separate from pull-request validation.
- Use the existing `infrastructure-home` Argo CD multi-source pattern for the deployment PR.

## What Not To Do

- Do not implement a fake or ad hoc auth protocol to unblock UI progress.
- Do not store frontend secrets in Vite env vars.
- Do not store plaintext vault item fields in PostgreSQL.
- Do not use Vault/OpenBao as the user-vault decrypt path.
- Do not run direct `kubectl apply`, `helm upgrade`, or Argo CD sync without explicit approval.
- Do not put private hostnames, IPs, domains, secrets, or kubeconfigs into the public repository.

## Open Questions

- Is OPAQUE practical enough for browser MVP given current libraries?
- Should Rust be installed locally, run through a dev container, or built only in Docker/GitHub
  Actions?
- Which Argon2id WASM package is acceptable after dependency review?
- Is GHCR image public or private for the first deployed MVP?
- Is the public route path-based, hostname-based, or edge-port-based?
- What object storage target will be used for CloudNativePG backups?
- Is CloudNativePG already installed in the cluster, or does it require a separate platform PR?
