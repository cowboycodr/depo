use std::net::SocketAddr;

use anyhow::Context;
use depo_api::{AppState, config::ApiConfig, migrate, router};
use depo_core::git::GitCommand;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ApiConfig::from_env()?;
    config.ensure_directories()?;

    let db = config.connect_database().await?;
    migrate(&db)
        .await
        .context("failed to run database migrations")?;

    let state = AppState {
        db,
        storage: config.storage_root.clone(),
        git: GitCommand::default(),
        inline_blob_limit: config.inline_blob_limit,
        auth_mode: config.auth_mode,
    };

    serve(config.bind_addr, state).await
}

async fn serve(bind_addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    axum::serve(listener, router(state))
        .await
        .context("api server exited")
}
