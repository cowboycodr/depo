pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod git_http;

#[cfg(test)]
mod git_http_tests;

use axum::{
    Router,
    routing::{get, post},
};
use depo_core::git::{GitCommand, StorageRoot};
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    Local,
    Jwt { public_key_pem: String },
}

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub storage: StorageRoot,
    pub git: GitCommand,
    pub inline_blob_limit: u64,
    pub git_http_body_limit: usize,
    pub auth_mode: AuthMode,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(api::handlers::health))
        .route(
            "/api/v1/repos",
            post(api::handlers::create_repo).get(api::handlers::list_repos),
        )
        .route(
            "/api/v1/repos/{owner}/{repo}",
            get(api::handlers::get_repo),
        )
        .route(
            "/api/v1/repos/{owner}/{repo}/commits",
            post(api::handlers::create_commit).get(api::handlers::list_commits),
        )
        .route(
            "/api/v1/repos/{owner}/{repo}/commits/{sha}",
            get(api::handlers::get_commit),
        )
        .route(
            "/api/v1/repos/{owner}/{repo}/diff",
            get(api::handlers::get_diff),
        )
        .route(
            "/api/v1/repos/{owner}/{repo}/tree",
            get(api::handlers::get_tree),
        )
        .route(
            "/api/v1/repos/{owner}/{repo}/blob",
            get(api::handlers::get_blob),
        )
        .route(
            "/api/v1/repos/{owner}/{repo}/view",
            get(api::handlers::get_view),
        )
        .fallback(git_http::handle)
        .with_state(state)
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
