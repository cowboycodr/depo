# Depo Architecture

## Overview

Depo is a self-hosted code forge designed to run anywhere—from a single machine to distributed infrastructure—with low latency, robustness, and stability as core goals. It provides Git repository hosting, code review surfaces, CI, and live log streaming without requiring external accounts, telemetry, or monthly bills.

This document describes the broad monorepo architecture and early design direction. The focused, current source of truth now lives in [`docs/`](./docs/README.md), especially for implemented API contracts, storage behavior, security assumptions, web app behavior, and next work.

---

## 0. Build Stance

Depo should be built as a continuous core spine, not as a slow phased roadmap. LLM-assisted development changes implementation speed, but it does not remove the need for clean boundaries. The working rule is:

```text
Move fast by making the real boundaries real early.
```

The first implementation pass should make repository storage, metadata, API contracts, and file reads real enough for the frontend to use. Avoid fake production paths, mock data hidden behind real endpoints, or UI surfaces that imply behavior the backend cannot perform.

The core spine now in the repository is:

```text
scaffold workspace
  -> depo-core Git/storage primitives
  -> SQLite repository metadata
  -> repo create/list/get APIs
  -> commit builder API
  -> tree/blob/read projections for the frontend
  -> authenticated Git smart-HTTP clone/fetch/push
```

The commit builder and read APIs proved the storage model, repository lifecycle, refs, commits, trees, and file rendering before the Git protocol surface was added. Git smart-HTTP now uses the same bare repository storage, so `git clone`, `git fetch`, and `git push` update the data the web app reads.

---

## 1. Monorepo Layout

```
depo/
├── package.json              # Root orchestration scripts
├── pnpm-workspace.yaml       # JS/TS workspace definition
├── Cargo.toml                # Rust workspace definition
├── justfile                  # Cross-language task runner (optional)
├── README.md                 # Project overview
├── AGENTS.md                 # Engineering standards
│
├── apps/
│   └── web/                  # SvelteKit frontend
│       ├── package.json
│       ├── src/
│       └── static/
│
├── packages/
│   └── api-client/           # TypeScript SDK for consumers
│       ├── package.json
│       ├── src/
│       │   ├── index.ts      # GitStorage class
│       │   ├── auth.ts       # Token generation
│       │   └── types.ts      # API contracts
│       └── tsconfig.json
│
├── services/
│   └── api/                  # Rust HTTP API and Git smart-HTTP adapter
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs       # Axum server bootstrap
│       │   ├── api/          # REST endpoint handlers
│       │   ├── git_http.rs   # Git smart-HTTP CGI adapter
│       │   ├── auth.rs       # Git credential verification, JWT scopes
│       │   ├── db/           # Schema, migrations, queries
│       │   └── config.rs     # Environment + runtime config
│       └── migrations/       # SQLx migrations
│
└── crates/
    └── depo-core/            # Shared Rust library
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── git/          # Repository operations, path validation
            ├── models/       # Domain types (Repo, Commit, Ref)
            └── crypto/       # JWT parsing, key management
```

### Rationale

- **pnpm workspaces** manage the JavaScript/TypeScript side (frontend + SDK).
- **Cargo workspaces** manage the Rust side (API service + shared core library).
- **No Turborepo/Nx on day one.** Per `AGENTS.md`, broad abstractions are not added before a second real use case exists. Root `package.json` scripts with `concurrently` are sufficient.
- **`depo-core`** exists as a separate crate so future services (runner, webhook worker) can depend on Git primitives and domain models without importing HTTP handlers.

---

## 2. Technology Stack

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Frontend | SvelteKit 5 + Tailwind 4 | Existing, proven density and precision for code surfaces. |
| API Client | TypeScript, published to npm | Mirrors the `@pierre/storage` ergonomics—easy for consumers and for our own frontend. |
| Backend | Rust + Tokio + Axum | Zero-GC latency, memory safety, single static binary. The borrow checker enforces correctness at compile time. |
| Git Engine | `git` binary (initial) → `gitoxide` (future) | Shelling out is acceptable per `AGENTS.md` if wrapped behind a clear Git service boundary with safe argument passing, input validation, and precise error capture. We migrate to `gitoxide` once behavior is proven. |
| Database | SQLite (WAL mode) + `sqlx` | Self-hosting by default. `sqlx` provides compile-time query checking. PostgreSQL path remains open. |
| Auth | ES256 JWT, customer-signed | Repo-scoped, TTL-controlled tokens. No central API key leak surface. |

### Why Rust for the backend

- **Predictable latency:** No garbage collection pauses under high concurrency.
- **Battle-tested async:** Tokio is the standard for high-throughput I/O.
- **Native Git options:** `gitoxide` is a pure-Rust Git implementation that gives in-process content-addressed storage, packfile parsing, and ref management.
- **Deployment:** A single static binary with no runtime dependencies. Runs identically on a Raspberry Pi 5 or a fleet of servers.

### Why SQLite first

Per `AGENTS.md`:

> "SQLite is acceptable as the initial metadata store if migrations, constraints, and access patterns are treated seriously."

SQLite is the right starting point because:
- Self-hosting is a core product value. Requiring PostgreSQL on day one raises the barrier to entry.
- The metadata layer (projects, permissions, sync state) fits SQLite's concurrency model well, especially with WAL mode.
- The entire backend runs on a single machine without additional infrastructure.

**Conditions for using SQLite seriously:**
1. Schema-first with migrations from day one.
2. Typed query boundaries via `sqlx` (compile-time checked queries).
3. Access patterns documented and intentional.
4. Migration path to PostgreSQL remains open. The data access layer is designed behind trait boundaries so the backend can be swapped later.

**When to switch to PostgreSQL:**
- Multi-node replication or high-availability failover is needed.
- Query patterns exceed SQLite's writer serialization.
- Row-level security or advanced features are required.

---

## 3. Architecture Boundaries

These layers must remain distinct. Do not let UI concerns shape Git storage. Do not let Git command output leak directly into frontend contracts.

```
Git Core (crates/depo-core)
  ├── Repository path validation
  ├── Ref/SHA sanitization
  ├── Packfile receive/send
  ├── Commit tree construction
  └── Bare repo lifecycle

Product API (services/api)
  ├── Repo CRUD endpoints
  ├── Commit builder API
  ├── Branch/merge operations
  ├── Permission enforcement
  └── Pagination, caching, errors

Interface (apps/web)
  ├── Repository browser
  ├── Diff viewer (uses @pierre/diffs)
  ├── File tree, nav, settings
  └── Stable data contracts only

Runner (future)
  ├── Job claiming
  ├── Checkout + execution
  ├── Log streaming
  └── Status reporting
```

---

## 4. Authentication Model

Production access uses JWT tokens signed by the instance owner. Each token:
- Grants access to a **single repository** (except `org:read` tokens, which are org-wide).
- Contains **explicit permission scopes**.
- Has a **configurable time-to-live (TTL)**.
- Is **customer-signed** for full control.

The API also has an explicit `DEPO_AUTH_MODE=local` development mode. Local mode is intentionally not a production trust model: Git smart-HTTP still requires HTTP credentials, but accepts any non-empty token supplied as `git:{token}`. This keeps local Git clients exercising the real auth challenge and Basic credential path without pretending a local token is cryptographically verified.

### Token Structure

```json
{
  "iss": "depo-instance-id",
  "sub": "user-or-agent-id",
  "repo": "owner/repo-name",
  "scopes": ["git:read", "git:write"],
  "iat": 1723453189,
  "exp": 1723456789
}
```

### Permission Scopes

| Scope | Description | Operations |
|-------|-------------|------------|
| `git:read` | Read repository contents | clone, fetch, pull, list files, read commits |
| `git:write` | Modify repository | push (includes read), create commits, delete branches |
| `repo:write` | Create repositories | `POST /api/v1/repos` |
| `org:read` | List repositories | `GET /api/v1/repos` (omit `repo` claim) |

### Key Management

- Admin generates an ES256 keypair via CLI: `depo admin key generate`.
- Public key stored in instance configuration (e.g., `~/.depo/authorized_keys` or environment).
- Private key held by the instance owner. Tokens are minted locally or via the SDK.
- API verifies tokens locally using the stored public key. No external auth provider required.

Implemented configuration:

| Mode | Required config | Behavior |
|------|-----------------|----------|
| `DEPO_AUTH_MODE=local` | none | Git smart-HTTP requires `Basic` auth with username `git` and any non-empty password. |
| `DEPO_AUTH_MODE=jwt` | `DEPO_AUTH_PUBLIC_KEY_PEM` or `DEPO_AUTH_PUBLIC_KEY_PATH` | Git smart-HTTP verifies ES256 JWTs, checks `repo`, and enforces `git:read`/`git:write`. |

### Git Remote Format

```
https://git:{jwt}@host/{owner}/{repo}.git
```

- Username is always `git`.
- Password is the JWT.
- The `repo` claim in the JWT must match the repository path.
- HTTP API routes still identify repositories explicitly with `{owner}/{repo}` path segments. Path identity says what resource is being touched; auth says whether the caller can touch it. This keeps logs, browser URLs, CLI calls, and self-hosted debugging clear.

---

## 5. API Surface

The API exposes durable Git primitives and frontend-optimized read projections over plain HTTPS. Base URL:

```
/api/v1
```

Repository identity is explicit:

```text
/api/v1/repos/{owner}/{repo}
```

Primitive endpoints exist for correctness. Projection endpoints exist for speed.

### Repositories

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/repos` | Create repository |
| `GET` | `/repos` | List repositories (cursor pagination) |
| `GET` | `/repos/{owner}/{repo}` | Get repository metadata |
| `DELETE` | `/repos/{owner}/{repo}` | Delete repository |

Create repository request:

```json
{
  "owner": "kian",
  "name": "depo",
  "defaultBranch": "main"
}
```

Create repository response:

```json
{
  "repo": {
    "id": "kian/depo",
    "owner": "kian",
    "name": "depo",
    "defaultBranch": "main",
    "createdAt": "2026-05-06T18:22:00Z",
    "updatedAt": "2026-05-06T18:22:00Z"
  }
}
```

### Git Protocol (HTTPS)

Standard Git commands over HTTPS with JWT authentication:

```bash
git clone https://git:JWT@host/owner/repo.git
git push origin main
git fetch origin
```

Local development uses the same credential shape:

```bash
git clone http://git:local@127.0.0.1:3847/owner/repo.git
git fetch origin
git push origin main
```

Implemented smart-HTTP endpoints:

| Method | Endpoint | Service |
|--------|----------|---------|
| `GET` | `/{owner}/{repo}.git/info/refs?service=git-upload-pack` | clone, fetch, pull, ls-remote |
| `POST` | `/{owner}/{repo}.git/git-upload-pack` | upload-pack negotiation |
| `GET` | `/{owner}/{repo}.git/info/refs?service=git-receive-pack` | push discovery |
| `POST` | `/{owner}/{repo}.git/git-receive-pack` | receive-pack push |

The adapter validates `{owner}` and `{repo}` through the same Depo ID types as the REST API, requires credentials before repository lookup, confirms the SQLite metadata record points at the configured storage root, and invokes `git http-backend` with argument arrays, explicit CGI environment, `GIT_PROJECT_ROOT`, `GIT_HTTP_EXPORT_ALL`, `REMOTE_USER`, and a timeout.

### Read APIs

The granular current API contract is documented in [`docs/api.md`](./docs/api.md). The table below includes both implemented read endpoints and designed near-term endpoints.

| Method | Endpoint | Status | Description |
|--------|----------|--------|-------------|
| `GET` | `/repos/{owner}/{repo}/tree?ref={ref}&path={path}` | Implemented | List tree entries at a path |
| `GET` | `/repos/{owner}/{repo}/blob?ref={ref}&path={path}` | Implemented | Get file metadata and content |
| `GET` | `/repos/{owner}/{repo}/commits?ref={ref}` | Implemented | List commit history |
| `GET` | `/repos/{owner}/{repo}/commits/{sha}` | Designed next | Get commit metadata |
| `GET` | `/repos/{owner}/{repo}/diff?base={sha}&head={sha}` | Designed next | Get diff between refs |

Text blobs return actual source code inline when they are below the configured inline size limit:

```json
{
  "path": "src/main.rs",
  "kind": "text",
  "language": "rust",
  "mode": "100644",
  "size": 921,
  "encoding": "utf-8",
  "content": "fn main() {\n    println!(\"hello depo\");\n}\n",
  "commitSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
  "etag": "\"blob-8f12c3b-src-main-rs\""
}
```

Large text files return a bounded preview plus a separate stream/download URL. Binary files return metadata and a download URL, not base64 embedded in repo-view responses.

### Write API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/repos/{owner}/{repo}/commits` | Commit builder: create commits without local Git |

Commit builder request:

```json
{
  "targetBranch": "main",
  "expectedHeadSha": null,
  "message": "Initial commit",
  "author": {
    "name": "Kian",
    "email": "kian@example.com"
  },
  "changes": [
    {
      "type": "upsert",
      "path": "README.md",
      "contentBase64": "IyBEZXBvCg==",
      "mode": "100644"
    }
  ]
}
```

Commit builder response:

```json
{
  "commit": {
    "sha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
    "treeSha": "b9532c5d5be50d88e2f45d7c229566b2f1f99731",
    "branch": "main"
  },
  "refUpdate": {
    "oldSha": "0000000000000000000000000000000000000000",
    "newSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
    "status": "updated"
  }
}
```

### Branches

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/repos/{owner}/{repo}/branches` | List branches |
| `POST` | `/repos/{owner}/{repo}/branches` | Create branch |
| `DELETE` | `/repos/{owner}/{repo}/branches/{branch}` | Delete branch |

### Frontend Projections

Opening a repository page must not require a chatty waterfall of primitive requests. The API should provide read projections shaped for the first-party UI while keeping the primitive endpoints available for SDK, CLI, and automation.

Repo browser first paint:

```http
GET /api/v1/repos/{owner}/{repo}/view?ref=main&path=README.md
```

Response:

```json
{
  "repo": {
    "id": "kian/depo",
    "owner": "kian",
    "name": "depo",
    "defaultBranch": "main"
  },
  "ref": {
    "name": "main",
    "kind": "branch",
    "commitSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2"
  },
  "branches": {
    "defaultBranch": "main",
    "items": [
      {
        "name": "main",
        "headSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2"
      }
    ]
  },
  "tree": {
    "nodes": [
      {
        "path": "README.md",
        "name": "README.md",
        "kind": "file",
        "mode": "100644",
        "size": 128
      },
      {
        "path": "src",
        "name": "src",
        "kind": "directory"
      },
      {
        "path": "src/main.rs",
        "name": "main.rs",
        "kind": "file",
        "mode": "100644",
        "size": 921
      }
    ]
  },
  "activeFile": {
    "path": "README.md",
    "kind": "text",
    "language": "markdown",
    "mode": "100644",
    "size": 128,
    "encoding": "utf-8",
    "content": "# Depo\n",
    "commitSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2"
  },
  "recentCommits": [
    {
      "sha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
      "title": "Initial commit",
      "author": {
        "name": "Kian",
        "email": "kian@example.com"
      },
      "committedAt": "2026-05-06T20:15:00Z"
    }
  ]
}
```

Compare view first paint:

```http
GET /api/v1/repos/{owner}/{repo}/compare-view?base=main&head=feature/sidebar
```

This response should include compare metadata, changed files, summary stats, commit metadata, and the first page of patches. Large patches paginate by file or hunk instead of blocking the whole page.

Frontend performance rules:

- First meaningful repo browser screen should be one request.
- First meaningful compare screen should be one request.
- Heavy data is paged: commits, large trees, large diffs, logs.
- Every cacheable read response includes an `etag` or content hash.
- `If-None-Match` is supported so the frontend can revalidate cheaply.
- Tree and blob responses include `commitSha` so frontend caching is exact.
- The frontend never parses raw Git output.
- SSE/WebSocket is reserved for streaming behavior: runner logs, job state, live pushes.

### Pagination

Cursor-based pagination for all list endpoints:
- `cursor` — opaque string from previous response.
- `limit` — results per page (default: 20, max: 100).
- Response includes `next_cursor` and `has_more`.

### Error Handling

All errors follow a consistent shape:

```json
{
  "error": {
    "code": "repo_not_found",
    "message": "Repository kian/depo does not exist.",
    "details": {
      "owner": "kian",
      "repo": "depo"
    }
  }
}
```

Clients branch on HTTP status codes and stable `error.code` values, not message strings.

### SDK Shape

The TypeScript SDK should wrap the HTTP API without hiding the resource model:

```ts
const depo = new DepoClient({ baseUrl, token });

const repo = await depo.repos.create({
  owner: "kian",
  name: "depo",
  defaultBranch: "main"
});

await repo.createCommit({
  targetBranch: "main",
  message: "Initial commit",
  author: { name: "Kian", email: "kian@example.com" },
  changes: [
    { type: "upsertText", path: "README.md", content: "# Depo\n" }
  ]
});

const view = await repo.view({ ref: "main", path: "README.md" });
```

---

## 6. Data Model

SQLite schema with `sqlx` migrations.

### `repositories`

| Column | Type | Constraints |
|--------|------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` — `owner/repo` format |
| `owner` | `TEXT` | `NOT NULL` |
| `name` | `TEXT` | `NOT NULL` |
| `default_branch` | `TEXT` | `DEFAULT 'main'` |
| `storage_path` | `TEXT` | `NOT NULL` — absolute path to bare repo |
| `created_at` | `DATETIME` | |
| `updated_at` | `DATETIME` | |

### `refs`

| Column | Type | Constraints |
|--------|------|-------------|
| `repo_id` | `TEXT` | `NOT NULL` FK → `repositories(id)` |
| `name` | `TEXT` | `NOT NULL` |
| `sha` | `TEXT` | `NOT NULL` |
| `type` | `TEXT` | `NOT NULL` — `'branch'` or `'tag'` |
| `updated_at` | `DATETIME` | |

**Primary key:** `(repo_id, name, type)`

### `commits` (metadata cache)

Optional in v1. Provides fast commit listing without parsing Git objects for every query.

| Column | Type | Constraints |
|--------|------|-------------|
| `repo_id` | `TEXT` | `NOT NULL` FK → `repositories(id)` |
| `sha` | `TEXT` | `NOT NULL` |
| `message` | `TEXT` | |
| `author_name` | `TEXT` | |
| `author_email` | `TEXT` | |
| `author_date` | `DATETIME` | |
| `parent_shas` | `TEXT` | JSON array |

**Primary key:** `(repo_id, sha)`

### `api_keys`

Instance-level admin keys for token verification.

| Column | Type | Constraints |
|--------|------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` |
| `public_key_pem` | `TEXT` | `NOT NULL` |
| `name` | `TEXT` | |
| `created_at` | `DATETIME` | |

---

## 7. Git Storage Layout

Repositories are stored as **bare repos** on disk. A bare repository contains only Git internals—object database, refs, config, hooks—without a checked-out working tree. This is the standard format for all Git hosting servers.

```
~/.depo/repos/
├── pacificcode/
│   └── depo.git/          ← bare repo
│       ├── HEAD
│       ├── config
│       ├── objects/
│       ├── refs/
│       │   └── heads/
│       │       └── main
│       └── hooks/
└── another-user/
    └── another-repo.git/  ← bare repo
```

All writes happen through:
1. **Git protocol** — `git push` from clients.
2. **Commit builder API** — server constructs Git objects in-memory and updates refs atomically.

The backend never edits files directly. There is no working tree to corrupt or desync.

---

## 8. Development Workflow

### Root `package.json` scripts

```json
{
  "scripts": {
    "dev": "concurrently -n api,web \"pnpm dev:api\" \"pnpm dev:web\"",
    "dev:api": "DEPO_AUTH_MODE=local cargo run -p depo-api",
    "dev:web": "pnpm --filter @depo/web dev",
    "build": "cargo build --workspace && pnpm --filter @depo/api-client build && pnpm --filter @depo/web build",
    "check": "cargo check --workspace && pnpm --filter @depo/api-client build && pnpm --filter @depo/web check",
    "test": "cargo test --workspace"
  }
}
```

### Prerequisites

- Rust stable (1.85+)
- Node.js 22+ and pnpm
- `git` binary on `$PATH` (or configured path)

### Running locally

```bash
# Start the backend in explicit local auth mode
pnpm dev:api

# Start the frontend
pnpm dev:web

# Run compile and SDK checks
pnpm check

# Run tests
pnpm test
```

---

## 9. Design Principles

These principles are derived from `AGENTS.md` and apply to every decision in this codebase.

- **Prefer explicit design over implicit behavior.** Every module has a clear contract.
- **Prefer small, stable modules over broad utility piles.** `depo-core` is the only shared library; everything else is scoped to its service.
- **Prefer typed boundaries over loose object passing.** Rust's type system and TypeScript's strict mode are non-negotiable.
- **Prefer real data models over UI-only state illusions.** The API is the source of truth; the frontend is a view.
- **Prefer durable primitives over feature-specific hacks.** The commit builder and Git protocol are primitives that support many use cases.
- **Prefer boring dependencies with clear ownership over clever fragile code.** Tokio, Axum, sqlx, and SvelteKit are boring and proven.
- **Prefer deletion of bad ideas over layering around them.** If a feature does not fit, remove it.

---

## 10. Current Build Status

The core spine, first web-usable repository flow, and authenticated Git remote flow are implemented:

- Workspace layout exists: root `Cargo.toml`, root `package.json`, `pnpm-workspace.yaml`, `crates/depo-core`, `services/api`, and `packages/api-client`.
- `depo-core` owns repository ID validation, repo file path validation, branch/ref/SHA validation, path-safe bare repo layout, Git command execution with argument arrays, stdin support, and timeouts, bare repo creation, commit construction, direct and recursive tree listing, blob reading, branch listing, and recent commit summaries.
- `services/api` owns SQLite migrations, metadata access for `repositories`, Git smart-HTTP routing, and Git credential verification.
- The API implements `POST /api/v1/repos`, `GET /api/v1/repos`, `GET /api/v1/repos/{owner}/{repo}`, `POST /api/v1/repos/{owner}/{repo}/commits`, `GET /api/v1/repos/{owner}/{repo}/tree`, `GET /api/v1/repos/{owner}/{repo}/blob`, and `GET /api/v1/repos/{owner}/{repo}/view`.
- The Git remote surface implements authenticated smart-HTTP clone/fetch/push at `/{owner}/{repo}.git`.
- `DEPO_AUTH_MODE=jwt` verifies ES256 JWTs for Git smart-HTTP using `DEPO_AUTH_PUBLIC_KEY_PEM` or `DEPO_AUTH_PUBLIC_KEY_PATH`. `DEPO_AUTH_MODE=local` remains an explicit local development mode.
- `/view` proves the frontend read path by returning repository metadata, resolved ref data, branches, recursive tree nodes, actual active file text, and recent commits in one response.
- `packages/api-client` wraps only the working API behavior.
- `apps/web` is copied from the existing standalone SvelteKit UI and wired to real API data with minimal visual changes. The root page lists repositories and creates a repository plus its first `README.md` commit through the API client. Repository pages load `/view`, browse the returned tree with file links, and render actual text blobs in a normal source viewer.

Verification:

- `pnpm run check`
- `pnpm run test`
- `pnpm run build`
- API integration coverage creates a repository, clones it through smart HTTP, fetches a server-side commit, pushes a client-side commit back through receive-pack, pushes an initial branch into an empty repository, and verifies the pushed files through the core repository reader.
- Local smoke: start `depo-api` with `DEPO_AUTH_MODE=local`, start `@depo/web`, submit the root create form, and confirm the rendered repository route shows the README content returned by `/view`.

Near-term remaining work:

- Enforce JWT scopes on the REST API. Git smart-HTTP has JWT verification; the REST endpoints still need the same auth boundary.
- Stream Git smart-HTTP request and response bodies instead of buffering them around `git http-backend`. The current implementation has an explicit `DEPO_GIT_HTTP_BODY_LIMIT` default of 64 MiB and is correct for bounded repos, but large packfiles need streaming before Depo is ready for large production repositories.
- Add pagination/conditional caching to large repository projections. The current `/view` tree is intentionally simple for the first web-usable slice and should not be treated as the final large-repo strategy.

Still intentional non-goals:

- Do not add a runner or CI execution surface yet.
- Do not build UI that implies real Git behavior before the API supports it.
- Do not hide missing auth behind silent fallbacks. A narrow local dev auth mode is acceptable only if it is explicit.

---

*This document is living. Update it when architecture decisions change.*
