CREATE TABLE IF NOT EXISTS lands (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    actor TEXT NOT NULL,
    source TEXT NOT NULL,
    ref_name TEXT NOT NULL,
    short_ref TEXT NOT NULL,
    old_sha TEXT NOT NULL,
    new_sha TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (
        kind IN (
            'branch_created',
            'branch_updated',
            'branch_deleted'
        )
    ),
    status TEXT NOT NULL DEFAULT 'received' CHECK (
        status IN (
            'received',
            'checking',
            'passed',
            'failed'
        )
    ),
    head_title TEXT,
    commit_count INTEGER NOT NULL DEFAULT 0,
    additions INTEGER NOT NULL DEFAULT 0,
    removals INTEGER NOT NULL DEFAULT 0,
    pushed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_lands_repo_pushed_at
    ON lands (repo_id, pushed_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS land_commits (
    land_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    sha TEXT NOT NULL,
    title TEXT NOT NULL,
    author_name TEXT NOT NULL,
    author_email TEXT NOT NULL,
    committed_at TEXT NOT NULL,
    additions INTEGER NOT NULL DEFAULT 0,
    removals INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (land_id, position),
    FOREIGN KEY (land_id) REFERENCES lands(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_land_commits_sha
    ON land_commits (sha);
