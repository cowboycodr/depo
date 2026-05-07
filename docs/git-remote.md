# Git Remote

State captured: 2026-05-07.

Depo supports clone, fetch, and push over Git smart HTTP. The implementation uses the standard `git http-backend` CGI protocol behind the Axum fallback route.

## Remote Formats

Local development:

```bash
http://git:local@127.0.0.1:3847/{owner}/{repo}.git
```

JWT mode:

```bash
https://git:{jwt}@host/{owner}/{repo}.git
```

The username is always `git`. In local mode, the password can be any non-empty token. In JWT mode, the password is an ES256 JWT with a matching `repo` claim and the required Git scope.

SSH syntax is not implemented:

```text
git@host:owner/repo.git
```

## Supported Smart HTTP Paths

```text
GET  /{owner}/{repo}.git/info/refs?service=git-upload-pack
POST /{owner}/{repo}.git/git-upload-pack
GET  /{owner}/{repo}.git/info/refs?service=git-receive-pack
POST /{owner}/{repo}.git/git-receive-pack
```

`git-upload-pack` supports clone, fetch, pull, and ls-remote. `git-receive-pack` supports push.

## Local Usage

Start the API:

```bash
pnpm dev:api
```

Create a repository metadata record first:

```bash
curl -X POST http://127.0.0.1:3847/api/v1/repos \
  -H 'content-type: application/json' \
  --data '{"owner":"kian","name":"demo","defaultBranch":"main"}'
```

Clone:

```bash
git clone http://git:local@127.0.0.1:3847/kian/demo.git
```

Add Depo as a remote to an existing project:

```bash
git remote add depo http://git:local@127.0.0.1:3847/kian/demo.git
git push -u depo main
```

After a push, the web app reads the pushed objects from the same bare repository.

## Authentication

Configuration is explicit:

```bash
DEPO_AUTH_MODE=local
DEPO_AUTH_MODE=jwt
```

Local mode:

- Requires Basic auth.
- Requires username `git`.
- Accepts any non-empty password.
- Exists only to exercise the real Git credential path during local development.

JWT mode:

- Accepts Basic auth with username `git` and JWT password.
- Also accepts Bearer tokens for direct HTTP calls to the Git auth boundary.
- Verifies ES256 with `DEPO_AUTH_PUBLIC_KEY_PEM` or `DEPO_AUTH_PUBLIC_KEY_PATH`.
- Requires `repo` to match `{owner}/{repo}`.
- Requires `git:read` for upload-pack.
- Requires `git:write` for receive-pack.
- Treats `git:write` as sufficient for read.

Current token claim shape:

```json
{
  "sub": "user-or-agent-id",
  "repo": "owner/repo",
  "scopes": ["git:read", "git:write"],
  "exp": 4102444800
}
```

## Safety Properties

The Git smart HTTP adapter:

- Validates `{owner}` and `{repo}` with the same Depo ID types as the REST API.
- Authenticates before repository lookup.
- Confirms the SQLite `storage_path` matches the configured storage root.
- Opens the bare repository before invoking `git http-backend`.
- Uses argument arrays, not shell interpolation.
- Sets explicit CGI environment variables.
- Sets `GIT_CONFIG_NOSYSTEM=1`.
- Runs blocking Git work in `spawn_blocking`.
- Captures stdout and stderr through the Git process wrapper.
- Uses a 120 second timeout for `git http-backend`.

## Current Limits

- Request bodies are buffered before `git http-backend`.
- Responses are buffered before being converted from CGI output to HTTP output.
- `DEPO_GIT_HTTP_BODY_LIMIT` defaults to 64 MiB.
- Large repositories and large packfiles need streaming before production use.
- There is no SSH remote.
- There is no per-user permission model beyond the Git JWT checks.
