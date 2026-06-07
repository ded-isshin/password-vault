# Research Note: Container CI, Observability, And Load Testing

Status: draft. Date: 2026-06-07.

## Why This Matters

The MVP should not depend on local mini-PC builds for release artifacts. It also needs stable
Kubernetes rollout behavior, safe telemetry, and basic load testing before production-like
deployment.

## Official Documentation Checked

- Docker Build with GitHub Actions:
  <https://docs.docker.com/build/ci/github-actions/>
- Docker Build GitHub Actions cache:
  <https://docs.docker.com/build/cache/backends/gha/>
- Docker Build SBOM/provenance attestations:
  <https://docs.docker.com/build/ci/github-actions/attestations/>
- GitHub publishing Docker images:
  <https://docs.github.com/en/actions/tutorials/publish-packages/publish-docker-images>
- GitHub artifact attestations:
  <https://docs.github.com/actions/security-guides/using-artifact-attestations-to-establish-provenance-for-builds>
- Kubernetes Deployments:
  <https://kubernetes.io/docs/concepts/workloads/controllers/deployment/>
- Grafana provisioning:
  <https://grafana.com/docs/grafana/latest/administration/provisioning/>
- Prometheus client libraries:
  <https://prometheus.io/docs/instrumenting/clientlibs/>
- Prometheus Operator ServiceMonitor/PodMonitor model:
  <https://prometheus-operator.dev/docs/developer/getting-started/>
- Grafana k6 API load testing:
  <https://grafana.com/docs/k6/latest/testing-guides/api-load-testing/>
- Grafana k6 Docker/local runs:
  <https://grafana.com/docs/k6/latest/get-started/running-k6/>
- Helm chart tests:
  <https://helm.sh/docs/topics/chart_tests/>

## Current Behavior Relevant To Us

- Docker's documented GitHub Actions flow uses Buildx, Docker metadata, registry login, image push,
  and optional SBOM/provenance.
- Docker-hosted GitHub runner images already include current Docker Buildx/BuildKit. The separate
  Docker setup-buildx action creates a containerized builder by default, which can introduce an extra
  Docker Hub pull for the BuildKit daemon image before the product image build even starts.
- GitHub's image publishing docs include GHCR publishing and artifact attestation examples.
- BuildKit cache can use GitHub Actions cache through `docker/build-push-action`.
- Docker Build provenance can expose build args, so build args must not carry secrets.
- Kubernetes Deployment rollout status depends on new pods becoming ready. Readiness probes are the
  gate that keeps unready pods out of service endpoints.
- Grafana provisioned dashboards should be stored as JSON and referenced by stable datasource names
  in this infrastructure.
- The local infrastructure observability model uses VictoriaMetrics `VMServiceScrape` objects, not
  Prometheus Operator `ServiceMonitor`.
- k6 supports constant arrival rate, checks, thresholds, and Docker-based execution.

## Decisions

- Publish product images to GHCR from GitHub Actions. Do not use Docker Hub automated builds as the
  primary build platform.
- Use Docker Hub only for trusted base/test images such as Docker Official `rust`, `postgres`,
  `debian`, and trusted `grafana/k6`/`alpine/helm` images.
- Use pinned image tags and avoid `latest` in CI/load/chart validation.
- Split container CI into a read-only PR smoke job and a separate publish job with `packages`,
  `id-token`, and `attestations` permissions.
- Add BuildKit SBOM/provenance for pushed images and GitHub artifact attestation bound to the image
  digest.
- Prefer the default GitHub-hosted runner Buildx/BuildKit path for single-platform MVP builds instead
  of a separate containerized BuildKit builder. This reduces Docker Hub dependency during CI setup.
- Use k6 as the first load-test tool. Locust and wrk/Vegeta are deferred.
- Add low-cardinality Prometheus HTTP metrics through `/metrics`; do not label metrics with login
  handles, user IDs, device IDs, item IDs, or secret-bearing values.
- Use a product-owned Helm chart with optional `VMServiceScrape`; keep production values and
  dashboards in `infrastructure-home`.

## Best Practices

- Keep PR workflows free of secrets and registry writes.
- Deploy by immutable digest in infrastructure values once the first image is published.
- Use `replicaCount >= 2`, readiness probes, graceful shutdown, a short pre-stop drain, PDB, and
  rolling update settings for live updates.
- Keep schema migrations backward-compatible using expand/contract releases. Do not run every app pod
  as an automatic migration actor in production.
- Keep load tests small in PR and run heavier tests manually or on a scheduled workflow.
- Verify dashboards in two phases: render/query syntax before deployment, then real data after
  deployment and traffic.

## Security Considerations

- Container provenance and SBOM are only useful if deployment references immutable image digests.
- Public repos must not use self-hosted runners with private home infrastructure credentials.
- Build containers must not mount Docker socket, kubeconfig, SSH keys, or real `.env` secrets.
- Metrics can leak metadata. Use route/status/method labels only for MVP HTTP metrics.
- Synthetic test data must use reserved domains and must not contain real secrets.

## How We Should Use It

- Product repo owns source, Dockerfile, image workflow, load tests, chart, and product docs.
- Infrastructure repo owns Argo CD application, production values, runtime secrets contract, and
  Grafana dashboard provisioning.
- GitHub Actions builds and publishes images; the mini-PC should not be the release build host.
- Kubernetes rollout should be GitOps-driven and human-approved before sync.

## What Not To Do

- Do not use Docker Hub automated builds as the product release pipeline.
- Do not deploy `:latest` or mutable tags.
- Do not put build secrets in Docker build args.
- Do not expose login handles or account identifiers as metrics labels.
- Do not turn PR load tests into stress tests.
- Do not run `kubectl apply`, `helm upgrade`, or Argo CD sync from product CI.

## Open Questions

- After the first GHCR image is published, the infrastructure PR must pin the production deployment by
  digest.
- Production migration job shape still needs a runbook before live user data.
- Password Vault dashboard can be rendered now, but real panel data cannot be verified until the app
  is deployed and scraped.

## Sources

See the official documentation list above and the implementation report for commands run.
