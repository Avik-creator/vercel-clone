FROM lukemathwalker/cargo-chef:latest-rust-bookworm AS chef
WORKDIR /app

# --- planner: generate a build recipe from Cargo.toml/Cargo.lock ---
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- builder: compile deps first (cached), then the binary ---
FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libgit2-dev cmake && \
    rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
# This layer is cached as long as Cargo.toml / Cargo.lock are unchanged.
RUN cargo chef cook --release -p vercel-clone-worker --recipe-path recipe.json

COPY . .
RUN cargo build --release --locked -p vercel-clone-worker
RUN cargo install railpack --locked

# --- runtime: minimal image, no Rust toolchain ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 docker.io git && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/vercel-clone-worker .
COPY --from=builder /usr/local/cargo/bin/railpack /usr/local/bin/railpack

RUN useradd -r -u 1001 -s /sbin/nologin appuser && \
    chown -R appuser:appuser /app

USER appuser
CMD ["./vercel-clone-worker"]
