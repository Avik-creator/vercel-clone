# Vercel Clone

A self-hosted deployment platform built with Rust. Connect a GitHub repository, push code, and get a live preview URL — similar to Vercel but running entirely on your own infrastructure.

## How It Works

```
GitHub push / UI trigger
        │
        ▼
  API (Axum/Rust)  ──NATS JetStream──▶  Build Worker (Rust)
        │                                      │
        │                               Docker container
        │                               git clone → npm install → npm run build
        │                                      │
        │◀──────── build result ───────────────┘
        │                                      │
   Postgres                              MinIO (S3)
  (state/logs)                        (build artifacts)
        │
        ▼
  Serve artifact
  ├─ Static export → serve files from MinIO directly
  └─ Standalone SSR → spawn node server, proxy requests
```

1. You trigger a deployment from the dashboard (or GitHub webhook fires)
2. The API publishes a build job to NATS JetStream
3. The build worker picks it up, spins up a Docker container, clones the repo, installs dependencies, and runs the build command
4. Build logs stream in real time via SSE to the dashboard
5. Artifacts are uploaded to MinIO; the DB is updated to `ready`
6. Visiting the preview URL serves the app — static files direct from MinIO, Next.js standalone apps via a proxied Node.js process

## Stack

| Layer | Technology |
|---|---|
| API server | Rust, Axum 0.8, sqlx |
| Build worker | Rust, Docker-in-Docker |
| Database | PostgreSQL 17 |
| Message queue | NATS JetStream |
| Object storage | MinIO (S3-compatible) |
| Frontend | Next.js 15, Tailwind CSS, SWR |
| Auth | JWT (access + refresh tokens), GitHub OAuth |

## Prerequisites

- Docker & Docker Compose
- A GitHub App ([create one](https://github.com/settings/apps)) for OAuth login and private repo access

## Quick Start

**1. Clone and configure**

```bash
git clone https://github.com/Avik-creator/vercel-clone
cd vercel-clone
cp crates/api/.env.example .env
```

Edit `.env` with your values (see [Environment Variables](#environment-variables) below).

**2. Start all services**

```bash
docker compose up -d
```

This starts:
- PostgreSQL on `localhost:5432`
- NATS on `localhost:4222`
- MinIO on `localhost:9000` (console at `localhost:9001`)
- API server on `localhost:8080`
- Build worker

**3. Open the dashboard**

```bash
cd frontend
pnpm install
pnpm dev
```

Visit `http://localhost:3000`

**4. Access preview deployments**

Preview URLs follow the pattern `{hash}-preview.localhost`. Configure your `/etc/hosts` or use a wildcard DNS resolver like `dnsmasq` to point `*.localhost` to `127.0.0.1`:

```bash
# macOS — already works, *.localhost resolves to 127.0.0.1
# Linux
echo "127.0.0.1 *.localhost" | sudo tee -a /etc/hosts
```

Then visit `http://{hash}-preview.localhost:8080` for any ready deployment.

## Environment Variables

| Variable | Description | Example |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string | `postgres://postgres:password@localhost:5432/vercel_clone` |
| `JWT_SECRET` | Secret for signing JWT tokens | random 64-char string |
| `GITHUB_APP_ID` | GitHub App ID | `123456` |
| `GITHUB_APP_PRIVATE_KEY` | GitHub App private key (PEM) | `-----BEGIN RSA PRIVATE KEY-----...` |
| `GITHUB_CLIENT_ID` | GitHub OAuth App client ID | |
| `GITHUB_CLIENT_SECRET` | GitHub OAuth App client secret | |
| `GITHUB_WEBHOOK_SECRET` | Webhook signature secret | |
| `BUILD_WORKER_SECRET` | Shared secret between API and worker | random string |
| `BASE_DOMAIN` | Domain for preview URLs | `localhost` |
| `NATS_URL` | NATS server URL | `nats://localhost:4222` |
| `MINIO_ENDPOINT` | MinIO endpoint | `http://localhost:9000` |
| `MINIO_ACCESS_KEY` | MinIO access key | `minioadmin` |
| `MINIO_SECRET_KEY` | MinIO secret key | `minioadmin` |
| `MINIO_BUCKET` | Bucket for build artifacts | `deployments` |
| `FRONTEND_URL` | Frontend URL for OAuth redirects | `http://localhost:3000` |

## Deploying a Project

1. **Create a project** — link it to a GitHub repository from the dashboard
2. **Set environment variables** — add any build-time env vars your app needs
3. **Configure build settings** (optional):
   - `build_command` — defaults to `npm run build`
   - `output_dir` — auto-detected; set explicitly if needed (`out`, `dist`, `.next`)
4. **Trigger a deployment** — click Deploy or push to the linked branch

### Supported Output Types

The worker auto-detects what your build produced:

| Type | Detection | How it's served |
|---|---|---|
| Next.js standalone | `.next/standalone/server.js` exists | Node.js process proxied by the API |
| Static export | `out/` directory exists | Files served directly from MinIO |
| Generic static | `dist/` or `build/` directory | Files served directly from MinIO |

For Next.js standalone, add this to your `next.config.ts`:

```ts
const nextConfig = {
  output: 'standalone',
}
```

For static export:

```ts
const nextConfig = {
  output: 'export',
}
```

## Project Structure

```
.
├── crates/
│   ├── api/                  # Axum API server
│   │   ├── src/
│   │   │   ├── routes/       # HTTP handlers
│   │   │   ├── services/     # Business logic, NATS, deployment servers
│   │   │   ├── models/       # DB models
│   │   │   └── main.rs       # Server setup, NATS subscribers
│   │   ├── migrations/       # sqlx migrations
│   │   └── Dockerfile
│   └── build-worker/         # Build job processor
│       ├── src/
│       │   ├── builder.rs    # Docker container orchestration
│       │   ├── storage.rs    # MinIO upload
│       │   ├── nats.rs       # Job queue consumer
│       │   └── main.rs       # Job dispatch, artifact extraction
│       └── Dockerfile
├── frontend/                 # Next.js dashboard
│   ├── app/                  # App router pages
│   ├── components/           # UI components
│   └── lib/                  # API client, hooks, auth context
└── docker-compose.yml
```

## API Reference

All endpoints are prefixed with `/v1`. Authenticated routes require `Authorization: Bearer <token>`.

| Method | Path | Description |
|---|---|---|
| `POST` | `/v1/auth/register` | Create account |
| `POST` | `/v1/auth/login` | Get access + refresh tokens |
| `POST` | `/v1/auth/refresh` | Rotate access token |
| `GET` | `/v1/auth/me` | Current user |
| `GET` | `/v1/projects` | List projects |
| `POST` | `/v1/projects` | Create project |
| `PATCH` | `/v1/projects/:id` | Update project settings |
| `GET` | `/v1/projects/:id/deployments` | List deployments for project |
| `POST` | `/v1/projects/:id/deployments` | Trigger deployment |
| `GET` | `/v1/deployments/:id` | Get deployment |
| `GET` | `/v1/deployments/:id/logs` | Stream build logs (SSE) |
| `POST` | `/v1/deployments/:id/cancel` | Cancel in-progress build |
| `POST` | `/v1/deployments/:id/promote` | Promote to production |
| `GET` | `/v1/github/repos` | List accessible GitHub repos |
| `POST` | `/v1/projects/:id/link` | Link GitHub repo to project |
| `GET/PUT` | `/v1/projects/:id/env` | Manage environment variables |

## Known Limitations

This is a learning project / local deployment platform. It is **not** production-ready as-is:

- Next.js standalone servers run inside the API container with no isolation from the API process
- No build resource limits (CPU, memory, disk) on build containers
- No rate limiting on API endpoints
- Environment variables stored in plaintext in the database
- No CDN — all artifact traffic goes through the API
- No custom domain support
- Preview URLs only work on the host machine without wildcard DNS setup

See the architecture notes in the codebase for what would be needed for a production deployment.

## License

MIT
