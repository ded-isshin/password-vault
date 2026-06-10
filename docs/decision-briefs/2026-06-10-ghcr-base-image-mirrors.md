# Decision Brief: GHCR Mirrors For Reviewed Base And CI Images

Status: accepted for MVP CI hardening.

Date: 2026-06-10.

## Context

Issue #98 tracks release fragility caused by Docker Hub availability. Digest pinning already removed
silent tag drift for the Rust build image, Debian runtime image, PostgreSQL service image, Node
synthetic image, and k6 image. It did not remove Docker Hub as the registry contacted during builds.

The project goal is not to build release artifacts on the mini-PC. Release builds should stay on
GitHub-hosted runners and publish product images to GHCR.

## Decision

Use GHCR as the preferred mirror for reviewed base and CI images used by Password Vault workflows.

The repository now has a `base-image-mirrors` GitHub Actions workflow that copies reviewed
upstream image digests into GHCR using `docker buildx imagetools create`. The mirror tags include
the upstream digest in the tag name so digest refreshes remain reviewable in pull requests.

Container and load workflows now select images in this order:

1. Use the GHCR mirror if it exists and is readable.
2. Fall back to the pinned upstream digest if the mirror is not available yet.

The Dockerfile keeps upstream digest defaults so local builds remain explicit and portable. CI
passes selected base images through build arguments.

## Mirrored Images

Current mirror set:

- `rust:1.96.0-bookworm`
- `debian:bookworm-slim`
- `node:22-bookworm-slim`
- `grafana/k6:2.0.0`

PostgreSQL and Helm remain digest-pinned upstream references for now because GitHub Actions service
containers and simple Helm lint containers start before workflow steps can reliably authenticate to
GHCR. They can be moved later if package visibility and runner behavior are proven.

## Rationale

This approach moves the recurring release-build dependency away from Docker Hub without adding a
self-hosted runner, local mini-PC build step, private registry secret, or custom registry service.

The fallback keeps first-run and fork-PR behavior safe: before mirror packages exist or if a fork
cannot read private packages, CI still uses the already reviewed upstream digest instead of failing
only because the mirror was not bootstrapped.

## Security Notes

- GHCR mirror publication uses the repository `GITHUB_TOKEN` with `packages: write`.
- PR workflows only request `packages: read`.
- No Docker Hub credentials are required.
- The mirror workflow copies already reviewed immutable upstream digests; it does not follow
  floating `latest` tags.
- Mirror tags are still registry tags, so a digest refresh must remain a reviewed dependency change.
  The source digest is embedded in the tag name to make accidental drift visible.
- Product deployments continue to use immutable Password Vault API image digests from GHCR.

## Operational Notes

- The mirror workflow runs weekly, manually, and after relevant workflow/Dockerfile changes on
  `main`.
- If Docker Hub is unavailable when the mirror workflow runs, product source code is unaffected.
- If a mirror is missing during PR smoke, the build falls back to the upstream pinned digest and
  prints that choice in the workflow log.
- A future hardening step can fail closed on missing mirrors after all GHCR packages are known to be
  public or readable by the required GitHub Actions contexts.

## Remaining Work

- Prove the mirror workflow on `main` and verify the resulting GHCR packages are readable by normal
  PR checks.
- Decide whether PostgreSQL service, Helm lint, and other tool images should move to GHCR mirrors
  after package-read behavior is verified.
- Consider a stricter fail-closed mode for release publishing once the mirrors are proven stable.

## Sources

- GitHub Docs, "Publishing Docker images":
  <https://docs.github.com/en/actions/tutorials/publish-packages/publish-docker-images>
- Docker Docs, "GitHub Actions cache":
  <https://docs.docker.com/build/cache/backends/gha/>
- Docker Docs, "Supply chain attestations":
  <https://docs.docker.com/build/metadata/attestations/>
