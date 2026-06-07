# Threat Model

Status: v1 draft for web MVP. Product code is currently limited to the Rust API scaffold.

## Scope

This threat model covers the first public web MVP for `password-vault`:

- personal account registration and login;
- account secret key as the recommended second KDF input;
- TOTP MFA and recovery codes;
- browser-side vault unlock and item encryption;
- versioned `/v1` API contracts;
- server-side authorization, sessions, audit events, and sync metadata;
- PostgreSQL data, backups, and CloudNativePG deployment assumptions;
- public repository, GitHub Actions, GHCR container images, and GitOps handoff.

Out of scope for this version:

- organizations and shared vaults;
- browser extension autofill;
- mobile and desktop clients;
- production Kubernetes manifests;
- live infrastructure details;
- billing and admin operations.

## Security Goals

- Server code cannot read plaintext vault item contents.
- Database compromise does not reveal plaintext vault item contents.
- A copied authentication database is not enough for normal password-only offline guessing.
- A live compromised backend has explicitly documented limits and residual risks.
- Clients can detect or safely handle stale, swapped, or downgraded vault revisions before trusting
  decrypted data.
- One user cannot access another user's account, vault records, devices, sessions, or audit events.
- TOTP protects login but is never treated as vault encryption.
- Recovery codes recover login-factor access only, not vault contents.
- Logs, metrics, traces, CI output, and audit events never include secret values.
- Public repository content does not expose private infrastructure data.
- Real user secrets are not accepted until backup, restore, and failover gates pass.

## Assets

| Asset | Sensitivity | Notes |
| --- | --- | --- |
| Master password / unlock secret | Critical | User-controlled input; must not be sent to backend. |
| Account secret key | Critical | Second KDF input; not stored server-side in plaintext. |
| Client-derived auth secret | Critical | Password-equivalent; backend stores only slow server-side hash. |
| User/vault keys | Critical | Never sent to backend unwrapped. |
| Encrypted item payloads | High | Ciphertext, but still sensitive and integrity-protected. |
| Plaintext item fields | Critical | Titles, URLs, usernames, passwords, notes, tags, custom fields. |
| TOTP seeds | High | Server-owned auth secret; needed for TOTP verification. |
| TOTP last-used-step metadata | Medium | Needed to reject replayed TOTP codes. |
| MFA recovery codes | High | Login-factor recovery only; store as one-way verifiers. |
| Session cookies | High | Authorize API access, not vault decrypt. |
| KDF metadata | Medium/High | Must not enable account enumeration or downgrade attacks. |
| Server auth verifier | High | Must not be raw replayable auth material. |
| Synthetic metadata secret | High | Used for non-enumerating login metadata for unknown accounts. |
| Device records | Medium | Audit/revocation metadata; may become cryptographic later. |
| Encrypted key wraps | High | Stored server-side, never unwrapped server-side. |
| Audit events | Medium | Must not contain secret values. |
| PostgreSQL data and WAL/base backups | High | Contains auth data, ciphertext, metadata, TOTP seed ciphertext. |
| Container images and build artifacts | High | Can alter delivered app behavior. |
| Browser bundle, WASM dependencies, service-worker cache | High | Can alter client-side crypto behavior. |
| GitHub repo, PRs, Actions logs | Medium/High | Public; can leak secrets or influence supply chain. |
| Repository rulesets, workflow files, `GITHUB_TOKEN` | High | Control build/review permissions. |
| GHCR tags/digests and future SBOM/provenance | High | Deployment supply-chain evidence. |
| Kubernetes runtime secrets | High | DB credentials, TOTP seed encryption key, app runtime config. |
| Kubernetes service accounts, RBAC, ingress/TLS, NetworkPolicy | High | Runtime isolation and exposure controls. |
| Observability data | Medium/High | Logs, traces, metrics labels, and audit streams can leak metadata. |

## Actors

- legitimate personal user;
- attacker with a copied database or backup;
- attacker with stolen session cookie;
- attacker controlling or compromising a user browser/device;
- malicious or compromised public PR contributor;
- compromised maintainer account or bypassed branch ruleset;
- malicious or mistaken maintainer/operator;
- compromised backend service or dependency;
- compromised CI workflow, build artifact, or container image;
- compromised Kubernetes service account, operator, or runtime secret;
- network attacker limited by TLS assumptions;
- node/pod/storage failure in the Kubernetes environment.

## Trust Boundaries

```text
User
  -> browser web app
  -> static asset / WASM / service-worker delivery
  -> /v1 API over TLS
  -> Kubernetes ingress / API pods
  -> backend service
  -> PostgreSQL / CloudNativePG
  -> object-store backup target

GitHub public repo / Actions
  -> container image build
  -> GHCR image package / digest
  -> GitOps handoff
  -> Argo CD
  -> Kubernetes runtime

Application / audit producers
  -> logs, metrics, traces, audit sinks
```

Important boundaries:

- Login session is not vault unlock.
- Backend authorization is not cryptographic authorization.
- TOTP is server-verified login MFA, not vault decryption.
- PostgreSQL stores ciphertext and metadata, not plaintext item contents.
- Product repository CI does not receive Kubernetes credentials.
- Infrastructure details stay out of the public product repository.
- Product CI and GitOps are separate trust boundaries.
- Observability sinks are public-safety sensitive even when they do not store vault data.

## Key Data Flows

### Registration And Login

1. User enters login handle, password, and account secret key.
2. Browser derives local auth and unlock material.
3. Browser sends only client-derived auth material or proof-like login data to backend.
4. Backend verifies a slow server-side hash and then TOTP if enrolled.
5. Backend issues an opaque session cookie.

Threat focus:

- password or account secret key exfiltration;
- user enumeration through pre-login metadata;
- offline guessing after database compromise;
- denial of service through expensive KDF/hash verification;
- TOTP replay and seed exposure.
- CSRF and session fixation for cookie-authenticated APIs.

MVP direction: `derived-auth-v1` uses the `pv-scram-sha-256-v1` verifier/proof profile documented
in [auth-protocol-v1.md](security/auth-protocol-v1.md). OPAQUE may reduce verifier and live-backend
auth-channel risk later, but needs library and browser review. The remaining implementation gate is
exact code-level transcript encoding and test vectors in #16.

### Vault Item Write

1. Browser unlocks local vault key material.
2. Browser encrypts item revision payload locally.
3. Browser submits ciphertext and allowed sync metadata to `/v1`.
4. Backend authorizes account/vault access and stores immutable revision.

Threat focus:

- backend decrypt path accidentally introduced;
- cross-user authorization bug;
- AES-GCM nonce/key misuse;
- tampering with associated data;
- stale revision, rollback, key-wrap substitution, or crypto metadata downgrade;
- metadata leakage.

### Vault Item Read And Sync

1. Browser requests encrypted revisions through `/v1`.
2. Backend returns ciphertext, key wraps, revision metadata, and allowed sync metadata.
3. Browser verifies version, associated data, revision ordering, and decrypts locally.

Threat focus:

- malicious server returns stale data or hides a new revision;
- ciphertext is swapped between users, vaults, items, or revisions;
- crypto algorithm, KDF parameters, or key epoch are downgraded;
- device/session revocation is misunderstood as erasing copied local data.

### Backup And Restore

1. CloudNativePG writes PostgreSQL data and WAL.
2. WAL archives and physical base backups are written to object storage.
3. Restore is tested into a separate namespace or cluster object.

Threat focus:

- backup exposure;
- missing WAL or base backup;
- untested restore;
- overwriting live data during drills;
- backup deletion, poisoning, ransomware, or missing retention;
- accepting real secrets before backup gates pass.

### CI, Image, And GitOps

1. Product changes enter through GitHub issues, branches, and PRs.
2. GitHub Actions validate docs and public safety on GitHub-hosted runners.
3. Release jobs build and publish images to GHCR.
4. Infrastructure deployment happens through a separate GitOps PR.

Threat focus:

- workflow-file tampering or ruleset bypass;
- mutable third-party Actions, runner-image drift, or dependency compromise;
- malicious image tag overwrite or digest mismatch in GitOps;
- public PR exfiltration through future build jobs;
- kubeconfig or cluster credentials accidentally added to product CI.

## Threats And Required Responses

| Threat | Impact | Required response | Follow-up |
| --- | --- | --- | --- |
| Raw master password reaches backend | Breaks zero-knowledge boundary | Reject password-over-TLS design; tests prove backend never receives raw password | #2, #3 |
| Account secret key stored server-side in plaintext | Enables password-only database attacks | Generate client-side, show/save through UX, never persist plaintext server-side | #2 |
| Pre-login metadata reveals account existence | Account enumeration | Constant-shape responses, synthetic metadata for unknown accounts, generic errors, rate limits | #2 |
| Copied auth database enables guessing | Vault/account compromise risk | Account secret key, explicit PBKDF2 browser-MVP profile, Argon2id hardening target, SCRAM-like verifier material, rate limits | Auth protocol, #3 |
| Silent KDF downgrade or mixed-profile enumeration | Weakens unlock/auth security or reveals legacy accounts | Versioned KDF profile; PBKDF2 is an explicit browser-MVP decision, not runtime fallback; pre-MVP Argon2id rows are migrated rather than served as separate login metadata; future changes need migration plan | #3 |
| Browser-delivered JavaScript is malicious | Unlock material theft | Accepted residual risk; pinned/reviewed dependencies, no third-party auth scripts, service-worker/cache policy, future stronger clients | #3, #9 |
| AES-GCM nonce/key misuse | Payload confidentiality/integrity failure | Per-revision content keys or strict nonce budget/rekey spec and tests | #3 |
| Malicious server serves stale or rolled-back vault revisions | User trusts old or hidden data | Client-verifiable monotonic revision or hash-chain design bound into AAD; stale-revision tests | #3, API contract |
| Malicious server swaps ciphertext, key wraps, or crypto metadata | Wrong data decrypts or downgrade succeeds | AAD binds user, vault, item, revision, key epoch, algorithm/version; downgrade rejection tests | #3 |
| Backend authorization bug exposes another user's records | Cross-user data exposure | Authorization tests for every account/vault/device/session path | #2, API contract |
| Backend decrypt path is introduced | Zero-knowledge failure | Negative tests proving backend-only code cannot decrypt item payloads | #3 |
| Live compromised backend abuses auth flow | Account/session compromise | Keep OPAQUE as future migration path; document residual runtime risk | Auth protocol |
| TOTP seed exposure | MFA bypass | Encrypt TOTP seeds at rest with app-level AEAD runtime key; keep Vault/OpenBao/KMS as future hardening | ADR 0005 |
| Replayed TOTP step | MFA bypass | Track last accepted timestep, use narrow window, enforce rate limits | ADR 0005 |
| Synthetic metadata secret compromise | Account enumeration or metadata prediction | Secret custody, backup, rotation, and synthetic-response tests | #2 |
| Recovery code decrypts vault data | Zero-knowledge failure | Recovery codes only recover login-factor access; store as one-way verifiers | ADR 0005 |
| Stolen session cookie | Account API access | HttpOnly, Secure, SameSite Strict cookies; session rotation, expiry, revocation, audit | ADR 0005 |
| CSRF against cookie-authenticated `/v1` mutations | Unauthorized state changes | `SameSite=Strict`, origin checks, Fetch Metadata, required `X-PV-CSRF`, tests | ADR 0005, API contract |
| Session fixation or weak session state transition | MFA/session bypass | Pre-MFA and post-MFA states, rotation after MFA/recovery, idle/absolute expiry, revoke-all semantics | ADR 0005 |
| Public PR abuses GitHub Actions | Secret or supply-chain exposure | GitHub-hosted runners only, minimal permissions, no secrets for untrusted PRs | #7 |
| Workflow or ruleset tampering | Review and CI bypass | Branch rulesets, required checks, CODEOWNERS for sensitive paths | #7 |
| Third-party Action or dependency compromise | Supply-chain compromise | Prefer trusted/pinned Actions, dependency review, Dependabot, release hardening | #7 |
| CI publishes malicious or mutable image | Supply-chain compromise | Restricted workflow permissions, GHCR publishing, SBOM/provenance, GitHub attestation, digest pinning | #7 |
| GHCR tag and GitOps digest drift | Wrong image deployed | GitOps should reference immutable image digests, not only mutable tags | #7 |
| Logs expose secrets | Secret leakage | Redaction, no secret values in logs/audit/CI, public-safety scans | #7 |
| Metrics or traces leak sensitive metadata | Metadata disclosure | Avoid secret values and high-cardinality sensitive labels; retention/access policy | #7 |
| PostgreSQL primary/node failure | Data loss or downtime | CloudNativePG 3 instances, anti-affinity, sync replication for real data | #5 |
| Backup target missing or restore untested | Irrecoverable data loss | No real user secrets until WAL, base backups, and restore drill pass | #5 |
| Backup deletion, poisoning, or restore failure | Irrecoverable or corrupted data | Object-store retention/immutability decision, restore into separate namespace, RTO/RPO recording | #5 |
| Object-store backup exposure | Auth data/ciphertext disclosure | Dedicated backup config, credential isolation, encryption, key separation, retention policy | #5 |
| Kubernetes service account or RBAC abuse | Runtime compromise | Least-privilege service accounts, RBAC review, no broad cluster role for app | #5 |
| Missing NetworkPolicy or ingress/TLS mistake | Lateral movement or public DB exposure | NetworkPolicy/RBAC/ingress requirements; PostgreSQL is never public | #5 |
| CloudNativePG operator or CRD compromise | Database control-plane compromise | Operator version review, namespace/RBAC separation, backup/restore validation | #5 |
| Direct cluster mutation bypasses GitOps | Unreviewed deployment risk | Product repo does not run `kubectl apply`; deployment through infrastructure PR | #5, #7 |

## Key Hierarchy And Custody

Working direction:

```text
password + account secret key
  -> browser KDF
  -> unlock material
  -> unwrap user/vault key material
  -> vault/root data key
  -> per-item revision content key
```

Server-side storage may include encrypted key wraps, KDF metadata, crypto version metadata, and item
revision ciphertext. The backend must never receive unwrapped vault keys or plaintext item payloads.

The crypto spec must define:

- wrapping algorithm and version;
- key-wrap AAD fields;
- item payload AAD fields;
- algorithm and KDF parameter versioning;
- key epoch behavior;
- downgrade rejection;
- how a second browser retrieves wrapped keys and unlocks locally.

Minimum AAD direction: bind ciphertext and key wraps to user/account ID, vault ID, item ID, revision
ID, key epoch, algorithm version, and payload purpose where applicable.

## Accepted Residual Risks

### Browser-Delivered Crypto

The web MVP depends on JavaScript delivered by the same service the user is logging into. If that
delivery path is compromised, malicious JavaScript can steal password, account secret key, or unlock
material before encryption.

This is accepted for the web MVP only if minimum gates are defined before real secrets: pinned and
reviewed WASM, dependency review, no third-party scripts on auth/unlock pages, service-worker/cache
policy, security headers, public-safety CI, and independent review for auth/crypto changes.

CSP and SRI are partial controls. They do not fully protect against a same-origin server that can
replace both HTML and bundle hashes. Reproducible or signed builds, dependency review, and future code
transparency are stronger deferred controls. Future extension, desktop, and mobile clients can reduce
browser-delivery risk through stronger release artifacts, but they do not remove supply-chain risk.

### Metadata Visibility

The MVP should assume that item existence, item count, ciphertext size, update timing, device/session
metadata, and audit event timing are visible to the server. The recommended MVP boundary encrypts
titles, URLs, usernames, passwords, notes, tags, and custom fields, so server-side content search is
not available.

Operational metrics can also leak metadata through labels, counts, and timing. Metrics and traces
must be designed as public-safety sensitive artifacts.

### Recovery Limits

Forgotten password, account secret key, or vault unlock material should be treated as unrecoverable
unless a future zero-knowledge recovery design is approved. MFA recovery codes recover login-factor
access only.

Soft device/session revocation cannot erase ciphertext, account secret keys, browser storage, or
plaintext already copied from a compromised device. The MVP accepts this limitation until strong
device enrollment and client-side local-storage policy are designed.

### Degraded Database Availability

For real user data, the initial recommendation is synchronous replication with `dataDurability:
required`. This can pause writes during degraded states. The product accepts temporary write
unavailability as safer than acknowledging a saved password and then losing it.

The product also depends on one cluster/site until a later multi-site disaster-recovery design exists.
Local-path storage is not portable between worker nodes; database survivability depends on
PostgreSQL replication and tested restore, not volume mobility.

### Public Tooling And CI Limits

GitHub-hosted runners, public repository settings, and third-party Actions are trusted dependencies.
The bootstrap public-safety workflow is a best-effort pattern scan, not proof that public content
cannot leak sensitive data. Release workflows must add stronger supply-chain controls before product
code or images are trusted for real secrets.

## Required Tests And Evidence Before Product Code Is Trusted

- Registration/login tests proving raw password and account secret key are not sent to backend.
- Server storage tests proving raw client auth secret is not persisted.
- Pre-login metadata non-enumeration tests.
- Rate-limit and anti-DoS tests before slow server-side auth verification.
- TOTP RFC 6238 vectors and replay rejection tests.
- Session cookie attribute, rotation, expiry, and revocation tests.
- CSRF protection tests for state-changing `/v1` routes.
- Session fixation and pre-MFA/post-MFA transition tests.
- Cross-user and cross-vault authorization tests.
- Generic error, 403/404, and non-enumeration tests for object-access endpoints.
- AES-GCM round-trip, associated-data tamper, and nonce/key-budget tests.
- Stale revision, rollback, key-wrap substitution, and crypto downgrade rejection tests.
- AAD-binding tests proving ciphertext cannot be swapped between users, vaults, items, or revisions.
- Negative test proving backend-only code cannot decrypt item payloads.
- Constant-time comparison tests or implementation review for auth verifier, TOTP, and recovery-code
  checks.
- Ciphertext size limit and write quota tests.
- Log, trace, metric, and panic redaction tests.
- Public-safety secret scan in CI.
- GitHub Actions minimal-permission review.
- Backup, restore, and failover drill evidence before real user data.

## Open Decisions

- #16: Exact `pv-scram-sha-256-v1` transcript encoding and test vectors.
- #16: Implementation of session, CSRF, rate-limit, and lockout behavior from ADR 0005.
- #3: Browser KDF, crypto payload format, nonce/rekey policy, and dependency review.
- #3: Revision rollback protection, AAD fields, key hierarchy, and crypto payload versioning.
- #3: Browser bundle integrity, service-worker/cache policy, and WASM dependency gates before real
  secrets.
- Runtime operations: TOTP seed-protection-key backup, restore, and rotation drill before real users.
- #5: PostgreSQL HA, backup target, restore drill, and failover plan.
- #5: Kubernetes namespace/RBAC/NetworkPolicy/ingress assumptions and CloudNativePG operator risk.
- #7: Branch ruleset, CODEOWNERS, and public repository safety gates.
- #7: GitHub Actions pinning, dependency review, GHCR image digest/provenance/SBOM/signing policy.
- #9: Multi-device client and browser extension roadmap.
- #9: Soft device revocation limits and future strong device enrollment.
- API contract: exact `/v1` request/response/error shapes.
- Recovery: whether a zero-knowledge-compatible vault recovery key exists in MVP.
- Browser bundle integrity: CSP/SRI limitations, signed/reproducible build, and code transparency
  strategy.

## Sources

- https://cheatsheetseries.owasp.org/cheatsheets/Threat_Modeling_Cheat_Sheet.html
- https://owasp.org/www-project-application-security-verification-standard/
- https://pages.nist.gov/800-63-4/sp800-63b.html
- https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html
- https://www.rfc-editor.org/rfc/rfc6238.html
- https://www.rfc-editor.org/rfc/rfc9106.html
- https://www.w3.org/TR/webcrypto/
