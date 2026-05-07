# Architecture

State captured: 2026-05-07.

## Monorepo Layout

```text
depo/
  apps/
    web/                  SvelteKit frontend copied from the existing web app
  crates/
    depo-core/            Rust Git, repository, storage, and validation primitives
  packages/
    api-client/           TypeScript client used by the web app
  services/
    api/                  Rust Axum HTTP API and Git smart HTTP adapter
```

Root orchestration is intentionally simple:

- `Cargo.toml` defines the Rust workspace.
- `pnpm-workspace.yaml` defines the TypeScript workspace.
- `package.json` provides local dev, build, check, and test scripts.

There is no task graph framework yet because the current repo does not need one.

## Layer Boundaries

Depo keeps these layers distinct:

```text
Git Core
  repositories, refs, commits, trees, blobs, diffs

Product API
  repositories, commits, projections, auth, future reviews/checks/jobs

Interface
  repository browser, file viewer, commit list, future diff/review/log surfaces

Runner
  future job claiming, checkout, execution, streaming, cancellation

Deployment
  future packaging, service lifecycle, updates, backups
```

Current implementation ownership:

- `crates/depo-core` owns Git identity validation, repo path validation, branch/ref/SHA validation, bare repo lifecycle, Git command execution, commit construction, tree reads, blob reads, branch listing, and commit summaries.
- `services/api` owns HTTP routing, SQLite migrations, repository metadata persistence, REST DTOs, auth verification for Git smart HTTP, and the `git http-backend` adapter.
- `packages/api-client` wraps the implemented API surface for TypeScript consumers.
- `apps/web` owns UI rendering and server-side data loading through the API client.

## Current Request Flow

Repository browser first paint:

```text
browser route /{owner}/{repo}
  -> SvelteKit server load
  -> DepoClient.repos.view(owner, repo, { ref, path })
  -> GET /api/v1/repos/{owner}/{repo}/view
  -> API resolves repository metadata from SQLite
  -> depo-core resolves Git ref and reads tree/blob/commits from bare repo
  -> web app renders tree and selected file content
```

Git push flow:

```text
git push http://git:token@host/{owner}/{repo}.git main
  -> Axum fallback route
  -> parse {owner}/{repo}.git smart HTTP target
  -> authenticate before repository lookup
  -> verify SQLite storage_path matches configured storage root
  -> run git http-backend with explicit CGI environment
  -> Git updates the same bare repo that the web API reads
```

## Implemented Git Core

Implemented:

- Path-safe `StorageRoot`.
- `RepoOwner`, `RepoName`, `RepoId`.
- `BranchName`, `GitSha`, `ValidatedRef`.
- `RepoFilePath` for file and tree paths.
- Git process wrapper with argument arrays, stdin support, stdout/stderr capture, status capture, signal capture on Unix, and timeouts.
- Bare repository creation and opening.
- Server-side commit creation with `commit-tree` and atomic `update-ref`.
- Tree listing through `ls-tree`.
- Blob reading through `cat-file`.
- Branch listing through `for-each-ref`.
- Recent commit summaries through `git log`.

Not implemented:

- Structured diff generation.
- Commit detail loading.
- Tag APIs.
- Delete repository.
- In-process Git implementation through `gitoxide`.

## Implemented Product API

Implemented REST endpoints:

- `GET /health`
- `POST /api/v1/repos`
- `GET /api/v1/repos`
- `GET /api/v1/repos/{owner}/{repo}`
- `POST /api/v1/repos/{owner}/{repo}/commits`
- `GET /api/v1/repos/{owner}/{repo}/tree`
- `GET /api/v1/repos/{owner}/{repo}/blob`
- `GET /api/v1/repos/{owner}/{repo}/commits`
- `GET /api/v1/repos/{owner}/{repo}/view`

Implemented Git protocol endpoints:

- `GET /{owner}/{repo}.git/info/refs?service=git-upload-pack`
- `POST /{owner}/{repo}.git/git-upload-pack`
- `GET /{owner}/{repo}.git/info/refs?service=git-receive-pack`
- `POST /{owner}/{repo}.git/git-receive-pack`

## Implemented Interface

Implemented:

- Root repository list.
- Root repository creation form.
- Repository code route.
- Recursive repository tree.
- File links with `path` query parameters.
- Auto-open README when available.
- Multi-tab file browser state.
- Text file viewer with line numbers.
- Commit list page using real Git commit summaries.

Existing but not fully wired to backend behavior:

- `DiffViewer.svelte` uses `@pierre/diffs`, but there is no structured diff API yet.
- File header diff controls exist for diff mode, but repository code mode currently renders source files.

## Runner And Deployment

Runner and CI execution are not implemented. No repository-controlled code is executed by Depo outside Git commands.

Deployment automation is also not implemented in this repository yet. Local development is supported; production service lifecycle, backups, upgrades, and Pi deployment need a dedicated slice.
