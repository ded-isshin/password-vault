# syntax=docker/dockerfile:1.18

ARG RUST_VERSION=1.96.0
ARG BUILD_REVISION=unknown

FROM rust:${RUST_VERSION}-bookworm AS build
ARG BUILD_REVISION
WORKDIR /workspace
ENV PATH="/usr/local/cargo/bin:${PATH}"

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY migrations ./migrations

RUN PASSWORD_VAULT_BUILD_REVISION="${BUILD_REVISION}" \
    cargo build --locked --release --bin password-vault-api

FROM debian:bookworm-slim AS runtime

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
    RUST_LOG="password_vault_api=info"

EXPOSE 8080
USER 10001:10001

ENTRYPOINT ["/usr/local/bin/password-vault-api"]
