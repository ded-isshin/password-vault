ARG RUST_IMAGE=docker.io/library/rust:1.96.0-bookworm@sha256:13c186980fa33cc12759b429662a1322939dbe697484b7c33b47dd2698d28460
ARG RUNTIME_IMAGE=docker.io/library/debian:bookworm-slim@sha256:0104b334637a5f19aa9c983a91b54c89887c0984081f2068983107a6f6c21eeb
ARG BUILD_REVISION=unknown

FROM ${RUST_IMAGE} AS build
ARG BUILD_REVISION
WORKDIR /workspace
ENV PATH="/usr/local/cargo/bin:${PATH}"

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY migrations ./migrations

RUN PASSWORD_VAULT_BUILD_REVISION="${BUILD_REVISION}" \
    cargo build --locked --release --bin password-vault-api

FROM ${RUNTIME_IMAGE} AS runtime

LABEL org.opencontainers.image.title="Password Vault API" \
      org.opencontainers.image.description="API service for the Password Vault MVP" \
      org.opencontainers.image.source="https://github.com/ded-isshin/password-vault" \
      org.opencontainers.image.licenses="MIT"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 app \
    && useradd --system --uid 10001 --gid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin app

COPY --from=build /workspace/target/release/password-vault-api /usr/local/bin/password-vault-api

ENV PV_BIND_ADDR="0.0.0.0:8080" \
    PV_METRICS_BIND_ADDR="0.0.0.0:9090" \
    RUST_LOG="password_vault_api=info"

EXPOSE 8080
EXPOSE 9090
USER 10001:10001

ENTRYPOINT ["/usr/local/bin/password-vault-api"]
