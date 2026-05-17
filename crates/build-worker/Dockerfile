FROM rust:bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libgit2-dev cmake && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates/build-worker/Cargo.toml crates/build-worker/
COPY crates/build-worker/src crates/build-worker/src
COPY crates/api/Cargo.toml crates/api/
COPY crates/api/src crates/api/src

RUN cargo build --release -p vercel-clone-worker

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 docker.io && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/vercel-clone-worker /app/vercel-clone-worker

CMD ["/app/vercel-clone-worker"]
