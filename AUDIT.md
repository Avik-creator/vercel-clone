# Vercel Clone — Production Audit

> **Date:** 2026-05-17  
> **Scope:** Full system — build pipeline, serving architecture, infrastructure, security, observability  
> **Status:** Development-grade. Not production-ready.

---

## Table of Contents

1. [Current Architecture Overview](#1-current-architecture-overview)
2. [Build System Audit](#2-build-system-audit)
3. [Railpack — Proposed Build Strategy](#3-railpack--proposed-build-strategy)
4. [Serving Architecture — All Runtime Scenarios](#4-serving-architecture--all-runtime-scenarios)
5. [Asset Serving — The MinIO Hot-Path Problem](#5-asset-serving--the-minio-hot-path-problem)
6. [Container Architecture — Build vs Serve Separation](#6-container-architecture--build-vs-serve-separation)
7. [Infrastructure Audit](#7-infrastructure-audit)
8. [Security Audit](#8-security-audit)
9. [Observability Audit](#9-observability-audit)
10. [Resilience & Error Handling](#10-resilience--error-handling)
11. [Proposed Target Architecture](#11-proposed-target-architecture)
12. [Migration Roadmap](#12-migration-roadmap)

---

## 1. Current Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Single VM                            │
│                                                             │
│  Traefik :80                                                │
│     │                                                       │
│     ├── api.localhost   ──► API :8080                       │
│     └── *.localhost     ──► API :8080 (serves all)         │
│                                  │                          │
│                    ┌─────────────┤                          │
│                    │             │                          │
│                  Postgres      NATS JetStream               │
│                  MinIO         (build_jobs stream)          │
│                    │             │                          │
│                    └─────────────┤                          │
│                                  │                          │
│                          Worker (1-N instances)             │
│                          build-{id} containers              │
│                          serve-{id} containers              │
└─────────────────────────────────────────────────────────────┘
```

### What exists and works

| Component | Status |
|-----------|--------|
| NATS JetStream job queue | Working |
| Build worker with semaphore concurrency | Working (just added) |
| MinIO artifact storage | Working |
| PostgreSQL with migrations | Working |
| Traefik reverse proxy | Working |
| Real-time SSE log streaming | Working |
| GitHub OAuth + App integration | Working |
| JWT + API key authentication | Working |
| Deployment cancellation / promotion | Working |
| Docker-based build execution | Working |

### What is fundamentally broken or missing

- Build system only supports Node.js and Rust (2 of ~20 common runtimes)
- API is in the hot path for every static asset request (no direct routing)
- Build containers and serve containers share the same Docker network as internal services
- All containers run as root with no resource limits
- No HTTPS anywhere
- All credentials hardcoded to defaults

---

## 2. Build System Audit

### Current implementation (`builder.rs`)

```rust
async fn detect_runtime(job: &BuildJob) -> &'static str {
    if job.build_command.as_ref().map_or(false, |c| c.contains("cargo")) {
        "rust:slim"
    } else {
        "node:22-alpine"   // everything else defaults to Node
    }
}
```

**This is the entire runtime detection logic.** If the build command contains "cargo" → Rust. Otherwise → Node. That's it.

### What happens for unsupported runtimes

| Framework / Language | User's build_command | What actually happens |
|---------------------|---------------------|----------------------|
| Python (FastAPI, Django, Flask) | `pip install -r requirements.txt` | Runs in `node:22-alpine` — fails immediately |
| Go | `go build ./...` | Runs in `node:22-alpine` — `go` not found, exit 127 |
| Ruby on Rails | `bundle exec rake assets:precompile` | Runs in `node:22-alpine` — fails |
| PHP (Laravel) | `composer install` | Fails |
| Java / Spring Boot | `mvn package` | Fails |
| Deno | `deno compile` | Fails |
| Bun | `bun run build` | Fails (unless Node happens to work) |
| Nuxt (Vue SSR) | `nuxt build` | Works (Node), but output detection may fail |
| SvelteKit | `vite build` | Works (Node), but serves incorrectly |
| Remix | `remix build` | Works (Node), but serves incorrectly |
| Astro | `astro build` | Works (Node), output goes to `dist/` |

### Output type detection gaps (`main.rs detect_output_type`)

After a build, the worker probes for known output directories:

```rust
// Probes in order: .next/standalone, then out/, build/, dist/, .next/
```

Problems:

| Framework | Actual output | Detected as |
|-----------|--------------|-------------|
| Remix | `build/` | Static — **wrong**, Remix is SSR |
| Nuxt | `.output/` | Not detected — falls back to `dist/` which doesn't exist |
| SvelteKit (Node adapter) | `build/` | Static — **wrong**, SvelteKit SSR needs a node process |
| Astro (static) | `dist/` | Static — correct |
| Astro (SSR) | `dist/` + server | Static — **wrong** |
| Python FastAPI | N/A | N/A — never gets this far |
| Go binary | `./myapp` | Nothing detected |
| Vite | `dist/` | Static — correct |
| Create React App | `build/` | Static — correct |
| Angular | `dist/myapp/` | Not detected unless output_dir is set |

### The root problem

The build system assumes every project is either:
- A Node.js app that outputs to a known static directory, or
- A Next.js app that outputs `.next/standalone/`

Everything else silently produces the wrong behaviour or fails.

---

## 3. Railpack — Proposed Build Strategy

[Railpack](https://github.com/railwayapp/railpack) is an open-source build tool from Railway that auto-detects language and framework, then generates an optimized Dockerfile. It replaces the entire manual `detect_runtime` / `deps_cmd` / `node_install_cmd` logic.

### What Railpack detects automatically

| Runtime | Detection method |
|---------|-----------------|
| Node.js | `package.json` present |
| Python | `requirements.txt`, `pyproject.toml`, `Pipfile` |
| Go | `go.mod` present |
| Rust | `Cargo.toml` present |
| Ruby | `Gemfile` present |
| PHP | `composer.json` present |
| Java | `pom.xml`, `build.gradle` |
| Deno | `deno.json` present |
| Bun | `bun.lockb` present |
| Static (nginx) | No server runtime detected |

### Framework detection within Node.js

| Framework | Detection | Output type |
|-----------|-----------|-------------|
| Next.js (standalone) | `next.config.*` + `output: 'standalone'` | SSR Node container |
| Next.js (static export) | `next.config.*` + `output: 'export'` | Static nginx |
| Remix | `remix.config.js` | SSR Node container |
| Nuxt | `nuxt.config.*` | SSR Node container |
| SvelteKit (node adapter) | `svelte.config.js` + `@sveltejs/adapter-node` | SSR Node container |
| SvelteKit (static adapter) | `svelte.config.js` + `@sveltejs/adapter-static` | Static nginx |
| Astro (SSR) | `astro.config.*` + `output: 'server'` | SSR Node container |
| Astro (static) | `astro.config.*` + `output: 'static'` | Static nginx |
| Vite / CRA / Angular | No SSR config | Static nginx |

### How Railpack changes the build worker

**Current flow:**
```
clone → detect runtime (2 options) → run build_command → detect output (6 dirs) → upload to MinIO
```

**With Railpack:**
```
clone → railpack generate-dockerfile → docker build → push to local registry → serve container = the built image
```

The worker no longer needs to:
- Detect the runtime manually
- Install deps (Railpack does it in the Dockerfile)
- Detect output directories
- Distinguish static vs SSR for the artifact upload

Instead:
1. Clone the repo
2. Run `railpack generate` to produce a `Dockerfile`
3. `docker build -t registry.internal/deployment-{id}:latest .`
4. Push to a local Docker registry (already running in docker-compose)
5. The serve step is just `docker run registry.internal/deployment-{id}:latest`

### What Railpack's generated container looks like

```
┌─────────────────────────────────────┐
│  Railpack-generated image           │
│                                     │
│  - Multi-stage build                │
│  - Builder stage: full toolchain    │
│  - Runtime stage: minimal base      │
│    (node:alpine, python:slim,        │
│     scratch for Go/Rust, etc.)      │
│  - Runs as non-root                 │
│  - Listens on $PORT (default 3000)  │
│  - Static output → nginx:alpine     │
└─────────────────────────────────────┘
```

### New worker flow in code terms

```rust
// Replace all of builder.rs with:
async fn run_build(job: &BuildJob, ...) -> anyhow::Result<String> {
    // 1. Clone
    git_clone(&job.git_url, &job.commit_sha, &work_dir).await?;

    // 2. Generate Dockerfile via Railpack CLI
    Command::new("railpack")
        .args(["generate", "--output", "Dockerfile.railpack"])
        .current_dir(&work_dir)
        .status().await?;

    // 3. Build image
    let image_tag = format!("localhost:5000/deployment-{}:latest", job.deployment_id);
    Command::new("docker")
        .args(["build", "-f", "Dockerfile.railpack", "-t", &image_tag, "."])
        .current_dir(&work_dir)
        .status().await?;

    // 4. Push to local registry
    Command::new("docker").args(["push", &image_tag]).status().await?;

    // 5. Return image reference instead of MinIO artifact key
    Ok(image_tag)
}
```

MinIO still stores static assets for direct serving (see §5), but the primary artifact becomes a Docker image in a local registry.

---

## 4. Serving Architecture — All Runtime Scenarios

### Current state (one path for everything)

```
Any request → API serve_artifact() → MinIO GET → buffer → response
                     │ (if not in MinIO)
                     └→ start Node container → proxy
```

This is wrong for most frameworks and has no concept of different runtime requirements.

### Required serving architecture per scenario

---

#### 4.1 Static Sites
*React (Vite/CRA), Vue (Vite), Angular, Svelte (static adapter), Astro (static), Next.js (export mode)*

**What they need:** HTTP file server. No runtime process.

**Serve with:** `nginx:alpine`

```
Traefik ──► nginx:alpine container
              └── /usr/share/nginx/html (volume-mounted from local disk)
```

```nginx
# auto-generated nginx.conf
server {
    listen 3000;
    root /usr/share/nginx/html;
    index index.html;

    # Long-lived cache for hashed assets
    location ~* \.(js|css|png|jpg|woff2|svg)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # SPA routing — fall back to index.html
    location / {
        try_files $uri $uri/ /index.html;
    }

    gzip on;
    gzip_types text/plain text/css application/javascript application/json;
}
```

**Cold start:** ~200ms. Memory: ~5MB. Zero CPU at idle.

---

#### 4.2 Next.js SSR (standalone mode)

**What it needs:** Node.js process for SSR + static file server for `/_next/static/*`

**Serve with:** nginx (static) + node (dynamic) in one container

```
Traefik ──► nginx:alpine
              ├── /_next/static/*  →  local .next/static/ (disk, no node involved)
              ├── /public/*        →  local public/ (disk)
              └── /*               →  proxy → node server.js :3000
```

```nginx
server {
    listen 80;

    location /_next/static/ {
        alias /app/.next/static/;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    location /public/ {
        alias /app/public/;
        expires 7d;
    }

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

**Startup:** nginx starts instantly. Node warms up in background. nginx returns 502 briefly then proxies once node is ready (much better UX than the current 5-retry loop in the API).

---

#### 4.3 Remix

**What it needs:** Node.js SSR process (Remix serves everything including assets)

**Output structure:**
```
build/
  server/       ← server-side modules
  client/       ← static assets (JS, CSS)
public/         ← static assets that don't go through Vite
```

**Serve with:** `node:22-alpine` running `node ./build/server/index.js`

```
Traefik ──► nginx
              ├── /build/client/*  →  local disk (hashed, immutable)
              ├── /public/*        →  local disk
              └── /*               →  proxy → remix server :3000
```

Railpack detects Remix automatically and configures this correctly.

---

#### 4.4 Nuxt.js (Vue SSR)

**Output:** `.output/` directory with:
```
.output/
  server/       ← nitro server bundle
  public/       ← static assets
```

**Serve with:** `node:22-alpine` running `node .output/server/index.mjs`

```
Traefik ──► nginx
              ├── /_nuxt/*    →  local .output/public/_nuxt/ (immutable)
              ├── /public/*   →  local .output/public/
              └── /*          →  proxy → nuxt server :3000
```

**Current bug:** The worker tries `out/`, `build/`, `dist/`, `.next/` in sequence. `.output/` is never probed, so Nuxt builds currently produce an error or serve nothing.

---

#### 4.5 SvelteKit

Two distinct modes depending on the adapter configured:

**Static adapter** (`@sveltejs/adapter-static`):
- Output: `build/` directory of pure HTML/JS/CSS
- Serve with: nginx (same as §4.1)

**Node adapter** (`@sveltejs/adapter-node`):
- Output: `build/` directory with `index.js` server entry
- Serve with: `node:22-alpine` running `node build/index.js`
- **Current bug:** Both modes output to `build/` — the worker cannot distinguish them and always serves as static, breaking SSR deployments

---

#### 4.6 Python — FastAPI / Flask / Django

**What it needs:** Python runtime + ASGI/WSGI server

**Railpack generates:**
```dockerfile
FROM python:3.12-slim
RUN pip install gunicorn uvicorn
CMD ["gunicorn", "-k", "uvicorn.workers.UvicornWorker", "app.main:app", "--bind", "0.0.0.0:3000"]
```

**Django static files:**
- `python manage.py collectstatic` runs at build time
- Output goes to `staticfiles/`
- Serve with: nginx for `/static/*` → proxy uvicorn for everything else

```
Traefik ──► nginx
              ├── /static/*  →  local staticfiles/ (disk)
              ├── /media/*   →  local media/ (disk, if present)
              └── /*         →  proxy → uvicorn :3000
```

**Current state:** These projects cannot build at all — the worker runs the pip command inside `node:22-alpine` which has no Python.

---

#### 4.7 Go

**What it needs:** A single compiled binary. That's it.

**Railpack generates:**
```dockerfile
FROM golang:1.22 AS builder
RUN go build -o /app ./...

FROM gcr.io/distroless/static
COPY --from=builder /app /app
CMD ["/app"]
```

**Serve with:** The compiled binary itself, listening on `$PORT`

```
Traefik ──► Go binary container
```

**Cold start:** <100ms. Memory: ~10-20MB. No language runtime overhead.

---

#### 4.8 Rust

**Same pattern as Go** — compile to binary, run in minimal container.

**Railpack generates:**
```dockerfile
FROM rust:1.78 AS builder
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /target/release/myapp /app
CMD ["/app"]
```

**Current state:** Rust builds technically work (the cargo detection exists) but there's no serve container logic for them — `deployment_servers.rs` only handles Next.js standalone. A Rust binary that built successfully would never get served.

---

#### 4.9 Ruby on Rails

**What it needs:** Ruby runtime + Puma web server + asset pipeline

**Railpack generates:**
```dockerfile
FROM ruby:3.3-slim AS builder
RUN bundle install
RUN bundle exec rake assets:precompile

FROM ruby:3.3-slim
CMD ["bundle", "exec", "puma", "-C", "config/puma.rb"]
```

**Serve with:**
```
Traefik ──► nginx
              ├── /assets/*   →  local public/assets/ (precompiled, immutable)
              ├── /packs/*    →  local public/packs/
              └── /*          →  proxy → puma :3000
```

---

#### 4.10 PHP (Laravel / Symfony)

**What it needs:** PHP-FPM + nginx

**Railpack generates:**
```dockerfile
FROM php:8.3-fpm-alpine
RUN composer install --no-dev
```

**Serve with:** nginx + PHP-FPM (two processes in one container via supervisord or s6-overlay)

```
Traefik ──► nginx
              ├── /build/*   →  local public/build/ (Vite assets)
              └── /*.php     →  PHP-FPM socket
```

---

#### 4.11 Java / Spring Boot

**What it needs:** JVM + fat JAR

**Railpack generates:**
```dockerfile
FROM maven:3.9-eclipse-temurin-21 AS builder
RUN mvn package -DskipTests

FROM eclipse-temurin:21-jre-alpine
COPY --from=builder target/*.jar app.jar
CMD ["java", "-jar", "app.jar"]
```

**Serve with:** The JAR itself exposes HTTP (Spring Boot embeds Tomcat/Undertow)

```
Traefik ──► Spring Boot container :8080
```

**Cold start:** 5-15 seconds. This needs the readiness probe in `deployment_servers.rs` extended to support configurable startup timeouts (currently hardcoded 30s which may be insufficient for JVM cold starts).

---

#### 4.12 Deno / Bun

**Bun:**
```dockerfile
FROM oven/bun:1
COPY . .
RUN bun install --frozen-lockfile
CMD ["bun", "run", "start"]
```

**Deno:**
```dockerfile
FROM denoland/deno:2.0
COPY . .
RUN deno cache main.ts
CMD ["deno", "run", "--allow-net", "--allow-read", "main.ts"]
```

---

### Summary: Serving strategy per output type

| Output Type | Serve Container | Who handles static | Who handles dynamic |
|-------------|----------------|-------------------|---------------------|
| Pure static | nginx:alpine | nginx (local disk) | N/A |
| Next.js standalone | nginx + node | nginx | node server.js |
| Remix | nginx + node | nginx | remix server |
| Nuxt | nginx + node | nginx | nitro server |
| SvelteKit (node) | nginx + node | nginx | node build/index.js |
| Python (FastAPI/Flask) | nginx + uvicorn | nginx | uvicorn |
| Django | nginx + gunicorn | nginx | gunicorn |
| Go / Rust | binary only | binary | binary |
| Ruby on Rails | nginx + puma | nginx | puma |
| PHP | nginx + php-fpm | nginx | php-fpm |
| Java | JVM jar | jar | jar |
| Bun / Deno | runtime | runtime | runtime |

---

## 5. Asset Serving — The MinIO Hot-Path Problem

### Current flow (broken for production)

```
Browser request for /logo.png
        │
        ▼
   API serve_artifact()
        │
        ├─ DB query: SELECT deployment WHERE url = host
        │
        ├─ MinIO GET /deployments/{id}/logo.png
        │      ← network call to MinIO on every request
        │
        ├─ object.body.collect().await   ← entire file buffered in API heap
        │
        └─ Response
```

**Problems with this:**
- Every asset request = 1 DB query + 1 MinIO network call + full memory buffer
- `/_next/static/` files are in MinIO AND on local disk (standalone download) — we hit MinIO anyway
- No `Cache-Control` headers — browser fetches every asset on every navigation
- No `ETag` / `Last-Modified` — no 304 Not Modified responses
- No streaming — a 5MB image is fully buffered in API memory before any bytes reach the browser
- No compression — served raw, no gzip/brotli
- No range request support — video/audio files cannot seek

### Proposed flow

```
Browser request for /logo.png
        │
        ▼
   Traefik
        │
        └─► serve-{id} container (nginx or runtime)
                │
                └─ serve from local disk (downloaded once at deploy time)
                        ← no API involved
                        ← no MinIO on hot path
```

MinIO's role changes to **cold storage only**:
- Written once by the build worker after a successful build
- Read once by the serve container manager when starting the container (download to local disk)
- Never touched again on the request path

### What needs to change

1. Traefik routes `{deployment-subdomain}.*` directly to serve containers, not to the API
2. `serve_artifact()` in `routes/deployments.rs` is removed entirely
3. `deployment_servers.rs` downloads all assets to disk when starting the container (already does this for standalone — needs extending to static sites)
4. nginx inside the serve container adds proper cache headers

---

## 6. Container Architecture — Build vs Serve Separation

### Current: everything shares one network

```
docker network: vercel-clone_default
├── traefik
├── api
├── postgres
├── nats
├── minio
├── build-{id}  ← arbitrary user code, can reach postgres/nats/minio
└── serve-{id}  ← user app, can reach postgres/nats/minio
```

A malicious repo's build script can `curl http://minio:9000` and exfiltrate all deployment artifacts, or `curl http://nats:4222` to publish fake build results.

### Proposed: three separate networks

```
network: internal          network: build-net         network: serve-net
├── api ◄──────────────────────────────────────────────────────────────┐
├── postgres                                                            │
├── nats        ◄── build-{id}                serve-{id} ─────────────┤
├── minio            (can reach minio                (Traefik-facing)  │
└── traefik ────────── for artifact upload,     traefik ───────────────┘
                        nothing else)
```

**Build containers:**
- Network: `build-net` (internet access for git clone + `minio:9000` only)
- `--cap-drop=ALL` — no Linux capabilities
- `--security-opt=no-new-privileges`
- `--cpus=2 --memory=4g` — bounded resources
- `--read-only` root filesystem except `/tmp` and `/app`
- Killed immediately after build completes

**Serve containers:**
- Network: `serve-net` (Traefik-facing only, no internal services)
- `--cpus=0.5 --memory=512m` — lightweight
- `--read-only` root filesystem (static files already on mounted volume)
- Killed after idle timeout (currently 5 minutes)

---

## 7. Infrastructure Audit

### Secrets

| Location | Current value | Required action |
|----------|--------------|-----------------|
| `docker-compose.yml` Postgres password | `postgres` | Generate random 32-char string, use Docker secret or `.env` |
| `docker-compose.yml` MinIO credentials | `minioadmin:minioadmin` | Generate random credentials |
| NATS | No auth | Add `--user`/`--pass` flags to NATS container |
| JWT secret | Hardcoded in `.env` | Rotate; use a secrets manager in prod |
| GitHub App private key | Placeholder in `.env` | Real key must never be committed |
| BUILD_WORKER_SECRET | Hardcoded in `.env` | Rotate |

All of these are in the git history if they were ever committed. Rotation alone is not enough — the repo history needs auditing.

### TLS

**Nothing is encrypted in transit.** Required changes:

```yaml
# traefik: add to docker-compose.yml
command:
  - --certificatesresolvers.letsencrypt.acme.email=you@example.com
  - --certificatesresolvers.letsencrypt.acme.storage=/acme.json
  - --certificatesresolvers.letsencrypt.acme.tlschallenge=true
  - --entrypoints.websecure.address=:443
  - --entrypoints.web.http.redirections.entrypoint.to=websecure
```

For NATS internal TLS (between API/worker and NATS):
```
nats --tls --tlscert=/certs/nats.crt --tlskey=/certs/nats.key
```

For Postgres SSL:
```
DATABASE_URL=postgres://...@db:5432/vercel_clone?sslmode=require
```

### Resource limits

None of the containers have limits. A single runaway build can OOM the entire VM, taking down Postgres, NATS, and all serving containers.

Recommended limits:

| Service | CPU | Memory |
|---------|-----|--------|
| Traefik | 0.5 | 256MB |
| Postgres | 2.0 | 2GB |
| NATS | 0.5 | 512MB |
| MinIO | 1.0 | 1GB |
| API | 1.0 | 512MB |
| Worker | 0.5 | 256MB |
| Build container | 2.0 | 4GB |
| Serve container (static) | 0.2 | 64MB |
| Serve container (SSR/API) | 0.5 | 512MB |

```yaml
# docker-compose.yml pattern
deploy:
  resources:
    limits:
      cpus: '2.0'
      memory: 4g
```

### Health checks

| Service | Current | Required |
|---------|---------|----------|
| Postgres | `pg_isready` | Done |
| NATS | None | `curl http://nats:8222/healthz` |
| MinIO | None | `curl http://minio:9000/minio/health/live` |
| API | `/health` endpoint exists | Wire into docker-compose |
| Worker | None | Liveness file touch (`/tmp/worker-alive`) |
| Serve containers | 30s HTTP poll (in code) | Extend timeout per runtime (JVM needs 60s+) |

### Volumes and persistence

| Data | Current | Risk | Fix |
|------|---------|------|-----|
| Postgres | Named volume | Disk failure = total data loss | Postgres streaming replication OR daily `pg_dump` to S3 |
| MinIO | Named volume | Disk failure = all artifacts lost | MinIO distributed mode OR replicate to real S3 |
| NATS | Named volume | Queue data loss on disk failure | NATS clustering (3 nodes) or accept replayability |
| Serve container files | `/tmp` on API host | Wiped on restart | Acceptable — re-downloads from MinIO on restart |
| Build temp files | `worker_tmp` volume | Shared between worker instances | Acceptable — UUID-namespaced |

### Postgres connection pool

Current: `max_connections = 20`, hardcoded in `db.rs`. In production with multiple API replicas this will exhaust Postgres connections.

Fix: Add PgBouncer in `transaction` mode between the API and Postgres. Postgres can handle fewer real connections while the API pool sees unlimited logical connections.

---

## 8. Security Audit

### Docker socket exposure

Both the API and worker mount `/var/run/docker.sock`. This is equivalent to giving root access to the host. Anyone who can exec into the API container owns the host.

**Short-term:** Accept the risk for single-VM dev deployments but document it.

**Production:** Replace with one of:
- **Docker-in-Docker (dind)** sidecar per worker — build containers are isolated inside dind
- **Podman rootless** — builds run without host root
- **Kaniko** — builds container images without Docker daemon access
- **Sysbox** — system container that sandboxes Docker

### Build container escape

Build containers run arbitrary user code from GitHub repos. Current risks:

| Risk | Current state | Fix |
|------|--------------|-----|
| Access internal services | Build container on main network | Separate `build-net` (see §6) |
| Read host filesystem | Volume mounts only to `/tmp/builds` | Already scoped; add `--read-only` |
| Fork bomb / resource exhaustion | No limits | `--cpus --memory --pids-limit=512` |
| Crypto mining | No CPU limits | CPU limits + build timeout (already have 600s) |
| Exfiltrate secrets via env | Env vars injected by user | Expected; document that env vars in builds are user-controlled |

### Authentication gaps

| Gap | Impact | Fix |
|-----|--------|-----|
| No JWT blacklist / revocation | Stolen token valid until expiry | Add Redis-backed token blacklist on logout |
| API keys have no scopes | Any key = full account access | Add `read:deployments`, `write:deployments` scopes |
| BUILD_WORKER_SECRET is static | Rotation requires redeploy | Issue short-lived tokens via NATS auth callout |
| GitHub OAuth scope is broad | Access to all repos | Request minimal scopes; use installation tokens scoped per repo |
| No audit log | Can't trace who did what | Add `audit_events` table (actor, action, resource, ip, timestamp) |

### Dockerfile: running as root

```dockerfile
# Current: no USER directive = runs as root
CMD ["./vercel-clone-api"]

# Fix:
RUN useradd -r -u 1001 appuser
USER appuser
CMD ["./vercel-clone-api"]
```

---

## 9. Observability Audit

### Logging

| Component | Current | Problem | Fix |
|-----------|---------|---------|-----|
| API | `tracing` to stdout | Unstructured text | `tracing-subscriber` with JSON format + ship to Loki |
| Worker | `tracing` to stdout | Same | Same |
| Build containers | Lines streamed to NATS | Lost on API crash before flush | Persist incrementally to object storage |
| Serve containers | Nothing | No visibility into serving errors | nginx access log → stdout → Loki |
| NATS | stdout | No retention | Forward to log aggregator |

Recommended log stack (lightweight):
```yaml
# Add to docker-compose.yml
loki:
  image: grafana/loki:3.0
  ports: ["3100:3100"]

grafana:
  image: grafana/grafana:11.0
  ports: ["3000:3000"]

alloy:  # Grafana Alloy (replaces Promtail)
  image: grafana/alloy:latest
  volumes:
    - /var/run/docker.sock:/var/run/docker.sock:ro
  # scrapes Docker container logs and ships to Loki
```

### Metrics

Nothing exports metrics today. No way to know:
- How many builds are queued / running / failing
- P50/P95 build duration
- How many serve containers are running
- Memory usage of serve containers
- Request rate and latency per deployment

Fix: Add Prometheus + Grafana:

```yaml
# Expose from API (axum-prometheus crate)
GET /metrics  → Prometheus scrape endpoint
```

Metrics to expose:
- `build_jobs_queued_total`
- `build_jobs_running`
- `build_duration_seconds` (histogram)
- `serve_containers_running`
- `http_request_duration_seconds` (per deployment)
- `minio_upload_bytes_total`

### Alerting

Zero alerting today. Minimum alerts for production:

| Alert | Condition |
|-------|-----------|
| Build queue depth | > 10 jobs queued for > 5 min |
| Build failure rate | > 20% of builds failing |
| API error rate | > 1% 5xx responses |
| NATS result consumer lag | > 50 unprocessed messages |
| Serve container OOM | Container exit code 137 |
| Disk usage | > 80% on MinIO volume |

### Distributed tracing

No trace IDs propagated between API → NATS → Worker → NATS → API. When a deployment fails it's impossible to correlate what happened across components.

Fix: Add `opentelemetry` crate and propagate trace context in NATS message headers. Ship traces to Jaeger or Grafana Tempo.

---

## 10. Resilience & Error Handling

### Background task supervision

```rust
// Current (main.rs) — fire and forget:
tokio::spawn(subscribe_build_results(state.clone(), nats.clone()));
tokio::spawn(subscribe_build_logs(state.clone(), nats.clone()));
```

If either task panics, it silently stops. The API continues serving requests but never updates deployment state again — deployments get stuck in `building` forever with no indication to the user.

Fix: Wrap in a supervisor that restarts on panic:

```rust
async fn supervised<F, Fut>(name: &'static str, mut factory: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    loop {
        let result = factory().await;
        tracing::error!(task = name, ?result, "task exited, restarting in 5s");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

### NATS dead-letter queue

Current: `max_deliver: 3` for build jobs (worker), `max_deliver: 5` for results (API). After exhaustion, messages are dropped silently.

Fix: Add a dead-letter stream:

```rust
// In nats.rs ensure_stream() calls — add:
ensure_stream(&context, "build_jobs_dlq", vec!["build.jobs.dlq.>"]).await?;
// Configure NATS consumer with:
nack_backoff: vec![5s, 30s, 120s],  // exponential backoff
deliver_subject: "build.jobs.dlq.>",  // after max_deliver, route here
```

Then add an API route `GET /v1/admin/failed-jobs` to inspect and replay DLQ messages.

### Graceful shutdown

Neither the API nor worker handles `SIGTERM`. Docker sends `SIGTERM` then `SIGKILL` after 10 seconds. Currently:
- In-progress build containers keep running (worker is gone, they're orphaned)
- In-flight HTTP requests are cut mid-response
- NATS messages being processed are re-queued and reprocessed

Fix:

```rust
// API
let shutdown = async {
    tokio::signal::unix::signal(SignalKind::terminate())?.recv().await;
};
axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;

// Worker
tokio::select! {
    _ = signal::ctrl_c() => { semaphore.close(); /* drain in-flight jobs */ }
    Some(job) = jobs.next() => { /* process */ }
}
```

### Build log persistence

Build logs are buffered in a `HashMap<Uuid, Vec<String>>` in the NATS service on the API. If the API restarts mid-build, all logs are lost. The deployment gets stuck — the worker finishes and publishes a result but the log buffer is gone.

Fix: Write log lines incrementally to Postgres (`build_log_lines` table) as they arrive, not as a single bulk update at the end.

### Artifact size limit

Nothing prevents a build from uploading 50GB of artifacts to MinIO. One deployment can fill the disk.

Fix in `storage.rs`:
```rust
const MAX_ARTIFACT_SIZE_BYTES: u64 = 500 * 1024 * 1024; // 500MB
if total_bytes > MAX_ARTIFACT_SIZE_BYTES {
    anyhow::bail!("artifact size {}MB exceeds 500MB limit", total_bytes / 1024 / 1024);
}
```

---

## 11. Proposed Target Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Single VM (prod baseline)               │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  network: internal                                       │   │
│  │  Traefik (HTTPS, Let's Encrypt)                         │   │
│  │  API (2 replicas)           Grafana + Loki + Alloy      │   │
│  │  Postgres + PgBouncer       Prometheus                  │   │
│  │  NATS (JetStream)           Local Docker Registry       │   │
│  │  MinIO                                                  │   │
│  └──────────────┬──────────────────────────┬──────────────┘   │
│                 │                            │                   │
│  ┌──────────────▼──────────┐  ┌─────────────▼──────────────┐  │
│  │  network: build-net      │  │  network: serve-net         │  │
│  │                          │  │                             │  │
│  │  Worker (3 replicas)     │  │  serve-{id} containers:     │  │
│  │  build-{id} containers   │  │  - nginx:alpine (static)    │  │
│  │  (internet + minio only) │  │  - nginx+node (Next.js SSR) │  │
│  │  --cpus=2 --memory=4g    │  │  - uvicorn (Python)         │  │
│  │  --cap-drop=ALL          │  │  - binary (Go/Rust)         │  │
│  │  railpack build          │  │  - puma (Rails)             │  │
│  └──────────────────────────┘  │  --cpus=0.5 --memory=512m  │  │
│                                 └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Build pipeline with Railpack

```
GitHub push / manual trigger
         │
         ▼
API creates deployment record (state: queued)
         │
         ▼
NATS build.jobs stream
         │
         ▼
Worker picks up job (semaphore: max 2 per instance)
         │
         ├─ git clone (authenticated)
         ├─ railpack generate → Dockerfile
         ├─ docker build → image
         ├─ docker push → local registry:5000
         ├─ upload static assets to MinIO (for CDN/backup)
         └─ publish BuildResult (state: ready, image: registry.../deployment-{id})
                  │
                  ▼
         API result subscriber updates DB
                  │
                  ▼
         Traefik label added dynamically for deployment subdomain
         (or serve container started on first request)
```

### Serving pipeline

```
User visits a1b2c3d4-preview.yourdomain.com
         │
         ▼
Traefik (routes to serve-{id} container directly)
         │
         ▼
If container not running:
  API starts it (docker run registry.../deployment-{id})
  Railpack image already knows how to serve
  nginx handles static, runtime handles dynamic
         │
         ▼
Container responds
         │
  (API never in the hot path for serving)
```

---

## 12. Migration Roadmap

Ordered by risk-reduction impact. Each tier is shippable independently.

### Tier 1 — Minimum for real users (1-2 weeks)

| Task | Files to change | Risk |
|------|----------------|------|
| Move all secrets to `.env`, add to `.gitignore` | `docker-compose.yml`, `.env` | Low |
| Add Traefik HTTPS + Let's Encrypt | `docker-compose.yml` | Low |
| Add resource limits to all containers | `docker-compose.yml` | Low |
| Add health checks to MinIO and NATS | `docker-compose.yml` | Low |
| Add graceful shutdown to API and worker | `api/main.rs`, `build-worker/main.rs` | Medium |
| Wrap background tasks in supervisor loop | `api/main.rs` | Low |
| Add artifact size limit | `build-worker/storage.rs` | Low |
| Run containers as non-root | Both `Dockerfile`s | Low |

### Tier 2 — Correct the serving architecture (2-3 weeks)

| Task | Files to change |
|------|----------------|
| Install Railpack CLI in worker Dockerfile | `build-worker/Dockerfile` |
| Replace `builder.rs` with Railpack invocation | `build-worker/builder.rs` |
| Add local Docker registry to docker-compose | `docker-compose.yml` |
| Replace output type detection with image-based serving | `build-worker/main.rs`, `api/services/deployment_servers.rs` |
| Add serve-net Docker network | `docker-compose.yml`, `deployment_servers.rs` |
| Remove `serve_artifact` from API routes | `api/routes/deployments.rs` |
| Route serve containers directly via Traefik labels | `deployment_servers.rs` |

### Tier 3 — Production hardening (3-4 weeks)

| Task | Files to change |
|------|----------------|
| Add NATS dead-letter queue | `build-worker/nats.rs`, `api/services/nats.rs` |
| Add PgBouncer to docker-compose | `docker-compose.yml` |
| Add Loki + Grafana to docker-compose | `docker-compose.yml` |
| Add Prometheus metrics endpoint | `api/main.rs` |
| Separate build-net from internal network | `docker-compose.yml`, `build-worker/builder.rs` |
| Add build container security flags | `build-worker/builder.rs` |
| Incremental build log persistence to DB | `api/services/nats.rs` |
| Postgres backup cron | Infrastructure |

### Tier 4 — Scale (as needed)

| Task | Prerequisite |
|------|-------------|
| Kubernetes migration (Helm chart) | Tier 1-3 complete |
| MinIO → real S3 (swap SDK endpoint) | Any time |
| NATS clustering (3 nodes) | Multi-VM |
| Postgres streaming replica | Multi-VM |
| Multi-region Traefik routing | Multi-VM |
| Canary deployments | Tier 2 complete |

---

*Generated from codebase analysis on 2026-05-17. Re-run this audit after each tier completion.*
