pub mod api;
pub mod config;
pub mod db;

use axum::{
    Router,
    routing::{get, post},
};
use depo_core::git::{GitCommand, StorageRoot};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Local,
}

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub storage: StorageRoot,
    pub git: GitCommand,
    pub inline_blob_limit: u64,
    pub auth_mode: AuthMode,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(api::health))
        .route("/api/v1/repos", post(api::create_repo).get(api::list_repos))
        .route("/api/v1/repos/{owner}/{repo}", get(api::get_repo))
        .route(
            "/api/v1/repos/{owner}/{repo}/commits",
            post(api::create_commit),
        )
        .route("/api/v1/repos/{owner}/{repo}/tree", get(api::get_tree))
        .route("/api/v1/repos/{owner}/{repo}/blob", get(api::get_blob))
        .route("/api/v1/repos/{owner}/{repo}/view", get(api::get_view))
        .with_state(state)
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
