# Development

State captured: 2026-05-07.

## Prerequisites

- Rust stable.
- Node.js 22 or newer.
- pnpm 10.30.0.
- `git` on `PATH`.

## Install

```bash
pnpm install
```

## Run Locally

Start the API:

```bash
pnpm dev:api
```

This runs:

```bash
DEPO_AUTH_MODE=local cargo run -p depo-api
```

Start the web app:

```bash
pnpm dev:web
```

Start both:

```bash
pnpm dev
```

## Migration State While Switching Branches

Local development uses `~/.depo/depo.db` by default. SQLite migrations are forward-only for the active checkout. If a feature branch applies a new migration and you then switch back to `main` before that migration exists there, `pnpm dev:api` can fail with:

```text
migration N was previously applied but is missing in the resolved migrations
```

Use one of these options:

- Switch back to the feature branch that owns the migration.
- Run with a separate data directory for throwaway branch testing:

```bash
DEPO_DATA_DIR=/tmp/depo-dev pnpm dev:api
```

- If the default local data can be discarded, remove `~/.depo/depo.db*` and restart the API.

## Environment Variables

API:

| Variable | Default | Notes |
| --- | --- | --- |
| `DEPO_AUTH_MODE` | none | Required. Use `local` or `jwt`. |
| `DEPO_AUTH_PUBLIC_KEY_PEM` | none | Required in JWT mode unless using `DEPO_AUTH_PUBLIC_KEY_PATH`. |
| `DEPO_AUTH_PUBLIC_KEY_PATH` | none | Path to ES256 public key in JWT mode. |
| `DEPO_BIND_ADDR` | `127.0.0.1:3847` | API bind address. |
| `DEPO_DATA_DIR` | `~/.depo` | Data directory containing SQLite DB and repos. |
| `DEPO_DATABASE_URL` | `sqlite://{DEPO_DATA_DIR}/depo.db` | SQLite URL. |
| `DEPO_INLINE_BLOB_LIMIT` | `1048576` | Max inline blob content size in bytes. |
| `DEPO_GIT_HTTP_BODY_LIMIT` | `67108864` | Max buffered Git smart HTTP request body in bytes. |

Web:

| Variable | Default | Notes |
| --- | --- | --- |
| `DEPO_API_ORIGIN` | `http://127.0.0.1:3847` | API origin used by SvelteKit server loads. |

## Checks

Run compile and frontend checks:

```bash
pnpm run check
```

Run tests:

```bash
pnpm run test
```

Run full build:

```bash
pnpm run build
```

Before claiming implementation work complete, run the relevant checks and document any failure.

## Create And Import A Repository

Start the API first:

```bash
pnpm dev:api
```

Create repository metadata:

```bash
curl -X POST http://127.0.0.1:3847/api/v1/repos \
  -H 'content-type: application/json' \
  --data '{"owner":"kian","name":"demo","defaultBranch":"main"}'
```

Push an existing local project into Depo:

```bash
git remote add depo http://git:local@127.0.0.1:3847/kian/demo.git
git push -u depo main
```

Open the web app and visit:

```text
/{owner}/{repo}
```

The repository tree and text file content should come from the pushed Git objects.

## Current Repository Remotes

The project has been published to:

```text
https://github.com/cowboycodr/depo.git
```

This checkout may also have a local Depo remote named `depo`:

```text
http://git:local@127.0.0.1:3847/cowboycodr/depo.git
```

That remote is useful for dogfooding Git smart HTTP, but it is not required for normal development.
