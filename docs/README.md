# Depo Documentation

State captured: 2026-05-07.

Depo is a self-hosted code forge. The current repository already hosts real Git repositories, exposes repository data through a Rust API, and renders that data in the copied SvelteKit web app. These docs capture what exists now, what has been intentionally designed, and what is still not implemented.

## Document Map

- [Product Decisions](./product-decisions.md) records the settled product and build decisions from the early planning work.
- [Architecture](./architecture.md) explains the current monorepo shape and boundaries.
- [API](./api.md) documents the implemented REST API, current response shapes, and planned API surfaces.
- [Git Remote](./git-remote.md) documents clone, fetch, and push over authenticated Git smart HTTP.
- [Storage](./storage.md) documents SQLite metadata, bare repository layout, validation, and Git process boundaries.
- [Security](./security.md) documents current trust assumptions, authentication modes, and unsafe work that is still out of scope.
- [Web App](./web-app.md) documents the copied UI, current routes, and data flow.
- [Development](./development.md) documents local setup, environment variables, commands, and import workflow.
- [Deployment](./deployment.md) documents the current deploy status and what is still required for self-hosting.
- [Next Work](./next-work.md) records the next coherent implementation goals.

## Source Of Truth Rule

The docs in this directory should be treated as the current granular source of truth. The root [ARCHITECTURE.md](../ARCHITECTURE.md) remains useful historical architecture context, but any future implementation work should keep these focused docs current when API contracts, storage behavior, security assumptions, or product boundaries change.

## Current Product Snapshot

Implemented now:

- Create and list repository metadata through the REST API.
- Store repositories as path-safe bare Git repositories on disk.
- Create commits server-side through a commit builder API.
- Read trees, blobs, branch heads, and recent commit summaries from real Git objects.
- Use `/api/v1/repos/{owner}/{repo}/view` for a low-latency repository browser projection.
- Inspect commit metadata and file diffs through commit detail and diff APIs.
- Clone, fetch, and push through authenticated Git smart HTTP at `/{owner}/{repo}.git`.
- Record successful Git smart HTTP branch ref updates as Lands.
- Use the Lands feed as the default repository view.
- Browse imported repositories in the web app, expand the tree, and view actual text file content.
- View a commits list page backed by real Git commit data.
- Open commit detail pages and render text diffs through the existing diff viewer.

Not implemented yet:

- REST API authentication.
- SSH Git remotes.
- Streaming Git smart HTTP packfile bodies.
- Runner, CI execution, checks, logs, and deployment automation.
- Production backup and update workflow.
