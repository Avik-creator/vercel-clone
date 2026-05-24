# BuildKit, buildctl, and the image build path

Deep dive into how this project builds Docker images: what **buildctl** is, how it talks to **buildkitd**, and every flag and concept in the worker’s `buildctl build` invocation.

**Related code:** `crates/build-worker/src/builder.rs`, `infra/buildkitd.toml`, `docker-compose.yml` (`buildkit`, `worker` services).

---

## Table of contents

1. [Why not `docker build`?](#1-why-not-docker-build)
2. [The client–daemon split](#2-the-clientdaemon-split)
3. [BuildKit (`buildkitd`)](#3-buildkit-buildkitd)
4. [buildctl](#4-buildctl)
5. [The Unix socket and `BUILDKIT_HOST`](#5-the-unix-socket-and-buildkit_host)
6. [Your exact `buildctl build` command](#6-your-exact-buildctl-build-command)
7. [Flag-by-flag reference](#7-flag-by-flag-reference)
8. [Nixpacks → Dockerfile → BuildKit](#8-nixpacks--dockerfile--buildkit)
9. [Registry push and two hostnames](#9-registry-push-and-two-hostnames)
10. [Build network isolation](#10-build-network-isolation)
11. [Layer cache and parallelism](#11-layer-cache-and-parallelism)
12. [Logs: what you see in the UI](#12-logs-what-you-see-in-the-ui)
13. [Permissions and sockets](#13-permissions-and-sockets)
14. [End-to-end sequence](#14-end-to-end-sequence)
15. [Glossary](#15-glossary)

---

## 1. Why not `docker build`?

A **container image** is a stack of read-only **layers** plus metadata (entrypoint, env, exposed ports). Something has to:

1. Run each Dockerfile step (`RUN npm install`, `COPY . .`, …)
2. Commit each step as a layer
3. Optionally **push** those layers to a **registry**

`docker build` does this inside **Docker Engine** (`dockerd`). That works, but for a deployment platform you often want:

| Need | `docker build` | BuildKit + buildctl |
|------|----------------|---------------------|
| Push image without loading a huge tarball on the client | Awkward (`docker build` then `docker push`, or buildx) | Native: `--output type=image,push=true` |
| Cache layers across builds | Yes, on the daemon | Yes, dedicated build daemon |
| Isolate build network from DB/NATS | Harder to control | `--opt network=...` per build |
| Run builds as non-root in worker | Worker would need docker.sock | Worker only needs **buildkit** socket |
| Stream structured build output | Plain text | buildctl streams progress to stdout |

This project uses **Moby BuildKit** as the builder and **buildctl** as the thin client. The worker never calls `docker build`.

---

## 2. The client–daemon split

Same pattern as Docker itself:

```
┌─────────────────┐         gRPC over Unix socket          ┌─────────────────┐
│  buildctl       │  ───────────────────────────────────►  │  buildkitd      │
│  (in worker)    │         BUILDKIT_HOST                  │  (buildkit svc) │
│  CLI client     │  ◄───────────────────────────────────  │  does real work │
└─────────────────┘         logs + status                └─────────────────┘
```

| Piece | Where it runs | Role |
|-------|---------------|------|
| **buildctl** | `worker` container | Parses CLI, sends “solve this Dockerfile and push here” |
| **buildkitd** | `buildkit` container | Pulls base images, runs `RUN` in containers, writes layers, pushes to registry |

They are **not** on the same machine in the abstract sense—they are **two containers** that share a **host directory** mounted as `/var/run/buildkit`, so the socket file is visible to both.

**Analogy:** `docker` CLI ↔ `dockerd`. **buildctl** ↔ **buildkitd**.

---

## 3. BuildKit (`buildkitd`)

**BuildKit** is Moby’s next-generation build engine (used under the hood by `docker buildx`). The long-running process is **`buildkitd`**.

In `docker-compose.yml`:

```yaml
buildkit:
  image: moby/buildkit
  privileged: true
  command: ["--addr", "unix:///run/buildkit/buildkitd.sock", "--config", "/etc/buildkit/buildkitd.toml"]
  volumes:
    - /var/run/buildkit:/var/run/buildkit   # host path → socket + state
```

### What buildkitd does during a build

1. **Receives** a solve request from buildctl (frontend = Dockerfile, context paths, output spec).
2. **Loads** the Dockerfile and builds a **directed acyclic graph (DAG)** of operations (each `RUN`, `COPY`, etc. is a vertex).
3. **Executes** steps—often each `RUN` runs in a short-lived container using the **OCI worker**.
4. **Caches** layer blobs keyed by step inputs (file hashes, command string, base image digest).
5. **Exports** the final image—here, **directly to the registry** (`push=true`), without sending the full image back to buildctl.

### `infra/buildkitd.toml`

```toml
[grpc]
address = ["unix:///run/buildkit/buildkitd.sock"]
socketMode = "0666"

[worker.oci]
enabled = true
snapshotter = "overlayfs"
max-parallelism = 8

[registry."registry:5000"]
http = true
insecure = true
```

| Section | Meaning |
|---------|---------|
| `[grpc]` | Listen on Unix socket; `0666` so non-root `appuser` in worker can connect (see [Permissions](#13-permissions-and-sockets)). |
| `[worker.oci]` | Use normal Linux containers + overlayfs for filesystem snapshots during steps. `max-parallelism = 8` caps concurrent steps inside one build. |
| `[registry."registry:5000"]` | Allow **insecure HTTP** push/pull to the local registry hostname used on `build-net`. Without this, BuildKit refuses non-TLS registries. |

`privileged: true` on the service is required so buildkitd can create nested containers and mount filesystems for build steps.

---

## 4. buildctl

**buildctl** is the command-line client shipped with BuildKit (installed in the worker image from the [Moby buildkit release](https://github.com/moby/buildkit/releases) tarball).

The worker spawns it like any subprocess:

```rust
let mut buildctl = Command::new("buildctl");
buildctl.args([ /* see section 6 */ ]);
buildctl.current_dir(work_dir);
buildctl.env("BUILDKIT_HOST", "unix:///var/run/buildkit/buildkitd.sock");
```

buildctl does **not** compile your app itself. It only:

- Reads local paths (`--local context`, `--local dockerfile`)
- Sends a **LLB** (low-level build) solve to buildkitd via the frontend `dockerfile.v0`
- Streams **progress lines** to stdout/stderr (which the worker forwards to NATS as build logs)

---

## 5. The Unix socket and `BUILDKIT_HOST`

**Unix domain socket:** a file on disk used for IPC between processes on the same host (here, between containers via a **bind-mounted directory**).

Host layout:

```
/var/run/buildkit/buildkitd.sock   ← created by buildkitd
```

Both containers mount:

```yaml
volumes:
  - /var/run/buildkit:/var/run/buildkit
```

Worker env:

```
BUILDKIT_HOST=unix:///var/run/buildkit/buildkitd.sock
```

buildctl reads `BUILDKIT_HOST` (or `--addr`) to know where to dial. Protocol is **gRPC** over that socket—not HTTP.

**Important:** This is **not** “BuildKit on another server.” It is a **separate container** on the same Docker host sharing one socket file.

---

## 6. Your exact `buildctl build` command

After Nixpacks writes `.nixpacks/Dockerfile`, the worker runs (conceptually):

```bash
cd /tmp/builds/{deployment_id}

buildctl build \
  --frontend dockerfile.v0 \
  --local context=. \
  --local dockerfile=.nixpacks \
  --opt network=vercel-clone_build-net \
  --output type=image,name=registry:5000/deployment-{uuid}:latest,push=true
```

Environment:

- `BUILDKIT_HOST=unix:///var/run/buildkit/buildkitd.sock`
- `HOME=/home/appuser` (needed when worker runs as non-root)

On success, the function returns the **serve** tag (different hostname):

```
localhost:5000/deployment-{uuid}:latest
```

That string is stored in Postgres and used later by `docker run` on the **host** daemon.

---

## 7. Flag-by-flag reference

### `buildctl build`

Top-level subcommand: “solve one build definition and produce the requested output.”

### `--frontend dockerfile.v0`

A **frontend** is a plugin that turns a high-level input into BuildKit’s internal graph (LLB).

| Frontend | Input |
|----------|--------|
| `dockerfile.v0` | Classic Dockerfile + build context |

Other frontends exist (e.g. `gateway.v0` for custom builders); this project only uses the Dockerfile frontend.

### `--local context=.`

**Local sources** are directories buildctl **uploads** (or exposes via session) to buildkitd before the solve starts.

| Name | Path (relative to `current_dir`) | Used for |
|------|----------------------------------|----------|
| `context` | `.` (repo root after `git clone`) | `COPY`, `.dockerignore`, files referenced in Dockerfile |
| `dockerfile` | `.nixpacks` | Directory containing the Dockerfile (see below) |

BuildKit convention: the dockerfile local is a **directory**; it looks for a file named `Dockerfile` inside `.nixpacks/`.

### `--opt network=vercel-clone_build-net`

**Build-time network** for `RUN` instructions inside the build containers.

- Value comes from `BUILD_NETWORK` in compose.
- Attaches build steps to **`build-net`** only, so `npm install` cannot reach `postgres:5432` or `nats:4222` on `default`.
- Registry is reachable as `registry:5000` because **buildkit** and **registry** are on `build-net`.

This is **not** the network your preview app uses at runtime (`serve-net`).

### `--output type=image,name=...,push=true`

**Exporter** defines what happens to the final image.

| Part | Meaning |
|------|---------|
| `type=image` | Produce an OCI/Docker image (manifest + config + layers). |
| `name=registry:5000/deployment-{id}:latest` | Image reference used for **push** (must match `buildkitd.toml` registry config). |
| `push=true` | Upload layers to the registry from **buildkitd**; do not return image tarball to buildctl. |

Why `push=true` matters (from code comments):

> Avoid sending the image tarball back to the client (which hangs on large images).

Without push, buildctl would wait for a potentially multi-GB stream over the socket—slow and memory-heavy for the worker.

Alternative outputs (not used here):

- `type=docker` — load into local dockerd
- `type=oci,dest=...` — write to disk
- `type=registry` — variant focused on registry export

---

## 8. Nixpacks → Dockerfile → BuildKit

BuildKit does **not** detect Node vs Rust. **Nixpacks** runs first:

```bash
nixpacks build -o . --env NIXPACKS_NODE_VERSION=22 --install-cmd "bun install" .
```

Outputs (under work dir):

```
.nixpacks/
  Dockerfile      ← consumed by buildctl
  (plan metadata)
```

Then buildctl only builds that Dockerfile. Separation of concerns:

| Tool | Responsibility |
|------|----------------|
| **git** | Source at exact commit |
| **Nixpacks** | “What base image, install, build, start command?” |
| **buildctl + buildkitd** | “Execute Dockerfile, cache layers, push image” |

### Dockerfile concepts (what BuildKit executes)

| Instruction | BuildKit behavior |
|-------------|-------------------|
| `FROM node:22` | Pull base image (metadata + layers); cache key includes digest |
| `RUN bun install` | New layer; runs in container on `build-net` |
| `COPY . .` | New layer; content from `context` local |
| `CMD` / `EXPOSE` | Image config metadata (used when you `docker run` later) |

Log lines like `#10 [stage-0 6/6] RUN bun run build` are BuildKit’s numbered steps—not buildctl parsing your repo.

---

## 9. Registry push and two hostnames

Same physical registry, two DNS names:

| Variable | Example | Who resolves it |
|----------|---------|-----------------|
| `BUILD_REGISTRY_URL` | `registry:5000` | **buildkitd** on `build-net` → push after build |
| `REGISTRY_URL` | `localhost:5000` | **Host dockerd** → pull when API runs preview |

```rust
let build_image_ref = image_tag(build_registry_url, job.deployment_id);
// registry:5000/deployment-{uuid}:latest  → buildctl --output name=...

let serve_image_ref = image_tag(registry_url, job.deployment_id);
// localhost:5000/deployment-{uuid}:latest → returned to API / DB
```

### Registry V2 push (what happens on push)

1. BuildKit creates a **manifest** (list of layer digests + config).
2. For each layer not already in the registry: **POST** blob upload.
3. **PUT** manifest under tag `deployment-{uuid}:latest`.

The worker never holds the full image; only buildkitd talks to `http://registry:5000`.

### Pull on serve (separate step)

When deployment is **Ready**, the API runs `docker run --pull always localhost:5000/deployment-{uuid}:latest`. The **host** Docker daemon pulls from `localhost:5000` (port published from the registry container). “Ready” in the DB does not imply the image is already on the host.

---

## 10. Build network isolation

```
                    ┌──────────── default ────────────┐
                    │  db, nats, api, minio, ...       │
                    └──────────────────────────────────┘

┌──────────── build-net ────────────┐
│  worker, buildkit, registry        │
│  RUN steps during build see this   │
└────────────────────────────────────┘

┌──────────── serve-net ─────────────┐
│  traefik, api, preview containers  │
└────────────────────────────────────┘
```

`--opt network=vercel-clone_build-net` limits **build-time** egress. It does not configure the preview container’s network (that is `docker run --network serve-net` in the API).

---

## 11. Layer cache and parallelism

### Layer cache

Each Dockerfile step produces a **layer** identified by:

- Parent layer ID
- Instruction text
- Content hashes for `COPY` sources
- Build args / env that affect the step

If inputs match a previous build, BuildKit **reuses** the cached layer and logs `CACHED` instead of re-running.

Cache is stored in buildkitd’s state (under its data dir on the volume), not in the worker’s tmpfs work dir (wiped per job).

### Parallelism

- **Across deployments:** `MAX_CONCURRENT_BUILDS` in the worker (semaphore).
- **Inside one build:** `max-parallelism = 8` in `buildkitd.toml` (independent DAG nodes can run in parallel when safe).

---

## 12. Logs: what you see in the UI

The worker wraps buildctl in `run_logged_command`:

- stdout/stderr lines → NATS `build.logs.{deployment_id}` → API → Postgres `build_log_lines` + SSE

Typical lines:

```
[stderr] #1 [internal] load build definition from Dockerfile
[stderr] #5 [stage-0 2/6] RUN bun install
[stderr] #5 DONE 12.3s
```

These are **BuildKit progress events** formatted for humans, not Rust `println!` from your code.

---

## 13. Permissions and sockets

### buildkitd.sock

Worker runs as **appuser** (uid 1001). Socket must be readable/writable:

- `socketMode = "0666"` in `buildkitd.toml`, and/or
- `infra/docker-entrypoint.sh` chmod on worker start

Error if broken:

```
dial unix /var/run/buildkit/buildkitd.sock: permission denied
```

### `HOME=/home/appuser`

buildctl (and buildkitd when resolving client metadata) may need a home directory. BuildKit running as uid 1001 inside nested containers failed with:

```
mkdir /home/appuser: permission denied
```

Mitigation: `infra/buildkit-entrypoint.sh` creates `/home/appuser` before `exec buildkitd`.

### Worker does not need `docker.sock`

Only **buildkit** socket for builds. The API container needs **docker.sock** for `docker run` previews—not the worker.

---

## 14. End-to-end sequence

```
1. API publishes BuildJob → NATS
2. Worker: git clone → /tmp/builds/{id}
3. Worker: nixpacks build → .nixpacks/Dockerfile
4. Worker: spawn buildctl build
      │
      ├─► buildctl reads context + dockerfile locals
      ├─► gRPC to buildkitd (unix socket)
      │
5. buildkitd:
      ├─► pull base images (e.g. from Docker Hub)
      ├─► RUN/COPY on build-net
      ├─► push layers → registry:5000/deployment-{id}:latest
      │
6. buildctl exits 0
7. Worker publishes BuildResult { Ready, image_ref: localhost:5000/... }
8. Worker acks NATS job
9. API: docker run --pull always localhost:5000/deployment-{id}:latest
10. Traefik routes {hash}-preview.localhost → container
```

---

## 15. Glossary

| Term | Definition |
|------|------------|
| **OCI image** | Open Container Initiative format: config JSON + ordered layer tarballs + manifest. |
| **Layer** | Filesystem diff from one Dockerfile step; immutable once built. |
| **Manifest** | Index pointing at layer digests and image config; what a **tag** like `:latest` resolves to. |
| **Registry** | HTTP API storing manifests and blobs (`registry:2` in this project). |
| **buildkitd** | BuildKit daemon that executes solves and caches layers. |
| **buildctl** | CLI client that submits solves and streams logs. |
| **Frontend** | Adapter (e.g. `dockerfile.v0`) from Dockerfile to internal LLB graph. |
| **LLB** | Low-Level Build: BuildKit’s internal DAG of operations. |
| **Solve** | One complete build request from frontend + locals to exporters. |
| **Exporter** | Final step: image to registry, tar to disk, etc. (`--output`). |
| **Local source** | Directory buildctl attaches as named input (`context`, `dockerfile`). |
| **BUILDKIT_HOST** | Address of buildkitd (`unix://...` or `tcp://...`). |
| **DAG** | Directed acyclic graph of build steps; enables caching and parallelism. |
| **Snapshotter** | How buildkitd stores step filesystems (`overlayfs` here). |

---

## Further reading

- [DOCS.md](../DOCS.md) — Full platform documentation
- [BuildKit README](https://github.com/moby/buildkit/blob/master/README.md)
- [buildctl build reference](https://github.com/moby/buildkit/blob/master/docs/reference/buildctl.md)
- [Dockerfile frontend](https://github.com/moby/buildkit/blob/master/docs/reference/dockerfile-frontend.md)
