# Agent Instructions

Depo is not a throwaway prototype. Treat every change as code the maintainer will eventually read closely, run on personal infrastructure, and depend on to host this project itself.

The standing rule is simple:

```text
No shortcuts that create code debt, hidden assumptions, or architectural mess.
```

Speed of implementation is not the bottleneck. Correctness, clarity, maintainability, and product quality are the bottlenecks.

## Engineering Standard

Write code as if another strong engineer will review every line.

- Prefer explicit design over implicit behavior.
- Prefer small, stable modules over broad utility piles.
- Prefer typed boundaries over loose object passing.
- Prefer real data models over UI-only state illusions.
- Prefer durable primitives over feature-specific hacks.
- Prefer boring dependencies with clear ownership over clever fragile code.
- Prefer deletion of bad ideas over layering around them.

Do not hide complexity. Name it, isolate it, and test it.

## No Shortcut Policy

Do not take shortcuts that make the project harder to reason about later.

Avoid:

- Fake implementations presented as real.
- Mock data leaking into production paths.
- Hardcoded paths, users, repository names, branches, ports, or secrets.
- Silent fallbacks that mask broken state.
- Stringly typed internal protocols when structured data is practical.
- One-off parsing when a structured API or parser is available.
- Untested process execution around Git, CI, deployment, or filesystem state.
- Mixing Git storage, product metadata, and UI state without a clear boundary.
- UI that looks finished but is not wired to real behavior where behavior is implied.
- Broad abstractions added before the second real use case exists.

If a temporary constraint is unavoidable, make it explicit in code or documentation and keep it narrowly scoped. Do not bury TODOs in critical paths as a substitute for design.

## Acceptable Pragmatism

"No shortcuts" does not mean "maximum architecture immediately."

It is acceptable to start with a simpler implementation when the boundary is honest and replaceable.

Examples:

- Shelling out to the local `git` binary is acceptable if it is wrapped behind a clear Git service boundary, uses safe argument passing, validates inputs, captures errors precisely, and has tests around behavior.
- SQLite is acceptable as the initial metadata store if migrations, constraints, and access patterns are treated seriously.
- A single-node runner is acceptable if job state, logs, cancellation, retries, and failure modes are modeled cleanly.
- A narrow CI command model is acceptable before a full workflow engine if the product does not pretend to support full GitHub Actions semantics.

The test is whether the implementation creates a clean stepping stone or a future cleanup burden.

## Architecture Boundaries

Keep these layers distinct:

```text
Git Core
  repositories, refs, commits, trees, blobs, diffs

Product API
  projects, changes, reviews, comments, checks, jobs, logs

Interface
  repository browser, diff viewer, CI logs, review surfaces

Runner
  job claiming, checkout, execution, streaming, status reporting

Deployment
  packaging, service lifecycle, updates, backups
```

Do not let UI concerns shape Git storage. Do not let Git command output leak directly into frontend contracts. Do not let runner internals become the product API.

## API Principles

The API should be composable and low latency.

- Design endpoints around stable resources and operations.
- Return structured errors with actionable messages.
- Keep response shapes consistent.
- Make pagination, streaming, and caching explicit where needed.
- Avoid endpoints that only satisfy one screen if a more general primitive is natural.
- Avoid chatty flows where one efficient endpoint would better match the product.

APIs should be useful to the first-party UI, future CLI tools, local automation, and external integrations.

## Git and Filesystem Safety

Git and filesystem code is critical infrastructure.

- Validate repository identifiers and paths.
- Prevent path traversal.
- Treat refs, SHAs, and branch names as untrusted input.
- Use argument arrays instead of shell interpolation.
- Define timeouts for external processes.
- Capture stdout, stderr, exit code, and signal.
- Avoid global mutable process state.
- Handle bare and working repositories intentionally.
- Keep repository storage layout documented.

Never execute arbitrary repository-controlled code except inside the runner boundary.

## Runner and CI Safety

The runner performs remote code execution by design. Treat it as dangerous.

- Separate app server responsibilities from runner responsibilities.
- Make job lifecycle states explicit.
- Stream logs without losing final status.
- Avoid exposing host secrets to jobs by default.
- Make workspace cleanup deterministic.
- Support cancellation and failed setup states.
- Record enough metadata to debug failures after the process exits.

Do not quietly execute CI for untrusted repositories or users without an explicit trust model.

## UI Standard

The UI must look and feel excellent. Do not build an admin dashboard with nicer colors.

Depo should feel dense, calm, fast, and precise:

- Stable layouts.
- Excellent spacing.
- High-quality typography.
- Fast keyboard navigation.
- Thoughtful loading, empty, error, and streaming states.
- No decorative filler.
- No explanatory text where direct interaction is better.
- No card-heavy marketing composition inside the app.
- No UI that implies behavior that does not exist.

The repo browser, diff viewer, and CI log viewer are signature surfaces. Treat them as product-defining work.

## Testing and Verification

Add tests in proportion to risk. The riskiest paths need the strongest tests.

High-priority test areas:

- Git command wrappers.
- Repository path validation.
- Ref and commit lookup.
- Diff generation.
- Metadata migrations.
- Job lifecycle transitions.
- Log streaming.
- Runner process handling.
- Deployment/update behavior.

For UI work, verify responsive layout and interaction quality, not just that components render.

Before claiming work is complete, run the relevant checks. If checks cannot run, say exactly why.

## Documentation

Keep documentation close to decisions.

Document:

- Storage layout.
- API contracts.
- Runner behavior.
- Deployment process.
- Security assumptions.
- Known limitations.
- Intentional non-goals.

Do not let important decisions live only in chat history.

### Page Title Convention

Page titles use a `type:identifier` scheme:

```
<pageType>:<owner>/<repo>[@<sha>]
```

- The `type:` prefix is present for sub-pages (commits, tree, blob, etc.).
- The repo root page omits the prefix (`<owner>/<repo>`).
- Commit-anchored pages append `@<shortSha>` to identify the specific commit.

| Page | Title |
|---|---|
| Home | `depo` |
| Repo root | `{owner}/{repo}` |
| Commits list | `commits:{owner}/{repo}` |
| Commit detail | `{owner}/{repo}@{shortSha}` |
| Tree (future) | `tree:{owner}/{repo}@{ref}` |
| Blob (future) | `blob:{owner}/{repo}@{ref}` |

## Change Discipline

Keep changes reviewable.

- Make cohesive edits.
- Avoid unrelated refactors.
- Preserve user changes.
- Do not churn formatting outside the touched area unless a formatter is already part of the workflow.
- Explain tradeoffs in final summaries.
- Call out incomplete work directly.

If the codebase direction is unclear, inspect first and then choose the smallest coherent next step.

## Final Reminder

This project should become self-hosting infrastructure. Every compromise must be judged against that future.

Build the version the maintainer will be glad to own.
