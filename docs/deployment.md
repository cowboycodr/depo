# Deployment

State captured: 2026-05-07.

Deployment is not implemented as a production workflow yet.

## Current Supported Mode

Depo currently supports local development:

```bash
pnpm dev:api
pnpm dev:web
```

The API stores data under `~/.depo` by default and binds to `127.0.0.1:3847`.

## Current Publication

The source repository is on GitHub:

```text
https://github.com/cowboycodr/depo.git
```

The same repository has also been pushed into a local Depo instance for dogfooding Git smart HTTP.

## Production Requirements Not Done Yet

Before Depo is production self-hosted, it needs:

- A release build and packaging story for the Rust API.
- A web build serving story.
- Service lifecycle management, likely systemd or a container.
- Explicit data directory ownership and permissions.
- Backup and restore process for SQLite and bare repositories.
- Upgrade process with migration rollback expectations.
- REST API auth.
- TLS termination.
- Git smart HTTP streaming for large packfiles.
- Observability for API errors and Git operations.
- A documented Pi deployment path if the Raspberry Pi remains the first deployment target.

## Deployment Boundary

Deployment should remain its own layer. The API should not grow runner, updater, or service-manager responsibilities directly. Deployment automation can call the API and manage processes, but it should not leak deployment state into Git storage or repository read contracts.
