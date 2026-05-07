# API

State captured: 2026-05-07.

Base URL:

```text
/api/v1
```

Repository identity is explicit in paths:

```text
/api/v1/repos/{owner}/{repo}
```

## Error Shape

REST errors use this envelope:

```json
{
  "error": {
    "code": "repo_not_found",
    "message": "Repository kian/depo does not exist.",
    "details": {
      "owner": "kian",
      "repo": "depo"
    }
  }
}
```

Clients should branch on HTTP status and `error.code`, not message text.

## Implemented Endpoints

### Health

```http
GET /health
```

Response:

```json
{ "ok": true }
```

### Create Repository

```http
POST /api/v1/repos
```

Request:

```json
{
  "owner": "kian",
  "name": "depo",
  "defaultBranch": "main"
}
```

`defaultBranch` is optional and defaults to `main`.

Response status: `201 Created`

```json
{
  "repo": {
    "id": "kian/depo",
    "owner": "kian",
    "name": "depo",
    "defaultBranch": "main",
    "createdAt": "2026-05-07T12:00:00.000Z",
    "updatedAt": "2026-05-07T12:00:00.000Z"
  }
}
```

### List Repositories

```http
GET /api/v1/repos
```

Response:

```json
{
  "repos": [],
  "nextCursor": null,
  "hasMore": false
}
```

The response shape is pagination-ready, but cursor pagination is not implemented yet.

### Get Repository

```http
GET /api/v1/repos/{owner}/{repo}
```

Response:

```json
{
  "repo": {
    "id": "kian/depo",
    "owner": "kian",
    "name": "depo",
    "defaultBranch": "main",
    "createdAt": "2026-05-07T12:00:00.000Z",
    "updatedAt": "2026-05-07T12:00:00.000Z"
  }
}
```

### Create Commit

```http
POST /api/v1/repos/{owner}/{repo}/commits
```

Request:

```json
{
  "targetBranch": "main",
  "expectedHeadSha": null,
  "message": "Initial commit",
  "author": {
    "name": "Kian",
    "email": "kian@example.com"
  },
  "changes": [
    {
      "type": "upsertText",
      "path": "README.md",
      "content": "# Depo\n",
      "mode": "100644"
    }
  ]
}
```

Supported change types:

- `upsertText`: sends UTF-8 text directly.
- `upsert`: sends bytes as `contentBase64`.

Supported file modes:

- `100644`
- `100755`

Response:

```json
{
  "commit": {
    "sha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
    "treeSha": "b9532c5d5be50d88e2f45d7c229566b2f1f99731",
    "branch": "main"
  },
  "refUpdate": {
    "oldSha": "0000000000000000000000000000000000000000",
    "newSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
    "status": "updated"
  }
}
```

`expectedHeadSha` is an optimistic concurrency guard. If supplied and the branch head does not match, the API returns `409 head_mismatch`.

### List Tree

```http
GET /api/v1/repos/{owner}/{repo}/tree?ref=main&path=src
```

Query parameters:

- `ref`: optional branch name or full 40 character commit SHA. Defaults to the repository default branch.
- `path`: optional tree path. Empty or omitted means repository root.

Response:

```json
{
  "path": "src",
  "commitSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
  "nodes": [
    {
      "path": "src/main.rs",
      "name": "main.rs",
      "kind": "file",
      "mode": "100644",
      "size": 921,
      "objectSha": "1111111111111111111111111111111111111111"
    }
  ]
}
```

### Read Blob

```http
GET /api/v1/repos/{owner}/{repo}/blob?ref=main&path=README.md
```

Response for inline text:

```json
{
  "path": "README.md",
  "kind": "text",
  "language": "markdown",
  "mode": "100644",
  "size": 128,
  "encoding": "utf-8",
  "content": "# Depo\n",
  "commitSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
  "objectSha": "1111111111111111111111111111111111111111",
  "etag": "\"blob-111111111111-README-md\""
}
```

Response for binary or too-large files keeps `content` null. Text files are inlined only up to `DEPO_INLINE_BLOB_LIMIT`.

### List Commits

```http
GET /api/v1/repos/{owner}/{repo}/commits?ref=main&limit=100
```

Query parameters:

- `ref`: optional branch name or full 40 character commit SHA. Defaults to the repository default branch.
- `limit`: optional integer. The API clamps this to the supported range in `depo-core`.

Response:

```json
{
  "commits": [
    {
      "sha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
      "title": "Initial commit",
      "author": {
        "name": "Kian",
        "email": "kian@example.com"
      },
      "committedAt": "2026-05-07T12:00:00Z"
    }
  ]
}
```

### Get Commit Detail

```http
GET /api/v1/repos/{owner}/{repo}/commits/{sha}
```

`sha` must be a full 40 character commit SHA.

This endpoint is the low-latency commit page projection. It returns repository metadata, commit metadata, aggregate diff stats, changed files, and inline old/new text content for one selected changed file when each side is under `DEPO_INLINE_BLOB_LIMIT`.

Query parameters:

- `path`: optional changed file path. If omitted, Depo hydrates the first changed file. Other changed files still return lightweight metadata and `oldFile.kind` / `newFile.kind` as `unloaded`.

Response:

```json
{
  "repo": {
    "id": "kian/depo",
    "owner": "kian",
    "name": "depo",
    "defaultBranch": "main",
    "createdAt": "2026-05-07T12:00:00.000Z",
    "updatedAt": "2026-05-07T12:00:00.000Z"
  },
  "commit": {
    "sha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
    "treeSha": "b9532c5d5be50d88e2f45d7c229566b2f1f99731",
    "parents": ["2222222222222222222222222222222222222222"],
    "author": {
      "name": "Kian",
      "email": "kian@example.com"
    },
    "authoredAt": "2026-05-07T12:00:00Z",
    "committer": {
      "name": "Kian",
      "email": "kian@example.com"
    },
    "committedAt": "2026-05-07T12:00:00Z",
    "title": "Expand README",
    "message": "Expand README\n"
  },
  "diff": {
    "baseSha": "2222222222222222222222222222222222222222",
    "headSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2",
    "stats": {
      "filesChanged": 1,
      "additions": 2,
      "removals": 0
    },
    "files": [
      {
        "path": "README.md",
        "oldPath": "README.md",
        "newPath": "README.md",
        "status": "modified",
        "oldMode": "100644",
        "newMode": "100644",
        "additions": 2,
        "removals": 0,
        "binary": false,
        "oldFile": {
          "path": "README.md",
          "kind": "text",
          "language": "markdown",
          "mode": "100644",
          "size": 7,
          "encoding": "utf-8",
          "content": "# Depo\n",
          "objectSha": "1111111111111111111111111111111111111111"
        },
        "newFile": {
          "path": "README.md",
          "kind": "text",
          "language": "markdown",
          "mode": "100644",
          "size": 27,
          "encoding": "utf-8",
          "content": "# Depo\n\nReal code hosting.\n",
          "objectSha": "3333333333333333333333333333333333333333"
        }
      }
    ]
  }
}
```

Root commits return `baseSha: null`. Added files have `oldFile.kind: "missing"`. Deleted files have `newFile.kind: "missing"`. Binary, too-large, and unloaded sides keep `content: null`.

### Get Diff

```http
GET /api/v1/repos/{owner}/{repo}/diff?base={baseSha}&head={headSha}
```

Query parameters:

- `head`: required full 40 character commit SHA.
- `base`: optional full 40 character commit SHA. If omitted, Depo diffs `head` against its first parent, or against the empty tree for root commits.
- `path`: optional changed file path to hydrate with inline old/new content.

The response shape is:

```json
{
  "repo": {},
  "diff": {}
}
```

`diff` has the same shape as the commit detail response's `diff` object.

### Repository View Projection

```http
GET /api/v1/repos/{owner}/{repo}/view?ref=main&path=README.md
```

This is the current low-latency frontend projection. It returns repository metadata, resolved ref data, branches, recursive tree nodes, the optional active file, and recent commits in one response.

Response shape:

```json
{
  "repo": {
    "id": "kian/depo",
    "owner": "kian",
    "name": "depo",
    "defaultBranch": "main",
    "createdAt": "2026-05-07T12:00:00.000Z",
    "updatedAt": "2026-05-07T12:00:00.000Z"
  },
  "ref": {
    "name": "main",
    "kind": "branch",
    "commitSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2"
  },
  "branches": {
    "defaultBranch": "main",
    "items": [
      {
        "name": "main",
        "headSha": "8f12c3bd0f4ff7bfb7267e7a61b3c4a8712a10b2"
      }
    ]
  },
  "tree": {
    "nodes": []
  },
  "activeFile": null,
  "recentCommits": []
}
```

## Frontend Latency Contract

Current behavior:

- Repository code page uses one `/view` request for the primary browser state.
- If the route has no selected file, the web load checks for a README in the returned tree and may make a second `/view` request with the README path.
- Commits page loads `/view` and `/commits` in parallel.
- Commit detail page uses one `/commits/{sha}` request for metadata, changed file list, aggregate stats, and inline old/new file contents.

Target behavior:

- First meaningful repository browser screen should remain one request.
- First meaningful compare screen should be one request.
- Large trees, commit histories, diffs, and logs should paginate or stream.
- Cacheable reads should expose exact content identity through commit SHAs, object SHAs, and ETags.

## Designed But Not Implemented

These endpoints are intentionally not documented as implemented:

- `GET /api/v1/repos/{owner}/{repo}/compare-view?base={base}&head={head}`
- `DELETE /api/v1/repos/{owner}/{repo}`
- Branch create/delete endpoints.
- Blob download or streaming endpoint for too-large and binary files.
