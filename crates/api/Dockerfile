FROM rust:slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates/api/Cargo.toml crates/api/
COPY crates/api/src crates/api/src
COPY crates/api/migrations crates/api/migrations
COPY crates/build-worker/Cargo.toml crates/build-worker/
COPY crates/build-worker/src crates/build-worker/src

RUN cargo build --release -p vercel-clone-api

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/vercel-clone-api /app/vercel-clone-api

EXPOSE 3000

CMD ["/app/vercel-clone-api"]
