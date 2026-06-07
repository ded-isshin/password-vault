# Session Report: Platform Artifacts, Observability, And Load Testing

## Goal

Add the first practical platform layer for the MVP: remote/containerized build artifacts, stable CI,
Helm rollout defaults, load-test scaffolding, and metrics exposure.

## Active Context

- Active repository: `password-vault`
- Branch: `feat/platform-artifacts-observability`
- Related issues: #20, #21, #22, #23
- Infrastructure repository inspected read-only. It was not modified because it has unrelated dirty
  worktree state.

## Work Completed

- Added `rust-toolchain.toml` pinned to Rust `1.96.0`.
- Updated Rust CI containers to `rust:1.96-bookworm`.
- Updated GitHub workflow checkout steps to `actions/checkout@v6` after GitHub CI warned that
  Node.js 20 actions are deprecated.
- Added `/metrics` endpoint using `axum-prometheus` with route/method/status HTTP metrics.
- Collapsed unmatched-route metric labels into `/<unmatched>` to avoid unbounded 404 cardinality.
- Added a multi-stage `Dockerfile` and `.dockerignore`.
- Added runtime CA certificates for future TLS database connections.
- Added container workflow:
  - PR job builds and smoke-tests locally without registry writes.
  - `main` publish job pushes to GHCR with BuildKit SBOM/provenance and GitHub attestation.
- Added product Helm chart with `Deployment`, `Service`, probes, PDB, optional Ingress,
  optional `VMServiceScrape`, and Helm test pod.
- Added read-only-rootfs support with writable `/tmp` `emptyDir`.
- Added Helm validation workflow using `alpine/helm:3.19.0`.
- Added k6 load-test suite using `grafana/k6:2.0.0`.
- Added Dependabot configuration for Cargo, GitHub Actions, and Docker.
- Added release/rollout runbook and container/observability/load research note.

## Decisions

- Use GHCR for product images. Do not use Docker Hub automated builds for release.
- Use Docker Hub only for trusted base/test images with pinned tags.
- Keep PR workflows read-only and separate from publish workflows.
- Do not run migrations automatically in production pods by default.
- Use VictoriaMetrics `VMServiceScrape`, matching the existing infrastructure model.
- Keep Grafana dashboard and Argo CD production values for a separate infra PR/worktree.

## Validation

Tested:

- Rust `1.96.0` container validation:
  - `cargo fmt --all -- --check`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo test --locked --workspace`
- Disposable PostgreSQL `17-bookworm` DB-backed test:
  - `cargo test --locked --workspace -- --test-threads=1`
- Docker image build:
  - `docker build -t password-vault-api:local-smoke .`
- Disposable Docker runtime smoke:
  - `postgres:17-bookworm`
  - `password-vault-api:local-smoke`
  - `/healthz`
  - `/readyz`
  - `/metrics`
  - 404 metrics cardinality probe
- k6 smoke:
  - `grafana/k6:2.0.0 run scenarios/smoke.js`
- Helm validation:
  - `alpine/helm:3.19.0 lint deploy/helm/password-vault`
  - `alpine/helm:3.19.0 template password-vault deploy/helm/password-vault --namespace password-vault --set image.tag=ci --set observability.vmServiceScrape.enabled=true`
- YAML parse:
  - Python `yaml.safe_load` over `.github/workflows/*.yml` and `.github/dependabot.yml`
- Diff hygiene and public-safety grep:
  - `git diff --check`
  - changed-file grep for common token/private-key/kubeconfig/private-IP patterns

Results:

- Non-DB validation: 29 unit tests, 1 migration test, and doc-tests passed.
- DB validation: 29 unit tests, 1 migration test, and doc-tests passed.
- Docker image build passed after runtime CA hardening.
- Runtime smoke passed.
- k6 smoke passed: `checks` 100%, `http_req_failed` 0%, 31 iterations at 2/s.
- Metrics cardinality probe passed: random 404 path rendered as `endpoint="/<unmatched>"` and the
  raw path was absent from `/metrics`.
- Helm lint/template passed.
- YAML parse passed.
- Public-safety grep found only documented placeholders, local loopback addresses, and expected
  public-safety text; no real secrets were identified.

Pending validation:

- GitHub-hosted workflow execution after PR is opened.
- Real Grafana dashboard data after infra PR, Argo CD sync, scrape, and traffic generation.

## Reviews

- Supply-chain subagent recommended GHCR over Docker Hub autobuilds, pinned trusted base images,
  SBOM/provenance, and digest-based deployment.
- Performance subagent recommended k6 for the MVP load suite and small PR smoke tests.
- Observability subagent confirmed the infra model uses VictoriaMetrics/Grafana, product-owned
  `VMServiceScrape`, and infra-owned dashboards.
- Claude Code reviewed the settled diff as independent architecture/security/platform reviewer.
  Accepted findings:
  - Unmatched 404 routes could create unbounded metrics cardinality with the default
    `axum-prometheus` endpoint label behavior. Fixed with `MatchedPathWithFallbackFn` and a test.
  - Runtime image should include CA certificates for future TLS database connections. Fixed.
  - Read-only root filesystem should still provide writable `/tmp`. Fixed with `emptyDir`.
  - Chart README should state that schema migrations remain operator-owned. Fixed.
  Deferred findings:
  - Public `/metrics` must be blocked when ingress is enabled. Documented now; final ingress policy
    belongs in the infrastructure deployment PR.
  - NetworkPolicy should eventually restrict ingress/egress by namespace/service selectors. Deferred
    because the MVP chart leaves NetworkPolicy disabled by default and prod topology belongs in
    infra values.
  - Full SHA pinning for GitHub Actions is desirable. Deferred to a follow-up hardening task because
    current workflows use trusted major-version actions plus Dependabot.
  - Manual load-test rate/duration clamping is useful. The scenario path is validated now; numeric
    clamping is deferred.

## Risks

- The app is not deployed yet, so Password Vault dashboard panels cannot show real data until the
  infra PR, Argo CD sync, and traffic generation happen.
- `/metrics` is exposed on the API port. If ingress is enabled, infra must block public access to
  `/metrics` or move metrics to an internal-only route/listener.
- Runtime secrets and database HA/backup remain infrastructure tasks.
- Image digest pinning must happen in infra values after the first GHCR image is published.
- Heavier load tests should be manual/scheduled, not required PR checks.

## Next Steps

- Finish local validation.
- Run Claude Code read-only review.
- Publish product PR if checks pass.
- Prepare a separate clean `infrastructure-home` PR/worktree for Argo CD application, prod values,
  and Grafana dashboard.
