# Development Environment

Status: draft. Related issue: #26.

## Decision

The MVP uses a container/CI-based Rust build environment by default.

Do not install Rust on the mini-PC host as part of routine product work. A local Rust installation can
be considered later, but it is a host-level toolchain change and needs explicit approval before it is
performed.

## Current Local Tooling

Verified on 2026-06-07:

- `rustc`: not found
- `cargo`: not found
- `node`: available
- `npm`: available
- `docker`: available
- `kubectl` client: available
- `helm`: not found
- `argocd`: not found

This means Rust code cannot currently be built directly on the host shell.

## MVP Build Strategy

### Local Rust Commands

Use Docker with the Rust Docker Official Image for local Rust validation when product code exists.
The exact command will be finalized after the Rust workspace is created, but it should follow this
shape:

```bash
docker run --rm \
  -u "$(id -u):$(id -g)" \
  -v "$PWD:/workspace" \
  -w /workspace \
  rust:<pinned-version>-bookworm \
  sh -lc 'export PATH=/usr/local/cargo/bin:$PATH; cargo test --workspace'
```

Rules:

- Pin the Rust image version in docs and CI before implementation PRs depend on it.
- Do not use `latest`.
- Add `/usr/local/cargo/bin` to `PATH` inside the container command. The Rust Docker image contains
  the toolchain there, and the shell PATH may not include it by default in this environment.
- Do not mount host secrets into the container.
- Do not mount `/var/run/docker.sock` into product build containers.
- Do not run privileged containers.
- Keep build artifacts inside the repository workspace or an explicitly documented cache path.

### GitHub Actions

Use GitHub-hosted runners only.

Rust CI should run either:

- in a GitHub Actions job container using the pinned Rust Docker Official Image; or
- on the hosted runner if the runner image already provides the pinned Rust version and the workflow
  verifies `rustc --version`.

Preferred MVP path:

```yaml
jobs:
  rust:
    runs-on: ubuntu-latest
    container:
      image: rust:<pinned-version>-bookworm
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
      - run: |
          export PATH=/usr/local/cargo/bin:$PATH
          cargo test --workspace
```

This avoids third-party Rust setup actions for the first MVP baseline.

### Frontend Commands

Node and npm are available locally. Frontend dependencies and commands will be defined when the
React/Vite app is scaffolded.

Vite client environment variables are public build-time data. Do not place secrets in frontend
environment variables.

### PostgreSQL Version

The MVP CI migration job uses `postgres:17-bookworm`.

The current migration uses PostgreSQL column-specific `ON DELETE SET NULL (device_id)` on a composite
foreign key, so supported PostgreSQL versions must be PostgreSQL 15 or newer. Keep CI on PostgreSQL
17 unless a database-platform ADR explicitly changes the target.

### Kubernetes And GitOps Commands

Local `kubectl` is available, but it must not be used for cluster mutation without explicit human
approval.

`helm` and `argocd` are not installed locally. Helm chart validation can initially use:

- GitHub Actions with a container that contains Helm; or
- a future approved local tool install; or
- `docker run` with a trusted Helm image after review.

Do not run Argo CD sync or direct cluster mutation commands without explicit human approval.

## Official Documentation Checked

- Rust installation: <https://www.rust-lang.org/tools/install>
- Rust Docker Official Image: <https://hub.docker.com/_/rust>
- Docker Official Images:
  <https://docs.docker.com/docker-hub/repos/manage/trusted-content/official-images/>
- Docker Rust image guide: <https://docs.docker.com/guides/rust/build-images/>
- GitHub Actions running jobs in a container:
  <https://docs.github.com/en/actions/writing-workflows/choosing-where-your-workflow-runs/running-jobs-in-a-container>
- GitHub-hosted runners:
  <https://docs.github.com/en/actions/how-tos/using-github-hosted-runners/using-github-hosted-runners/about-github-hosted-runners>

## Security Considerations

- Containerized local builds avoid modifying the host toolchain.
- Docker image selection is a supply-chain decision; pin the image tag and consider digest pinning
  after the first scaffold.
- Public CI must use minimal permissions and must not expose secrets to pull requests.
- Build containers must not receive kubeconfigs, GitHub tokens, `.env` files with real secrets, SSH
  keys, or host service sockets.

## Follow-Up Work

- #14 added the first Rust workspace and test commands.
- #15 adds SQLx migrations. Local migration tests need a disposable PostgreSQL database and
  `PV_TEST_DATABASE_URL`; routine `cargo test --workspace` skips the migration integration test when
  that variable is absent.
  The CI migration job uses a PostgreSQL service container and the dummy URL
  `postgres://postgres:postgres@postgres:5432/password_vault_test`.
  CI uses `cargo fetch --locked`, `cargo clippy --locked`, and `cargo test --locked` so the
  Rust 1.85-compatible lockfile cannot silently drift.
- #20 must add the final CI workflow.
- #24 must run its OPAQUE proof-of-concept in this selected build environment.
- #21 must define Helm validation once a chart exists.
