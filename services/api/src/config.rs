use std::{env, net::SocketAddr, path::PathBuf, str::FromStr};

use anyhow::{Context, bail};
use depo_core::git::{StorageRoot, StorageRootError};
use jsonwebtoken::DecodingKey;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use crate::AuthMode;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub bind_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub storage_root: StorageRoot,
    pub database_url: String,
    pub inline_blob_limit: u64,
    pub git_http_body_limit: usize,
    pub auth_mode: AuthMode,
}

impl ApiConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let auth_mode = match env::var("DEPO_AUTH_MODE") {
            Ok(value) if value == "local" => AuthMode::Local,
            Ok(value) if value == "jwt" => {
                let public_key_pem = auth_public_key_pem()?;
                DecodingKey::from_ec_pem(public_key_pem.as_bytes())
                    .context("DEPO_AUTH_PUBLIC_KEY_PEM must contain an ES256 public key")?;
                AuthMode::Jwt { public_key_pem }
            }
            Ok(value) => {
                bail!("unsupported DEPO_AUTH_MODE={value:?}; supported values are local and jwt")
            }
            Err(_) => bail!(
                "DEPO_AUTH_MODE must be set explicitly; use DEPO_AUTH_MODE=local for local development or DEPO_AUTH_MODE=jwt for signed tokens"
            ),
        };

        let bind_addr = env::var("DEPO_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3847".to_owned())
            .parse::<SocketAddr>()
            .context("DEPO_BIND_ADDR must be a socket address")?;

        let data_dir = match env::var("DEPO_DATA_DIR") {
            Ok(value) => PathBuf::from(value),
            Err(_) => default_data_dir()?,
        };
        let storage_root = StorageRoot::new(data_dir.join("repos"))?;
        let database_url = env::var("DEPO_DATABASE_URL")
            .unwrap_or_else(|_| format!("sqlite://{}", data_dir.join("depo.db").display()));
        let inline_blob_limit = env::var("DEPO_INLINE_BLOB_LIMIT")
            .ok()
            .map(|value| value.parse::<u64>())
            .transpose()
            .context("DEPO_INLINE_BLOB_LIMIT must be an integer byte count")?
            .unwrap_or(1024 * 1024);
        let git_http_body_limit = env::var("DEPO_GIT_HTTP_BODY_LIMIT")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()
            .context("DEPO_GIT_HTTP_BODY_LIMIT must be an integer byte count")?
            .unwrap_or(64 * 1024 * 1024);

        Ok(Self {
            bind_addr,
            data_dir,
            storage_root,
            database_url,
            inline_blob_limit,
            git_http_body_limit,
            auth_mode,
        })
    }

    pub fn ensure_directories(&self) -> Result<(), StorageRootError> {
        std::fs::create_dir_all(&self.data_dir).map_err(|source| StorageRootError::CreateDir {
            path: self.data_dir.clone(),
            source,
        })?;
        self.storage_root.ensure_exists()
    }

    pub async fn connect_database(&self) -> anyhow::Result<SqlitePool> {
        connect_database(&self.database_url).await
    }
}

pub async fn connect_database(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)
        .with_context(|| format!("invalid SQLite database URL {database_url:?}"))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to connect SQLite database {database_url:?}"))
}

fn default_data_dir() -> anyhow::Result<PathBuf> {
    let home = env::var("HOME").context("HOME must be set when DEPO_DATA_DIR is not provided")?;
    Ok(PathBuf::from(home).join(".depo"))
}

fn auth_public_key_pem() -> anyhow::Result<String> {
    if let Ok(value) = env::var("DEPO_AUTH_PUBLIC_KEY_PEM") {
        if value.trim().is_empty() {
            bail!("DEPO_AUTH_PUBLIC_KEY_PEM must not be empty when DEPO_AUTH_MODE=jwt");
        }
        return Ok(value);
    }

    if let Ok(path) = env::var("DEPO_AUTH_PUBLIC_KEY_PATH") {
        return std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read DEPO_AUTH_PUBLIC_KEY_PATH {path:?}"));
    }

    bail!("DEPO_AUTH_MODE=jwt requires DEPO_AUTH_PUBLIC_KEY_PEM or DEPO_AUTH_PUBLIC_KEY_PATH")
}
