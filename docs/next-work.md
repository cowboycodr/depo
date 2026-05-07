# Next Work

State captured: 2026-05-07.

Depo can now receive real pushes, render imported repositories, list commits, and inspect changed files in pushed commits.

## Completed Slice

```text
/goal make pushed commits inspectable in Depo: commit detail API, file diffs, and a web commit page using the existing diff viewer
```

Implemented:

- `BareRepository::commit_detail`.
- `BareRepository::diff_between`.
- Root commit diffs.
- First-parent commit diffs.
- `GET /api/v1/repos/{owner}/{repo}/commits/{sha}`.
- `GET /api/v1/repos/{owner}/{repo}/diff?base={base}&head={head}`.
- Typed API client methods and response types.
- Linked commit rows.
- `/{owner}/{repo}/commits/{sha}` web route.
- Changed-file sidebar and text diff rendering through `DiffViewer.svelte`.
- Selected-file diff hydration so commit pages avoid shipping every changed file body at once.

## Current Product Answers

The product can answer:

- What repositories exist?
- What files are in a repository?
- What is inside a selected text file?
- What recent commits exist?
- Can Git clients clone, fetch, and push?
- What changed in this commit?

## Recommended Next Goal

After commit inspection, the next infrastructure hardening should be REST API auth:

```text
/goal add REST API authentication to Depo with local mode kept explicit and JWT repo scopes shared with Git smart HTTP
```

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
