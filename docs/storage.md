# Storage

State captured: 2026-05-07.

Depo currently stores product metadata in SQLite and Git data in bare repositories on disk.

## Data Directory

Default data directory:

```text
~/.depo
```

Configured by:

```bash
DEPO_DATA_DIR=/path/to/depo-data
```

Default layout:

```text
~/.depo/
  depo.db
  repos/
    {owner}/
      {repo}.git/
```

## SQLite Metadata

The current migration creates one table:

```sql
CREATE TABLE IF NOT EXISTS repositories (
    id TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    name TEXT NOT NULL,
    default_branch TEXT NOT NULL DEFAULT 'main',
    storage_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (owner, name)
);
```

The API enables SQLite WAL mode and foreign keys when connecting.

`refs`, `commits`, `api_keys`, users, organizations, reviews, checks, jobs, and logs are not in the schema yet.

## Bare Repository Layout

Repositories are bare Git repositories:

```text
repos/
  kian/
    depo.git/
      HEAD
      config
      objects/
      refs/
        heads/
          main
      hooks/
```

There is no working tree. Depo reads and writes Git objects through Git commands against the bare repository.

## Path Safety

Repository identity is validated before it is used as storage path material.

Current identifier rules:

- `owner` is required, max 64 characters.
- `repo` is required, max 100 characters.
- Allowed owner/repo characters are ASCII letters, numbers, dash, underscore, and dot.
- `.` and `..` are rejected.
- Values may not start or end with dot.
- Full repository IDs must be exactly `owner/repo`.

Repository file path rules:

- Paths are relative.
- Empty path is allowed only for tree roots.
- Paths may not start or end with slash.
- Paths may not contain backslash, null, colon, control characters, empty segments, `.`, or `..`.
- Paths are limited to 4096 characters.

Branch rules reject unsafe Git ref syntax:

- Full `refs/...` input.
- Leading dash or slash.
- Trailing slash, dot, or `.lock`.
- `..`, `//`, `@{`.
- ASCII control or whitespace characters.
- `~`, `^`, `:`, `?`, `*`, `[`, and backslash.
- Empty ref path segments.

SHA rules:

- Full 40 character SHA-1 only.
- Hex characters only.
- Normalized to lowercase.

## Git Process Boundary

Depo currently shells out to the system `git` binary. This is acceptable only because it is wrapped behind a narrow boundary.

Current process properties:

- Uses `std::process::Command`.
- Passes arguments as arrays.
- Does not use shell interpolation.
- Defaults stdin to null.
- Supports explicit stdin for Git smart HTTP.
- Captures stdout and stderr.
- Captures exit code and Unix signal.
- Applies timeouts.
- Kills timed-out child processes.

Repository methods add `--git-dir {path}` for bare repository operations.

## Git Write Paths

There are two current write paths:

1. Git smart HTTP pushes through `git http-backend`.
2. Commit builder API constructs Git objects and updates refs.

The commit builder:

- Rejects empty messages.
- Rejects empty change sets.
- Validates target branch names.
- Validates repository file paths.
- Validates file modes.
- Uses a temporary Git index.
- Uses `hash-object`, `update-index`, `write-tree`, `commit-tree`, and `update-ref`.
- Supports `expectedHeadSha` for optimistic concurrency.

## Known Storage Gaps

- There is no repository delete operation.
- There is no object cache in SQLite.
- There is no refs table yet.
- There is no commit metadata table yet.
- There are no backups.
- There is no storage compaction policy.
- There is no migration path to PostgreSQL yet.
- Large tree and blob reads need pagination or streaming behavior before large production repositories.
