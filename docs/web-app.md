# Web App

State captured: 2026-05-07.

The web app in `apps/web` was copied into Depo and wired to real API data. It should not be redesigned unless functionality requires it.

## Stack

- SvelteKit 5.
- Tailwind 4 through `@tailwindcss/vite`.
- TypeScript.
- `@depo/api-client` for API access.
- `@pierre/diffs` is installed and used by `DiffViewer.svelte`, but diff data is not wired yet.
- Iconify lucide icons.

## API Origin

Server-side loads use:

```ts
env.DEPO_API_ORIGIN ?? "http://127.0.0.1:3847"
```

The default local API origin is:

```text
http://127.0.0.1:3847
```

## Routes

### `/`

Current behavior:

- Loads repository list through `client.repos.list()`.
- Renders a create repository form.
- Submits `owner` and `name` to `client.repos.create()`.
- Redirects to `/{owner}/{repo}` after creation.

The create form currently creates repository metadata and a bare repository. It does not create an initial README commit.

### `/{owner}/{repo}`

Current behavior:

- Reads `ref` and `path` query parameters.
- Calls `client.repos.view(owner, repo, { ref, path })`.
- If no path is selected and a README exists in the returned tree, calls `/view` again with that README path.
- Renders the repository tree.
- Renders actual text file content for selected text blobs.
- Shows non-preview messaging for binary or too-large blobs.
- Keeps local tab state for opened files.
- Supports tab closing and drag-and-drop reordering.

This route is the main proof that imported repositories are readable through the web app.

### `/{owner}/{repo}/commits`

Current behavior:

- Reads optional `ref` query parameter.
- Loads repository view and commits in parallel.
- Groups commits by date.
- Shows commit title, author, relative time, and short SHA.

Commit rows are not linked to commit detail pages yet because commit detail and diff APIs do not exist.

## Components

Important product surfaces:

- `RepositoryTree.svelte`: builds a visible tree from recursive API nodes and supports expandable folders.
- `FileViewer.svelte`: renders actual text content with line numbers.
- `FileHeader.svelte`: owns tabs, file metadata, and diff-mode controls.
- `DiffViewer.svelte`: existing diff renderer wrapper for future commit/diff pages.
- `NavBar.svelte`: repository navigation between code and commits.

## UI Boundary

Do not make broad visual changes for their own sake. The current instruction is to modify the UI only where real functionality needs it.

Allowed UI work:

- Wire existing surfaces to real backend behavior.
- Add loading, empty, and error states where behavior needs them.
- Add links or controls for implemented behavior.
- Improve layout only when current functionality breaks or becomes unclear.

Avoid:

- Marketing pages inside the app.
- Decorative filler.
- Fake review/check/diff controls.
- Mock data in production routes.
- Large redesigns unrelated to the current product slice.

## Current Frontend Gaps

- No commit detail route.
- No diff route.
- No real compare view.
- No REST auth token handling.
- No pagination UI for large trees or commit histories.
- No blob download route for binary or too-large files.
- No CI log surface.
