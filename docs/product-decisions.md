# Product Decisions

State captured: 2026-05-07.

This file records the early decisions that should not live only in chat history.

## Product Intent

Depo is personal self-hosting infrastructure for code. It should be small enough to run on a single machine, but built with the same seriousness as infrastructure that will eventually host itself.

The product should grow into:

- Git repository hosting.
- Repository browsing.
- Commit and diff inspection.
- Review surfaces.
- CI and log streaming.
- Self-hosted deployment and update workflows.

## Development Stance

Depo is not being built as a slow milestone roadmap. LLM-assisted development changes implementation speed, so the project should move by making the real boundaries real early instead of waiting through artificial phases.

The current core spine is:

```text
workspace scaffold
  -> Git and storage primitives
  -> SQLite repository metadata
  -> repository create/list/get API
  -> commit builder API
  -> tree/blob/commit read APIs
  -> web app wired to real repository data
  -> authenticated Git smart HTTP clone/fetch/push
```

The project should continue with cohesive vertical slices that prove real behavior end to end.

## Web App Absorption

The existing SvelteKit web app was copied into this project. It should not be treated as a throwaway mockup and should not be redesigned for its own sake.

Rules for future UI work:

- Keep the existing visual language unless functionality requires a change.
- Preserve the dense, calm repository-browser feel.
- Do not replace real data with mock data in production paths.
- Do not add UI controls that imply behavior the backend cannot perform.
- Wire functionality through explicit API contracts rather than local illusions.

## API Shape

The API must serve three audiences:

- The first-party web app.
- Future CLI and automation.
- External integrations.

That means Depo needs both primitive resources and fast projections. Primitive endpoints expose stable operations like repository creation, blob reads, and commit creation. Projection endpoints, such as `/view`, return the data needed for a first meaningful screen without a chatty frontend waterfall.

## Source Code Visibility

The web app should show the actual source code of a selected file. The current implementation does this for text blobs smaller than `DEPO_INLINE_BLOB_LIMIT`. Binary files and files above the inline limit return metadata without inline content.

This is an important product boundary: the UI is reading Git objects, not displaying copied fixture text.

## Git Remote Identity

Git smart HTTP uses username `git`.

Local development remote format:

```bash
http://git:local@127.0.0.1:3847/{owner}/{repo}.git
```

JWT remote format:

```bash
https://git:{jwt}@host/{owner}/{repo}.git
```

SSH-style `git@host:owner/repo.git` remotes are not implemented yet.

## Current Completion Line

The first useful imported-project flow is complete:

1. Create repository metadata in Depo.
2. Add a Depo Git remote to a real local project.
3. Push over authenticated Git smart HTTP.
4. Open the repository in the web app.
5. Expand the tree.
6. View actual text file contents.
7. View recent commits.

Commit inspection is now part of the first usable spine: commits list links to a commit detail page, and the commit detail API returns structured file diffs for the existing diff viewer.
