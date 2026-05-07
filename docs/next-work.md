# Next Work

State captured: 2026-05-07.

Depo can now receive real pushes and render imported repositories in the web app. The next work should build on that by making pushed commits inspectable.

## Recommended Next Goal

```text
/goal make pushed commits inspectable in Depo: commit detail API, file diffs, and a web commit page using the existing diff viewer
```

## Why This Is Next

The current product can answer:

- What repositories exist?
- What files are in a repository?
- What is inside a selected text file?
- What recent commits exist?
- Can Git clients clone, fetch, and push?

It cannot yet answer the next natural question:

```text
What changed in this commit?
```

Commit inspection is the right next slice because it connects the existing Git remote, commits list, API client, and existing `DiffViewer.svelte` into a real code review primitive.

## Suggested Scope

Backend:

- Add `BareRepository::commit_detail`.
- Add `BareRepository::diff`.
- Validate full commit SHAs and refs.
- Handle root commits.
- Return structured metadata and file patches, not raw Git output as the API contract.

API:

- `GET /api/v1/repos/{owner}/{repo}/commits/{sha}`
- `GET /api/v1/repos/{owner}/{repo}/diff?base={base}&head={head}`
- Stable errors for invalid SHAs, missing commits, and invalid ranges.

API client:

- Add typed methods and response types.

Web:

- Link commit rows to a commit detail route.
- Add `/{owner}/{repo}/commits/{sha}`.
- Render metadata and changed files.
- Reuse `DiffViewer.svelte` for actual patches.
- Avoid review controls until comments/reviews exist.

Tests:

- Commit lookup success.
- Invalid SHA rejection.
- Missing commit handling.
- Root commit diff.
- Two-commit diff.
- API route tests.

Docs:

- Update [API](./api.md), [Architecture](./architecture.md), and [Web App](./web-app.md) when the endpoints and page exist.

## Hardening After That

After commit inspection, the next infrastructure hardening should be REST API auth:

- Apply JWT verification to REST routes.
- Model repo read/write scopes consistently.
- Keep local mode explicit.
- Avoid silently exposing repository contents on a network interface.

Other important follow-ups:

- Stream Git smart HTTP bodies.
- Add pagination and conditional caching to large projections.
- Add blob download routes for binary and too-large files.
- Add deployment packaging and backups.
- Add runner design only after the Git/review spine is solid.
