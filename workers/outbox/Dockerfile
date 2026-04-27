# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS builder

WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends build-essential ca-certificates clang cmake git libclang-dev libprotobuf-dev ninja-build perl protobuf-compiler pkg-config && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY proto ./proto
COPY services ./services
COPY workers ./workers

RUN cargo build --locked --release --package outbox

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libstdc++6 && rm -rf /var/lib/apt/lists/* && groupadd --system relay && useradd --system --gid relay --home-dir /nonexistent --shell /usr/sbin/nologin relay
COPY --from=builder /app/target/release/outbox /usr/local/bin/outbox
USER relay
ENTRYPOINT ["/usr/local/bin/outbox"]
