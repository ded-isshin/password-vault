# Session Report: CI Base Image Digest Pinning

Status: draft report for the #98 follow-up.

## Goal

Reduce container build drift and CI fragility after the main container workflow hit a Docker Hub
token endpoint `504` while resolving the Rust build image.

## Active context

- Active repository: `password-vault`
- Out of scope: infrastructure GitOps rollout and Kubernetes mutation
- Risk: medium, because this changes CI and container supply-chain inputs

## Work completed

- Pinned the Dockerfile Rust build image by immutable digest.
- Pinned the Dockerfile Debian runtime image by immutable digest.
- Pinned PostgreSQL 18 service containers in Rust, container, and load workflows by immutable digest.
- Pinned the k6 load-test image by immutable digest in container and load workflows.
- Updated development and container CI research docs to describe digest pinning and future GHCR mirror
  consideration if Docker Hub availability keeps affecting release builds.

## Validation

Tested:

- `docker build --build-arg BUILD_REVISION="$(git rev-parse HEAD)" -t password-vault-api:pin-ci-base-images .`
- Runtime smoke against the locally built image:
  - `/healthz`
  - `/readyz`
  - public `/metrics` returns `404`
  - internal metrics include HTTP and build info metrics
- `docker run --rm grafana/k6:2.0.0@sha256:a33a0cfdc4d2483d6b7a3a22e726a499ff2831a671a49239104cd34a9937523c version`
- `docker run --rm postgres:18-bookworm@sha256:501c9112cb737119b90618d7c09ad4eaab243e4b370050eb280061284cd2ed63 postgres --version`
- Required docs file existence check
- `node --check crates/api/static/app.js`
- `node --check load/synthetic/browser-api-journey.mjs`
- `SYNTHETIC_SELF_TEST_ONLY=true node load/synthetic/browser-api-journey.mjs`
- Public-safety grep equivalent to `.github/workflows/security.yml`
- `git diff --check`

Not tested yet:

- GitHub Actions PR checks for this branch
- GitHub Actions main container publish after merge
- GHCR base-image mirroring

## Official sources consulted

- Docker Build with GitHub Actions
- Docker Build GitHub Actions cache backend
- Docker Build attestations and SBOM/provenance docs
- GitHub publishing Docker images
- GitHub artifact attestations

## Claude Code usage

Purpose: independent CI/supply-chain review.

Summary of output:

- No blocking findings.
- Confirmed the GitHub Actions service-image digest syntax, shell `K6_IMAGE` use, and Dockerfile
  `ARG`/`FROM` pattern are valid.
- Flagged that digest pinning is an integrity/drift mitigation, not a full availability fix for
  Docker Hub token endpoint failures.
- Recommended keeping #98 open for GHCR mirrors, Docker Hub authentication, or pull retry/backoff.
- Recommended documenting a digest refresh procedure because some digests are intentionally repeated
  across workflows and docs.

Accepted suggestions:

- Documented that Docker Hub availability risk remains after digest pinning.
- Added a digest refresh procedure.
- Kept GHCR mirroring/auth/retry as #98 follow-up work rather than claiming this PR fully closes it.

## Risks

- Digest pinning removes tag drift but does not fully remove Docker Hub as an upstream dependency.
- Digest refreshes need dependency-review discipline so the repository does not silently accumulate
  stale base images.
- Full Docker Hub independence requires operating GHCR mirrors for reviewed base/test images; that
  is deliberately left as a follow-up unless flakes repeat.

## Next steps

1. Open a PR for #98.
2. Let GitHub Actions validate service-container digest syntax and container smoke.
3. If checks pass, merge and record the result on #98.
