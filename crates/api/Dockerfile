FROM lukemathwalker/cargo-chef:latest-rust-bookworm AS chef
WORKDIR /app

# --- planner: generate a build recipe from Cargo.toml/Cargo.lock ---
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- builder: compile deps first (cached), then the binary ---
FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
# This layer is cached as long as Cargo.toml / Cargo.lock are unchanged.
RUN cargo chef cook --release -p vercel-clone-api --recipe-path recipe.json

COPY . .
RUN cargo build --release --locked -p vercel-clone-api

# --- runtime: minimal image with docker CLI to spawn deployment containers ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl gnupg lsb-release gosu && \
    install -m 0755 -d /etc/apt/keyrings && \
    curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc && \
    chmod a+r /etc/apt/keyrings/docker.asc && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian bookworm stable" \
        > /etc/apt/sources.list.d/docker.list && \
    apt-get update && apt-get install -y --no-install-recommends docker-ce-cli && \
    apt-get purge -y curl gnupg lsb-release && \
    apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/vercel-clone-api .
COPY --from=builder /app/crates/api/migrations ./migrations
COPY infra/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh && \
    mkdir -p /home/appuser && \
    useradd -r -u 1001 -d /home/appuser -s /sbin/nologin appuser && \
    chown appuser:appuser /home/appuser && \
    chown -R appuser:appuser /app

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["./vercel-clone-api"]
