# Depo Architecture

## Overview

Depo is a self-hosted code forge designed to run anywhere—from a single machine to distributed infrastructure—with low latency, robustness, and stability as core goals. It provides Git repository hosting, code review surfaces, CI, and live log streaming without requiring external accounts, telemetry, or monthly bills.

This document describes the monorepo layout, technology stack, architecture boundaries, authentication model, and API surface. It is the source of truth for how the system is built and why.

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
│   └── api/                  # Rust HTTP API + Git smart-HTTP
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs       # Axum server bootstrap
│       │   ├── api/          # REST endpoint handlers
│       │   ├── git/          # Git protocol implementation
│       │   ├── auth/         # JWT verification, scopes
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

All access requires JWT tokens signed by the instance owner. Each token:
- Grants access to a **single repository** (except `org:read` tokens, which are org-wide).
- Contains **explicit permission scopes**.
- Has a **configurable time-to-live (TTL)**.
- Is **customer-signed** for full control.

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

### Git Remote Format

```
https://t:{jwt}@host/{owner}/{repo}.git
```

- Username is always `t` (for token).
- Password is the JWT.
- The `repo` claim in the JWT must match the repository path.

---

## 5. API Surface (Initial Milestone)

The API mirrors the SDK primitives over plain HTTPS. Base URL:

```
https://api.{host}/api/v1
```

### Repositories

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/repos` | Create repository |
| `DELETE` | `/repos/{id}` | Delete repository |
| `GET` | `/repos` | List repositories (cursor pagination) |
| `GET` | `/repos/{id}` | Get repository metadata |

### Git Protocol (HTTPS)

Standard Git commands over HTTPS with JWT authentication:

```bash
git clone https://t:JWT@host/owner/repo.git
git push origin main
git fetch origin
```

### Read APIs

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/repos/{id}/files?ref={branch}` | List files at ref |
| `GET` | `/repos/{id}/file?path={path}&ref={branch}` | Get file content |
| `GET` | `/repos/{id}/commits?ref={branch}` | List commit history |
| `GET` | `/repos/{id}/diff?base={sha}&head={sha}` | Get diff between refs |

### Write API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/repos/{id}/commits` | Commit builder: create commits without local Git |

### Branches

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/repos/{id}/branches` | List branches |
| `POST` | `/repos/{id}/branches` | Create branch |
| `DELETE` | `/repos/{id}/branches/{name}` | Delete branch |

### Pagination

Cursor-based pagination for all list endpoints:
- `cursor` — opaque string from previous response.
- `limit` — results per page (default: 20, max: 100).
- Response includes `next_cursor` and `has_more`.

### Error Handling

All errors follow a consistent shape:

```json
{
  "error": "insufficient permissions"
}
```

Clients branch on HTTP status codes, not message strings.

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
    "dev": "concurrently \"pnpm --filter web dev\" \"cargo run -p api\"",
    "build": "pnpm --filter web build && cargo build --release -p api",
    "check": "pnpm --filter web check && cargo check --workspace",
    "test": "pnpm --filter api-client test && cargo test --workspace",
    "db:migrate": "cd services/api && cargo sqlx migrate run",
    "db:prepare": "cd services/api && cargo sqlx prepare"
  }
}
```

### Prerequisites

- Rust stable (1.85+)
- Node.js 22+ and pnpm
- `git` binary on `$PATH` (or configured path)

### Running locally

```bash
# Start both frontend and backend
pnpm dev

# Run database migrations
cd services/api && cargo sqlx migrate run

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

## 10. Migration from Current State

1. **Move `depo-web` → `apps/web`.** Preserve all source; update workspace imports.
2. **Create `packages/api-client`.** Build the TypeScript SDK from scratch, matching Code Storage's ergonomics.
3. **Create `services/api`.** Rust Axum server with SQLite via sqlx. Implement repo CRUD, Git protocol, and read APIs.
4. **Create `crates/depo-core`.** Shared Git operations, path validation, and domain types.
5. **Wire frontend to real API.** Replace mock data in `DiffViewer`, `RepositoryTree`, and other components with live API calls.
6. **Add root tooling.** pnpm workspace, Cargo workspace, root scripts, and CI configuration.

---

*This document is living. Update it when architecture decisions change.*
