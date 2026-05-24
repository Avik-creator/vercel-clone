# Vercel Clone — System Documentation

Complete reference for architecture, services, build/serve pipelines, configuration, and troubleshooting. For quick start, see [README.md](./README.md).

---

## Table of contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Docker Compose services](#3-docker-compose-services)
4. [Networks](#4-networks)
5. [Deployment lifecycle](#5-deployment-lifecycle)
6. [Build pipeline](#6-build-pipeline)
7. [Local container registry](#7-local-container-registry)
8. [BuildKit and buildctl](#8-buildkit-and-buildctl) — **deep dive:** [docs/BUILDKIT-AND-BUILDCTL.md](./docs/BUILDKIT-AND-BUILDCTL.md)
9. [Serve pipeline (preview URLs)](#9-serve-pipeline-preview-urls)
10. [NATS JetStream](#10-nats-jetstream)
11. [Build logs and SSE](#11-build-logs-and-sse)
12. [API server](#12-api-server)
13. [Build worker](#13-build-worker)
14. [Frontend dashboard](#14-frontend-dashboard)
15. [Authentication and GitHub](#15-authentication-and-github)
16. [Database schema](#16-database-schema)
17. [Configuration reference](#17-configuration-reference)
18. [Local development](#18-local-development)
19. [Troubleshooting](#19-troubleshooting)
20. [Project layout](#20-project-layout)

---

## 1. Overview

This project is a **self-hosted deployment platform** (Vercel-like) that:

1. Connects GitHub repositories to **projects**
2. Triggers **deployments** on push or from the UI
3. **Builds** the app in isolation (Nixpacks + BuildKit)
4. **Pushes** a Docker image to a local registry
5. **Runs** a preview container and exposes it via Traefik

Core binaries:

| Binary | Crate | Role |
|--------|-------|------|
| API | `crates/api` | HTTP API, auth, DB, NATS subscribers, starts preview containers |
| Worker | `crates/build-worker` | Consumes build jobs, runs git + nixpacks + buildctl |

The API never runs builds. The worker never serves HTTP to users.

---

## 2. Architecture

### High-level flow

```
GitHub webhook / UI "Deploy"
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│  API (Axum, runs as appuser)                                 │
│  • Creates deployment row (queued)                           │
│  • Publishes BuildJob → NATS JetStream                       │
│  • Subscribes build.logs / build.results                     │
│  • On Ready: docker run image + Traefik labels                 │
└───────────────┬─────────────────────────────┬───────────────┘
                │                             │
         PostgreSQL                      NATS JetStream
         (state, logs)                         │
                │                             ▼
                │              ┌──────────────────────────────┐
                │              │  Worker (appuser)             │
                │              │  • Pull job (durable consumer)│
                │              │  • git clone → /tmp/builds/{id}│
                │              │  • nixpacks plan               │
                │              │  • buildctl → BuildKit         │
                │              │  • Publish logs + BuildResult  │
                │              └──────────┬─────────────────────┘
                │                         │
                │                         ▼
                │              ┌──────────────────────────────┐
                │              │  BuildKit (buildkitd)         │
                │              │  privileged, build-net only   │
                │              │  push → registry:5000          │
                │              └──────────┬─────────────────────┘
                │                         │
                │                         ▼
                │              ┌──────────────────────────────┐
                │              │  Registry (registry:2)        │
                │              │  localhost:5000 on host       │
                │              └──────────┬─────────────────────┘
                │                         │
                │         docker run --pull always
                │                         ▼
                │              ┌──────────────────────────────┐
                │              │  serve-{deployment_id}        │
                │              │  on serve-net                 │
                └──────────────┴──────────┬─────────────────────┘
                                          ▼
                               Traefik :80 / :443
                               {hash}-preview.localhost
```

### Design principles

- **Image-based deploys** — The artifact is a Docker image in a registry, not thousands of files in object storage (Phase 2). MinIO remains in the stack for legacy/optional paths but is not the primary serve path.
- **Queue between API and worker** — NATS JetStream gives durable, at-least-once job delivery.
- **Thin clients, fat daemons** — Worker uses `buildctl` against `buildkitd`; API uses `docker` CLI against the host daemon.
- **Network isolation** — Builds on `build-net`; previews on `serve-net`; data services on `default`.

---

## 3. Docker Compose services

| Service | Image | Purpose |
|---------|-------|---------|
| **traefik** | `traefik:v3` | Reverse proxy; routes preview hosts and `api.localhost` |
| **db** | `postgres:17-alpine` | Primary database |
| **pgbouncer** | `edoburu/pgbouncer` | Connection pooling (production compose; local override may bypass) |
| **nats** | `nats:2.11-alpine` | JetStream message bus |
| **minio** | `minio/minio` | S3-compatible storage (legacy / auxiliary) |
| **registry** | `registry:2` | Local Docker image registry on port **5000** |
| **buildkit** | `moby/buildkit` | BuildKit daemon (`buildkitd`) |
| **api** | built from `crates/api/Dockerfile` | REST API + NATS consumers + `docker run` for previews |
| **worker** | built from `crates/build-worker/Dockerfile` | Build job processor |
| **prometheus** | `prom/prometheus` | Metrics scrape target |
| **loki** | `grafana/loki` | Log aggregation |
| **grafana** | `grafana/grafana` | Dashboards |
| **alloy** | `grafana/alloy` | Log shipping |
| **postgres-backup** | `postgres-backup-local` | Daily DB backups |

### Ports (host)

| Port | Service |
|------|---------|
| 80, 443 | Traefik |
| 8081 | Traefik dashboard |
| 5432 | PostgreSQL |
| 6432 | PgBouncer |
| 4222, 8222 | NATS (client, monitoring) |
| 9000, 9001 | MinIO API, console |
| **5000** | **Docker registry** |
| 8080 | API (direct; also via Traefik) |
| 9090 | Prometheus |
| 3100 | Loki |
| 3001 | Grafana |

---

## 4. Networks

Three user-defined networks isolate traffic:

| Network | Name | Who attaches | Purpose |
|---------|------|--------------|---------|
| **default** | `vercel-clone_default` | api, db, nats, minio, registry, traefik, … | Internal platform services |
| **build-net** | `vercel-clone_build-net` | worker, buildkit | Builds: git, npm/bun, registry push; no DB/NATS from build jobs |
| **serve-net** | `vercel-clone_serve-net` | api, traefik, preview containers | Public-facing preview routing only |

Build containers (inside BuildKit) use `--opt network=vercel-clone_build-net` so user build scripts cannot reach `postgres:5432` or `nats:4222` on the default network.

---

## 5. Deployment lifecycle

### States

| State | Meaning |
|-------|---------|
| `queued` | Row created; job published to NATS |
| `building` | Worker reported build in progress |
| `uploading` | Legacy state (API still handles it; image-based flow typically skips this) |
| `ready` | Build succeeded; `image_ref` set; API should start serve container |
| `error` | Build or serve failed |
| `cancelled` | User cancelled |

### Typical timeline

1. **Create deployment** — API inserts row, sets `url` to `{8-char-hash}-preview.{BASE_DOMAIN}`, publishes `build.jobs.{deployment_id}`.
2. **Worker picks job** — Publishes `Building`, clones repo to `/tmp/builds/{deployment_id}`.
3. **Build** — Nixpacks + buildctl; logs on `build.logs.{id}`; lines persisted to `build_log_lines`.
4. **Complete** — Worker publishes `Ready` with `image_ref` like `localhost:5000/deployment-{uuid}:latest`.
5. **API** — Updates DB, calls `DeploymentServers::start_image`, Traefik routes preview host to container.
6. **Idle cleanup** — After 300s without a repeat `start_image` touch, API may remove idle serve containers (see serve section).

### Preview URL

- Pattern: `{hash}-preview.{BASE_DOMAIN}` (e.g. `6b84844b-preview.localhost`)
- **Local dev:** use `http://` (see `docker-compose.override.yml`, `SERVE_TLS=false`)
- **Production compose:** Traefik may use HTTPS via Let's Encrypt

`BASE_DOMAIN` must **not** include a port (use `localhost`, not `localhost:8080`).

---

## 6. Build pipeline

Implementation: `crates/build-worker/src/builder.rs`

### Steps

| Step | Command / action | Output |
|------|------------------|--------|
| 1 | `git clone` + `git checkout` | Source in work dir |
| 2 | `nixpacks build -o .` | `.nixpacks/Dockerfile`, plan in logs |
| 3 | `buildctl build` | Image pushed to registry |
| 4 | Return `image_ref` | Host-side tag stored via NATS → API → DB |

### Nixpacks

- Version: **v1.41.0** (installed in worker image)
- Auto-detects Node, Bun, pnpm, etc.
- Install command inferred from lockfiles (`bun.lock` → `bun install`, etc.)
- `NIXPACKS_NODE_VERSION` (default `22`) passed as build env
- Project `build_command` from DB used when set

### Concurrency and deduplication

- `MAX_CONCURRENT_BUILDS` (default `2`) — semaphore in worker
- **In-flight dedup** — second job for same `deployment_id` is acked and skipped (avoids duplicate clone into same directory)
- Work dir wiped before each fresh clone

### Timeouts

- Default build timeout: **600 seconds** (`BUILD_TIMEOUT_SECS` / config)

---

## 7. Local container registry

### What it is

The [Docker Registry V2](https://distribution.github.io/distribution/) (`registry:2`) stores built images on disk in volume `registry_data`.

### Image naming

```
{registry_host}/deployment-{deployment_uuid}:latest
```

Examples:

- BuildKit push: `registry:5000/deployment-4d7b0719-29f2-4d1a-ab66-510dc818deb3:latest`
- DB / docker run: `localhost:5000/deployment-4d7b0719-29f2-4d1a-ab66-510dc818deb3:latest`

### Two hostnames (critical)

| Variable | Value (compose) | Used by |
|----------|-----------------|---------|
| `BUILD_REGISTRY_URL` | `registry:5000` | BuildKit inside Docker network |
| `REGISTRY_URL` | `localhost:5000` | Host Docker daemon (`docker run`) |

Same physical registry; different DNS names depending on **who** connects.

### Push vs pull

| Phase | Operation | Who |
|-------|-----------|-----|
| Push | After build, BuildKit uploads layers | `buildkitd` → `registry:5000` |
| Pull | Before preview starts | Host daemon ← `localhost:5000` |

**Ready in Postgres does not mean the image is on the host.** The API uses `docker run --pull always` so the host daemon fetches layers before starting the container.

### BuildKit registry config

`infra/buildkitd.toml`:

```toml
[registry."registry:5000"]
http = true
insecure = true
```

Required because the local registry uses plain HTTP.

---

## 8. BuildKit and buildctl

### Roles

| Component | Runs in | Role |
|-----------|---------|------|
| **buildctl** | Worker container | CLI client; sends build + push request |
| **buildkitd** | `buildkit` container | Executes Dockerfile steps, layer cache, push |

They communicate via a **shared Unix socket** mounted from the host:

```
/var/run/buildkit/buildkitd.sock
```

Worker env: `BUILDKIT_HOST=unix:///var/run/buildkit/buildkitd.sock`

This is **not** a remote machine — it is a **separate container** on the same host (like `docker` CLI vs `dockerd`).

### buildctl invocation

```text
buildctl build \
  --frontend dockerfile.v0 \
  --local context=. \
  --local dockerfile=.nixpacks \
  --opt network=vercel-clone_build-net \
  --output type=image,name=registry:5000/deployment-{id}:latest,push=true
```

Benefits:

- Worker never holds multi-GB image tarballs
- Layer caching inside BuildKit
- Build logs stream to NATS (`#10 RUN bun install`, etc.)

### Privilege and permissions

- **buildkit** service runs `privileged: true`
- **worker** runs as **appuser** (uid 1001) via `infra/docker-entrypoint.sh`
- Socket permissions: GID mapping or `chmod 666` on `buildkitd.sock` and sometimes `docker.sock` (OrbStack/Docker Desktop)

BuildKit entrypoint (`infra/buildkit-entrypoint.sh`) pre-creates `/home/appuser` for uid 1001 so metadata pulls do not fail with `mkdir /home/appuser: permission denied`.

---

## 9. Serve pipeline (preview URLs)

Implementation: `crates/api/src/services/deployment_servers.rs`

### On deployment Ready

When NATS delivers `BuildResult { state: Ready, image_ref: Some(...) }`:

1. API updates `deployments` row
2. Loads preview `url` (host only, no scheme)
3. Runs equivalent to:

```text
docker run -d \
  --pull always \
  --name serve-{deployment_id} \
  --network vercel-clone_serve-net \
  --cpus 0.5 --memory 512m \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  -e PORT=3000 \
  -l traefik.enable=true \
  -l traefik.http.routers.serve-{uuid}.rule=Host(`{hash}-preview.localhost`) \
  -l traefik.http.routers.serve-{uuid}.entrypoints=web \
  -l traefik.http.services.serve-{uuid}.loadbalancer.server.port=3000 \
  {image_ref}
```

4. Waits until HTTP to `http://serve-{id}:3000/` succeeds (readiness poll)
5. Traefik on `serve-net` forwards browser traffic to that container

### One container per deployment

- Each deployment ID gets its own image tag and **`serve-{deployment_id}`** container
- Redeploying creates a **new** deployment UUID → new image → new container → new preview URL
- Repeat visits to the same preview URL hit the **same** container (not a new container per HTTP request)

### Idle cleanup

- Background task every 60s removes containers idle for **300 seconds** (tracked in API memory when `start_image` last ran)
- Note: `last_accessed` is updated on `start_image`, not on every HTTP request to the preview

### API and Docker socket

The API container mounts `/var/run/docker.sock` so it can start preview containers on the **host** daemon. Entrypoint ensures `appuser` can run `docker` (group mapping or `chmod 666` fallback).

---

## 10. NATS JetStream

### Streams

| Stream | Subjects | Purpose |
|--------|----------|---------|
| `build_jobs` | `build.jobs.>`, `build.jobs` | Durable build queue |
| `build_jobs_dlq` | `dlq.build.jobs.>` | Failed jobs for admin replay |
| `build_results` | `build.results.>`, `build.results` | State updates from worker |
| `build_logs` | `build.logs.>`, `build.logs` | Ephemeral log lines (also persisted in Postgres) |

### Worker consumer (`build-worker`)

- Durable: `build-worker`
- Explicit ack after build completes
- `max_deliver: 3`, backoff 5s / 30s / 120s
- `ack_wait: 600s` (must cover long builds)

Worker **acks only after** publishing final `BuildResult`. Failed publish → `nak` for retry.

On failure after max delivers, worker can publish to **DLQ** (`dlq.build.jobs.{deployment_id}`).

### API consumers

| Task | Subscription | Handler |
|------|--------------|---------|
| Logs | `build.logs.>` | Insert `build_log_lines`, broadcast to SSE |
| Results | JetStream pull `build_results` | Update `deployments`, start serve on Ready |

### Auth and TLS

- NATS user/password via compose
- TLS optional: `NATS_TLS_CA` path to CA cert (`infra/nats/certs/`)
- Local override often sets `NATS_TLS_CA=""` to disable TLS

---

## 11. Build logs and SSE

### Path

```
Worker → publish build.logs.{deployment_id}
       → API subscriber → INSERT build_log_lines
                       → tokio broadcast channel
                       → SSE GET /v1/deployments/{id}/logs
```

### Persistence

- **Incremental:** each line → `build_log_lines (deployment_id, line, timestamp)`
- **Aggregate:** on terminal state, `deployments.build_log` updated via `string_agg` from lines

### SSE behavior

- Replays all historical lines from DB first
- Then live tail from broadcast channel
- Terminal: `close_log_sender` → SSE `done` event
- Auth: `Authorization: Bearer` header or `?token=` query param

### Frontend

`frontend/components/build-log-viewer.tsx` — batches lines (~32ms), direct DOM append for performance during large BuildKit output.

---

## 12. API server

### Responsibilities

- REST API + GitHub webhooks
- JWT authentication
- Publish build jobs
- Subscribe to NATS logs/results
- Start/stop preview containers
- Admin DLQ endpoints

### Background tasks (`main.rs`)

| Task | Interval | Function |
|------|----------|----------|
| Log subscriber | continuous | `subscribe_all_logs` |
| Result subscriber | continuous | `subscribe_build_results` |
| Idle serve cleanup | 60s | `cleanup_idle` |

### Key routes

See [README.md](./README.md#api-reference) for full list.

Admin (Bearer `ADMIN_SECRET` or `BUILD_WORKER_SECRET`):

- `GET /v1/admin/failed-jobs`
- `POST /v1/admin/failed-jobs/{sequence}/replay`

### Runtime user

- Runs as **appuser** (uid 1001) after `infra/docker-entrypoint.sh`
- Needs Docker socket access to start previews

---

## 13. Build worker

### Responsibilities

- Consume `build_jobs` from JetStream
- Clone, build, push image
- Publish logs and results
- DLQ on failure

### Filesystem

- Build work dirs: **tmpfs** `/tmp/builds` (uid 1001)
- Ephemeral; removed after successful job ack

### Health

- Touches `/tmp/worker-alive` every 5s for Docker healthcheck

### Installed tools

- `git`, `nixpacks`, `buildctl` (from BuildKit release)
- No Docker daemon inside worker — only BuildKit socket

---

## 14. Frontend dashboard

- **Stack:** Next.js, SWR, Tailwind
- **Dev:** `cd frontend && pnpm dev` → `http://localhost:3000`
- **API:** `NEXT_PUBLIC_API_URL` → `http://localhost:8080`

Features: projects, deployments, live logs, GitHub linking, env vars, API keys.

Preview links use `http://` for `*.localhost` (`deploymentPublicUrl` in `frontend/lib/utils.ts`).

---

## 15. Authentication and GitHub

### Auth

- Register / login → JWT access token (stored in `localStorage`)
- Refresh token flow
- GitHub OAuth for login and repo access

### GitHub App / webhook

- Webhook: `POST /webhooks/github` — push events can trigger deployments
- Installation token used for private repo clone (injected into `BuildJob.github_token`)
- Configure `GITHUB_APP_ID`, `GITHUB_APP_PRIVATE_KEY`, `GITHUB_WEBHOOK_SECRET`, OAuth client ID/secret

### Build job payload

```json
{
  "deployment_id": "uuid",
  "project_id": "uuid",
  "git_url": "https://github.com/org/repo",
  "commit_sha": "...",
  "branch": "main",
  "build_command": "optional",
  "github_token": "optional",
  "env_vars": { "KEY": "VALUE" }
}
```

---

## 16. Database schema

### Core tables

| Table | Purpose |
|-------|---------|
| `users` | Accounts (email/password or GitHub) |
| `projects` | Linked repos, build settings, `env_vars` JSONB |
| `deployments` | Per-deploy state, `url`, `image_ref`, `build_log`, timestamps |
| `build_log_lines` | Incremental log lines per deployment |
| `api_keys` | Hashed API keys |

### Important columns (`deployments`)

| Column | Description |
|--------|-------------|
| `state` | `deployment_state` enum |
| `url` | Preview hostname only (e.g. `abc12345-preview.localhost`) |
| `image_ref` | Docker image tag for serve (e.g. `localhost:5000/deployment-...:latest`) |
| `build_log` | Aggregated log text (terminal builds) |

Migrations: `crates/api/migrations/`

---

## 17. Configuration reference

### Root `.env` (used by compose)

| Variable | Description |
|----------|-------------|
| `POSTGRES_USER` / `POSTGRES_PASSWORD` | Database credentials |
| `NATS_USER` / `NATS_PASSWORD` | NATS auth |
| `MINIO_ROOT_USER` / `MINIO_ROOT_PASSWORD` | MinIO |
| `JWT_SECRET` | API JWT signing |
| `GITHUB_*` | App, OAuth, webhook |
| `BUILD_WORKER_SECRET` | Worker callback auth; fallback for admin |
| `ADMIN_SECRET` | Optional admin API auth |
| `BASE_DOMAIN` | Preview URL domain (no port) |
| `API_PUBLIC_URL` | OAuth callback base |
| `FRONTEND_URL` | OAuth redirect after login |
| `ACME_EMAIL` | Let's Encrypt (production Traefik) |

### Worker environment (compose)

| Variable | Default | Description |
|----------|---------|-------------|
| `REGISTRY_URL` | `localhost:5000` | Tag stored in DB / used by API |
| `BUILD_REGISTRY_URL` | `registry:5000` | Tag used for BuildKit push |
| `BUILD_NETWORK` | `vercel-clone_build-net` | BuildKit network opt |
| `BUILDKIT_HOST` | unix socket path | buildctl → buildkitd |
| `MAX_CONCURRENT_BUILDS` | `2` | Worker parallelism |
| `NIXPACKS_NODE_VERSION` | `22` | Node version for Nixpacks |
| `HOME` | `/home/appuser` | Required for non-root buildctl |

### API environment (compose)

| Variable | Description |
|----------|-------------|
| `SERVE_NETWORK` | Docker network for preview containers |
| `SERVE_TLS` | Use Traefik `websecure` + TLS for previews |
| `DOCKER_NETWORK` | Legacy/default network name |
| `DATABASE_URL` | Postgres connection |
| `NATS_URL`, `NATS_TLS_CA` | NATS connection |

---

## 18. Local development

### Start stack

```bash
cp crates/api/.env.example .env
# Edit .env with secrets

docker compose up -d
```

`docker-compose.override.yml` adjusts local dev:

- HTTP-only Traefik (`SERVE_TLS=false`)
- API on `api.localhost` via port 80
- Direct Postgres (no PgBouncer) for scram auth
- NATS without TLS

### Frontend

```bash
cd frontend && pnpm install && pnpm dev
```

### Rebuild after Rust changes

```bash
docker compose build api worker
docker compose up -d api worker
```

### Test a preview

1. Create project + deployment in UI
2. Wait for `ready`
3. Open `http://{hash}-preview.localhost` (macOS `*.localhost` → 127.0.0.1)

### Useful commands

```bash
# Worker logs
docker compose logs worker -f

# API logs
docker compose logs api -f

# List preview containers
docker ps --filter name=serve-

# Images in registry (on host)
curl -s http://localhost:5000/v2/_catalog | jq

# NATS monitoring
curl -s http://localhost:8222/healthz
```

---

## 19. Troubleshooting

### Build fails: BuildKit socket permission denied

**Symptom:** `dial unix .../buildkitd.sock: permission denied`

**Cause:** Worker runs as `appuser`; socket not writable.

**Fix:** Ensure `infra/docker-entrypoint.sh` runs and chmods buildkit socket; rebuild/restart worker.

---

### Build fails: mkdir /home/appuser permission denied

**Symptom:** buildctl stderr during `load metadata` for nixpacks base image.

**Cause:** BuildKit daemon cannot create client home dir.

**Fix:** `infra/buildkit-entrypoint.sh` creates `/home/appuser`; worker sets `HOME=/home/appuser`.

---

### git clone exit code 128

**Symptom:** `destination path '.' already exists` or clone errors while build steps appear in logs.

**Cause:** Duplicate NATS jobs for same deployment running concurrently.

**Fix:** Worker in-flight dedup (already in codebase); avoid double-publishing jobs.

---

### Deployment Ready but preview 404

**Symptom:** State `ready`, Traefik returns 404.

**Causes:**

1. API could not `docker run` (docker.sock permission for appuser)
2. Image not pulled (`--pull always` missing — should be present)
3. Wrong preview URL scheme (`https://` locally — use `http://`)

**Check:**

```bash
docker compose logs api | grep -E "failed to start|started deployment"
docker ps --filter name=serve-
```

---

### Unable to find image locally

**Symptom:** API log: `Unable to find image 'localhost:5000/deployment-...' locally`

**Cause:** Image in registry but not on host daemon yet.

**Fix:** `docker run --pull always` (in `deployment_servers.rs`).

---

### Build logs empty in UI after failure

**Cause:** Frontend only connected SSE for active states (fixed in `BuildLogViewer`).

**Check:** `build_log_lines` in Postgres; `GET /v1/deployments/{id}/logs` with auth.

---

### BASE_DOMAIN / host mismatch

**Symptom:** Preview URL never routes.

**Cause:** `BASE_DOMAIN=localhost:8080` stores wrong host; Traefik expects `hash-preview.localhost` on port 80.

**Fix:** `BASE_DOMAIN=localhost` only.

---

## 20. Project layout

```
vercel-clone/
├── crates/
│   ├── api/                    # Axum API
│   │   ├── src/
│   │   │   ├── routes/         # HTTP handlers
│   │   │   ├── services/       # deployments, nats, deployment_servers, admin, github
│   │   │   └── main.rs         # NATS subscribers, background tasks
│   │   └── migrations/
│   └── build-worker/
│       ├── src/
│       │   ├── builder.rs      # git, nixpacks, buildctl
│       │   ├── nats.rs         # JetStream consumer
│       │   └── main.rs         # Job dispatch, dedup
│       └── Dockerfile
├── frontend/                   # Next.js dashboard
├── infra/
│   ├── docker-entrypoint.sh    # appuser, socket permissions
│   ├── buildkit-entrypoint.sh
│   ├── buildkitd.toml
│   └── nats/
├── docker-compose.yml
├── docker-compose.override.yml # Local dev overrides
├── DOCS.md                     # This file
├── AUDIT.md                    # Security/hardening checklist
└── README.md                   # Quick start
```

---

## Further reading

- [docs/BUILDKIT-AND-BUILDCTL.md](./docs/BUILDKIT-AND-BUILDCTL.md) — buildctl, buildkitd, flags, registry, networks (detailed)
- [README.md](./README.md) — Quick start and API table
- [AUDIT.md](./AUDIT.md) — Production hardening notes
- Blog: [Part 1](https://avikmukherjee.com/blog/i-built-a-mini-vercel-like-clone-in-rust-in-one-day-every-mistake) · [Part 2](https://avikmukherjee.com/blog/vercel-clone-rust-phase-2-nixpacks-buildkit-audit)
