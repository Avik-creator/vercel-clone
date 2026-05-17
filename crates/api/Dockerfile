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

# --- runtime: minimal image, no Rust toolchain ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/vercel-clone-api .
COPY --from=builder /app/crates/api/migrations ./migrations

EXPOSE 3000
CMD ["./vercel-clone-api"]
