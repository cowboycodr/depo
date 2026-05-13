use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow)]
pub struct RepositoryRecord {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub storage_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct LandRecord {
    pub id: String,
    pub repo_id: String,
    pub actor: String,
    pub source: String,
    pub ref_name: String,
    pub short_ref: String,
    pub old_sha: String,
    pub new_sha: String,
    pub kind: String,
    pub status: String,
    pub head_title: Option<String>,
    pub commit_count: i64,
    pub additions: i64,
    pub removals: i64,
    pub pushed_at: String,
}

#[derive(Debug, Clone)]
pub struct NewLand {
    pub repo_id: String,
    pub actor: String,
    pub source: String,
    pub ref_name: String,
    pub short_ref: String,
    pub old_sha: String,
    pub new_sha: String,
    pub kind: String,
    pub status: String,
    pub head_title: Option<String>,
    pub commit_count: i64,
    pub additions: i64,
    pub removals: i64,
    pub commits: Vec<NewLandCommit>,
}

#[derive(Debug, Clone)]
pub struct NewLandCommit {
    pub sha: String,
    pub title: String,
    pub author_name: String,
    pub author_email: String,
    pub committed_at: String,
    pub additions: i64,
    pub removals: i64,
}

pub async fn insert_repository(
    pool: &SqlitePool,
    id: &str,
    owner: &str,
    name: &str,
    default_branch: &str,
    storage_path: &str,
) -> Result<RepositoryRecord, sqlx::Error> {
    sqlx::query_as::<_, RepositoryRecord>(
        r#"
        INSERT INTO repositories (id, owner, name, default_branch, storage_path)
        VALUES (?1, ?2, ?3, ?4, ?5)
        RETURNING id, owner, name, default_branch, storage_path, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(owner)
    .bind(name)
    .bind(default_branch)
    .bind(storage_path)
    .fetch_one(pool)
    .await
}

pub async fn get_repository(
    pool: &SqlitePool,
    owner: &str,
    name: &str,
) -> Result<Option<RepositoryRecord>, sqlx::Error> {
    sqlx::query_as::<_, RepositoryRecord>(
        r#"
        SELECT id, owner, name, default_branch, storage_path, created_at, updated_at
        FROM repositories
        WHERE owner = ?1 AND name = ?2
        "#,
    )
    .bind(owner)
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn list_repositories(pool: &SqlitePool) -> Result<Vec<RepositoryRecord>, sqlx::Error> {
    sqlx::query_as::<_, RepositoryRecord>(
        r#"
        SELECT id, owner, name, default_branch, storage_path, created_at, updated_at
        FROM repositories
        ORDER BY owner ASC, name ASC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn insert_land(pool: &SqlitePool, land: NewLand) -> Result<LandRecord, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let record = sqlx::query_as::<_, LandRecord>(
        r#"
        INSERT INTO lands (
            id,
            repo_id,
            actor,
            source,
            ref_name,
            short_ref,
            old_sha,
            new_sha,
            kind,
            status,
            head_title,
            commit_count,
            additions,
            removals
        )
        VALUES (
            'land_' || lower(hex(randomblob(12))),
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7,
            ?8,
            ?9,
            ?10,
            ?11,
            ?12,
            ?13
        )
        RETURNING id, repo_id, actor, source, ref_name, short_ref, old_sha, new_sha, kind, status,
            head_title, commit_count, additions, removals, pushed_at
        "#,
    )
    .bind(&land.repo_id)
    .bind(&land.actor)
    .bind(&land.source)
    .bind(&land.ref_name)
    .bind(&land.short_ref)
    .bind(&land.old_sha)
    .bind(&land.new_sha)
    .bind(&land.kind)
    .bind(&land.status)
    .bind(&land.head_title)
    .bind(land.commit_count)
    .bind(land.additions)
    .bind(land.removals)
    .fetch_one(&mut *tx)
    .await?;

    for (position, commit) in land.commits.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO land_commits (
                land_id,
                position,
                sha,
                title,
                author_name,
                author_email,
                committed_at,
                additions,
                removals
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&record.id)
        .bind(position as i64)
        .bind(&commit.sha)
        .bind(&commit.title)
        .bind(&commit.author_name)
        .bind(&commit.author_email)
        .bind(&commit.committed_at)
        .bind(commit.additions)
        .bind(commit.removals)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(record)
}

pub async fn list_lands(
    pool: &SqlitePool,
    repo_id: &str,
    limit: i64,
) -> Result<Vec<LandRecord>, sqlx::Error> {
    sqlx::query_as::<_, LandRecord>(
        r#"
        SELECT id, repo_id, actor, source, ref_name, short_ref, old_sha, new_sha, kind, status,
            head_title, commit_count, additions, removals, pushed_at
        FROM lands
        WHERE repo_id = ?1
        ORDER BY pushed_at DESC, id DESC
        LIMIT ?2
        "#,
    )
    .bind(repo_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}
