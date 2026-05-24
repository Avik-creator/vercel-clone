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

# --- nixpacks: install from GitHub releases ---
FROM debian:bookworm-slim AS nixpacks-builder
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*
RUN curl -sSL https://github.com/railwayapp/nixpacks/releases/download/v1.41.0/nixpacks-v1.41.0-aarch64-unknown-linux-musl.tar.gz -o /tmp/nixpacks.tar.gz && \
    tar -xzf /tmp/nixpacks.tar.gz -C /usr/local/bin && \
    rm /tmp/nixpacks.tar.gz

# --- runtime ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 git gosu && \
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
COPY --from=nixpacks-builder /usr/local/bin/nixpacks /usr/local/bin/nixpacks
COPY infra/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh && \
    mkdir -p /home/appuser && \
    useradd -r -u 1001 -d /home/appuser -s /sbin/nologin appuser && \
    chown appuser:appuser /home/appuser && \
    chown -R appuser:appuser /app

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["./vercel-clone-worker"]
