# Session Report: MVP Execution Bootstrap

## Goal

Turn the password-vault idea into an executable MVP delivery program with GitHub issues, milestone,
Project fields, official-docs research, and a durable implementation plan.

## Active Context

- Active context: `password-vault + infrastructure-home`
- Product repository: `<redacted-path>/products/password-vault`
- Infrastructure repository: `<redacted-path>/infrastructure-home`, read-only inspection only
- GitHub repository: `ded-isshin/password-vault`
- GitHub Project: <https://github.com/users/ded-isshin/projects/2>

## Work Completed

- Created milestone `v0.2-working-mvp`.
- Created delivery issues #11 through #23.
- Created OPAQUE/browser compatibility spike #24 in `v0.1-research`.
- Created freshness/rollback issue #25.
- Created Rust build environment issue #26.
- Added issues to GitHub Project #2 and set Status, Priority, Area, Risk, and Phase.
- Added `docs/mvp-implementation-plan.md`.
- Added `docs/research/official-docs-mvp-stack-2026-06-07.md`.
- Added `docs/research/opaque-browser-compatibility-2026-06-07.md`.
- Confirmed the existing infrastructure GitOps pattern is Argo CD multi-source: product Helm chart
  plus production values from `infrastructure-home`.

## Subagents Used

### MVP Planner

Purpose: independent backlog and dependency graph.

Summary:

- Recommended browser-first personal-account MVP.
- Identified critical path: auth/crypto/TOTP decisions, API contract, backend/schema, browser
  crypto, vault sync, frontend, image/chart, GitOps deploy.
- Marked real-secret usage as blocked until backup/restore/failover gates.

Accepted:

- Delivery issues and dependency graph.
- Explicit non-goals for organizations, extension, mobile, and KeePass import.
- Backup/restore gate before real user secrets.

Deferred:

- Exact implementation protocol until ADRs close.

### Platform/GitOps Analyst

Purpose: read-only GitOps pattern review.

Summary:

- Future infrastructure PR should add `kubernetes/gitops/prod/apps/password-vault/` with
  `application.yaml`, `kustomization.yaml`, `values-prod.yaml`, and `README.md`.
- Existing pattern uses product Helm chart source and infra-owned values.
- Secrets, external port/host/path, GHCR pull model, and database operator status remain open.

Accepted:

- GitOps file list.
- Values/secrets placeholders.
- Dedicated approval gate for Argo CD sync or cluster mutation.

Deferred:

- Actual infra edits until product chart exists and a clean infra branch/worktree is prepared.

### Security/Auth/API Analyst

Purpose: independent auth, crypto, TOTP, and API shape review.

Summary:

- Recommended OPAQUE-first auth design.
- Recommended Argon2id, HKDF, AES-GCM, secure server-side session cookies, CSRF controls, and TOTP
  as login MFA.
- Identified OPAQUE/browser library maturity and Argon2id WASM as blockers.

Accepted:

- Created #24 OPAQUE compatibility spike.
- Kept OPAQUE as a spike/preferred upgrade path while preserving derived-auth-key as the current
  MVP default unless #24 proves OPAQUE practical.
- Added security test requirements.

Deferred:

- Exact endpoint message schemas until #2, #3, #4, and #24 are resolved.

## Claude Code Used?

Yes.

Purpose: independent architecture review of the MVP execution plan, auth/crypto direction, API
contract, and Kubernetes/GitOps gates.

Summary of output:

- The documentation baseline is strong, but the new OPAQUE-first wording conflicted with existing
  derived-auth-key-first docs.
- The API contract and sync protocol currently disagree on paths and verbs for item updates and
  delta sync.
- Rollback/revision freshness is required by the threat model but not yet designed in the crypto or
  sync specs.
- Missing local `rustc`/`cargo` is an entry gate for Rust implementation, not just a minor risk.

Accepted suggestions:

- Kept derived-auth-key as the documented MVP default unless #24 proves OPAQUE practical.
- Added freshness/rollback design as a required #3/#13 input.
- Promoted Rust build environment choice to a Phase 2 entry gate.
- Will create separate tracking issues for freshness/rollback protection and Rust toolchain path.

Rejected suggestions:

- None.

Deferred suggestions:

- Exact OpenAPI schema reconciliation for #13.
- Account-secret-key UX details for #2.
- API-first non-browser session model.
- CloudNativePG version pin and install-state verification for #5/#22.

## Commands Run

- `gh repo view ded-isshin/password-vault --json ...`
- `gh issue list -R ded-isshin/password-vault --state open ...`
- `gh project list --owner ded-isshin --format json`
- `gh project field-list 2 --owner ded-isshin --format json`
- `gh project item-list 2 --owner ded-isshin --format json --limit 100`
- `gh api repos/ded-isshin/password-vault/milestones --paginate ...`
- `gh api -X POST repos/ded-isshin/password-vault/milestones ...`
- `gh label create ...`
- `gh issue create ...`
- `gh project item-add 2 --owner ded-isshin --url ...`
- `gh project item-edit ...`
- `find <redacted-path>/infrastructure-home/kubernetes/gitops/prod -maxdepth 4 -type f`
- `sed -n ... <redacted-path>/infrastructure-home/kubernetes/gitops/prod/apps/hiringtrace/application.yaml`
- `git switch -c docs/mvp-execution-plan`
- official web searches for Rust, Axum, SQLx, React, Vite, WebCrypto, RFCs, OWASP, Kubernetes,
  Argo CD, Helm, CloudNativePG, and GitHub Actions
- `rustc --version || true`
- `cargo --version || true`
- `node --version || true`
- `npm --version || true`
- `docker --version || true`
- `kubectl version --client || true`
- `helm version --short || true`
- `argocd version --client || true`
- `npm view @serenity-kit/opaque ... --json`
- `npm view opaque-wasm ... --json`
- `npm view argon2-browser ... --json`
- `claude -p --permission-mode plan --tools "" --no-session-persistence --model opus --effort high ...`

## Files Inspected

- `AGENTS.md`
- `docs/api-contract.md`
- `docs/architecture.md`
- `docs/research/initial-stack-analysis.md`
- `docs/agent-reports/2026-06-07-threat-model-v1.md`
- `<redacted-path>/infrastructure-home/CODEX.md`
- `<redacted-path>/infrastructure-home/kubernetes/gitops/prod/apps/hiringtrace/application.yaml`
- `<redacted-path>/infrastructure-home/kubernetes/gitops/prod/apps/hiringtrace/values-prod.yaml`
- `<redacted-path>/infrastructure-home/kubernetes/gitops/prod/apps/kustomization.yaml`

## Files Changed

- `docs/mvp-implementation-plan.md`
- `docs/research/official-docs-mvp-stack-2026-06-07.md`
- `docs/research/opaque-browser-compatibility-2026-06-07.md`
- `docs/agent-reports/2026-06-07-mvp-execution-bootstrap.md`

## Validation

Tested:

- GitHub milestone and issues exist.
- Project fields for new issues were populated.

Needs verification:

- Markdown/link validation after Claude Code output is incorporated.
- GitHub Actions after PR push.

Not tested:

- No product code exists in this branch.
- No Kubernetes, Helm, Argo CD, Terraform, or runtime secret changes were made.
- Rust build commands were not run because `rustc` and `cargo` are not installed locally.

## Risks

- MVP remains at planning/spec stage; implementation has not started.
- Rust implementation is blocked locally until we choose an approved toolchain path: local install,
  dev container, or container/CI-based build.
- OPAQUE and browser Argon2id dependency choices remain open.
- Future GitOps work must avoid unrelated dirty `infrastructure-home` changes.
- Deployment requires explicit approval and real secret handling outside Git.

## Open Questions

- Public or private GHCR image for MVP?
- Which route: path, host, or external edge port?
- Backup target and credentials source?
- Whether CloudNativePG is already installed and ready.

## Next Steps

1. Incorporate Claude Code architecture review.
2. Validate docs locally.
3. Open a PR for #12.
4. Start #24 OPAQUE/browser spike and #13 API contract work.
5. Start backend scaffold only after security-sensitive protocol boundaries are clear enough.

## Approval Needed

No additional approval is needed for the current docs/issue planning PR.

Explicit approval remains required for repository settings/secrets, runtime secrets, cluster
mutation, Argo CD sync, Terraform/Helm/Kubectl mutation, and accepting real user secrets.
