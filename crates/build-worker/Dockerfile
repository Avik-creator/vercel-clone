FROM lukemathwalker/cargo-chef:latest-rust-bookworm AS chef
WORKDIR /app

# --- planner ---
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- rust builder ---
FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libgit2-dev cmake && \
    rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release -p vercel-clone-worker --recipe-path recipe.json

COPY . .
RUN cargo build --release --locked -p vercel-clone-worker

# --- railpack: install via official install script ---
FROM debian:bookworm-slim AS railpack-builder
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*
RUN curl -sSL https://railpack.com/install.sh | sh -s -- --bin-dir /usr/local/bin

# --- runtime ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 docker.io git && \
    rm -rf /var/lib/apt/lists/*

# Install buildctl for BuildKit communication
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    curl -fsSL https://github.com/moby/buildkit/releases/download/v0.15.0/buildkit-v0.15.0.linux-arm64.tar.gz -o /tmp/buildkit.tar.gz && \
    mkdir -p /tmp/buildkit && tar -xzf /tmp/buildkit.tar.gz -C /tmp/buildkit && \
    cp -r /tmp/buildkit/bin/* /usr/local/bin/ && \
    rm -rf /tmp/buildkit*

WORKDIR /app
COPY --from=builder /app/target/release/vercel-clone-worker .
COPY --from=railpack-builder /usr/local/bin/railpack /usr/local/bin/railpack

RUN useradd -r -u 1001 -s /sbin/nologin appuser && \
    usermod -aG docker appuser && \
    usermod -aG root appuser && \
    chown -R appuser:appuser /app

USER appuser
CMD ["./vercel-clone-worker"]
