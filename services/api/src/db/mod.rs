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
