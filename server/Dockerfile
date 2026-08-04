# Multi-stage build for the tengri-server binary (plus the tengri/leonardo
# CLIs). Build context is `server/`. Target linux/arm64 (Apple Silicon builder
# -> Asahi arm64 host). glibc Debian runtime deliberately avoids musl's 16K-page
# edge cases; no jemalloc/mimalloc in the dep tree, so the system allocator is
# page-size agnostic.

FROM rust:1-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin tengri-server --bin tengri --bin leonardo

FROM debian:bookworm-slim AS runtime
# `curl` backs the container HealthCmd (`curl -fsS .../health`) used by the
# Quadlet; `ca-certificates` covers outbound TLS.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/tengri-server /usr/local/bin/tengri-server
COPY --from=builder /app/target/release/tengri        /usr/local/bin/tengri
COPY --from=builder /app/target/release/leonardo      /usr/local/bin/leonardo
# The leonardo CLI reads its JSON inputs from `<CARGO_MANIFEST_DIR>/data`, which
# bakes to `/app/data` at compile time. Ship the files there (read as files, so
# they can be bind-mounted over to edit without a rebuild).
COPY data /app/data
ENV SERVER_ADDR=0.0.0.0:3000
EXPOSE 3000
ENTRYPOINT ["tengri-server"]
