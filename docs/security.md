# Security

State captured: 2026-05-07.

Depo is infrastructure. Security assumptions must stay explicit.

## Current Trust Boundary

Implemented authentication applies to Git smart HTTP only.

The REST API currently has no authentication or authorization. This is acceptable only for local development and must be fixed before exposing Depo beyond a trusted local environment.

## Auth Modes

`DEPO_AUTH_MODE` is required.

Local development:

```bash
DEPO_AUTH_MODE=local
```

JWT mode:

```bash
DEPO_AUTH_MODE=jwt
DEPO_AUTH_PUBLIC_KEY_PEM='-----BEGIN PUBLIC KEY-----...'
```

or:

```bash
DEPO_AUTH_MODE=jwt
DEPO_AUTH_PUBLIC_KEY_PATH=/path/to/public.pem
```

Local mode is not a production trust model. It exists so local Git clients still exercise Basic auth and the Git credential challenge path.

## Git Smart HTTP Authorization

Git remote username:

```text
git
```

Scopes:

- `git:read` authorizes clone/fetch/upload-pack.
- `git:write` authorizes push/receive-pack and also satisfies read.

JWT mode checks:

- ES256 signature.
- Required `exp`, `repo`, and `scopes` claims.
- `repo` equals `{owner}/{repo}`.
- Scope grants the requested Git operation.

## Repository-Controlled Code

Depo must not execute arbitrary repository-controlled code except inside the future runner boundary.

Current behavior:

- No runner exists.
- No CI jobs execute.
- Git commands inspect and mutate Git repository state.
- The web app renders source text; it does not execute repository files.

Future runner work must model:

- Trust policy.
- Job lifecycle states.
- Workspace cleanup.
- Cancellation.
- Secret exposure.
- Log streaming.
- Failure retention.

## Input Validation

Current validated inputs:

- Repository owners.
- Repository names.
- Full repository IDs.
- Branch names.
- Full commit SHAs.
- Repository file paths.
- Git smart HTTP paths and services.

Current API errors expose stable codes and bounded detail. Git smart HTTP errors are text/plain because Git clients expect plain protocol-compatible failures.

## Known Security Gaps

- REST API auth is not implemented.
- There is no user, org, or permission model.
- There is no admin key rotation surface.
- There is no CSRF model for the web app.
- There is no rate limiting.
- There is no audit log.
- Git smart HTTP buffers request bodies and responses.
- There is no runner sandbox.
- There is no backup encryption or restore verification.

These are not optional polish items. They are required before production exposure.
