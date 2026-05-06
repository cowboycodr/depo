use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use depo_core::git::{
    BareRepository, BlobContent, BlobKind, BranchName, CommitAuthor, CommitChange, CommitRequest,
    GitSha, RepoFilePath, RepoId, RepositoryError, TreeEntry, TreeEntryKind, ValidatedRef,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{AppState, db};

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

pub async fn create_repo(
    State(state): State<AppState>,
    Json(payload): Json<CreateRepoRequest>,
) -> Result<(StatusCode, Json<RepositoryResponse>), ApiError> {
    let owner = depo_core::git::RepoOwner::parse(&payload.owner)?;
    let name = depo_core::git::RepoName::parse(&payload.name)?;
    let id = RepoId::new(owner, name);
    let default_branch = BranchName::parse(payload.default_branch.as_deref().unwrap_or("main"))?;

    if db::get_repository(&state.db, id.owner().as_str(), id.name().as_str())
        .await?
        .is_some()
    {
        return Err(ApiError::conflict(
            "repo_already_exists",
            format!("Repository {id} already exists."),
            json!({ "owner": id.owner().as_str(), "repo": id.name().as_str() }),
        ));
    }

    let repo = BareRepository::create(
        &state.storage,
        id.clone(),
        default_branch.clone(),
        state.git.clone(),
    )?;
    let storage_path = repo
        .path()
        .to_str()
        .ok_or_else(|| {
            ApiError::internal(
                "non_utf8_storage_path",
                "Repository storage path is not valid UTF-8.",
                json!({ "repo": id.as_full_name() }),
            )
        })?
        .to_owned();

    let record = match db::insert_repository(
        &state.db,
        &id.as_full_name(),
        id.owner().as_str(),
        id.name().as_str(),
        default_branch.as_str(),
        &storage_path,
    )
    .await
    {
        Ok(record) => record,
        Err(error) => {
            let _ = std::fs::remove_dir_all(repo.path());
            return Err(ApiError::from(error));
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(RepositoryResponse {
            repo: RepositoryDto::from(record),
        }),
    ))
}

pub async fn list_repos(
    State(state): State<AppState>,
) -> Result<Json<RepositoryListResponse>, ApiError> {
    let repos = db::list_repositories(&state.db)
        .await?
        .into_iter()
        .map(RepositoryDto::from)
        .collect();

    Ok(Json(RepositoryListResponse {
        repos,
        next_cursor: None,
        has_more: false,
    }))
}

pub async fn get_repo(
    State(state): State<AppState>,
    Path(params): Path<RepoPathParams>,
) -> Result<Json<RepositoryResponse>, ApiError> {
    let record = load_repo_record(&state, &params.owner, &params.repo).await?;
    Ok(Json(RepositoryResponse {
        repo: RepositoryDto::from(record),
    }))
}

pub async fn create_commit(
    State(state): State<AppState>,
    Path(params): Path<RepoPathParams>,
    Json(payload): Json<CommitBuilderRequest>,
) -> Result<Json<CommitBuilderResponse>, ApiError> {
    let (_record, repo) = load_repo(&state, &params.owner, &params.repo).await?;
    let target_branch = BranchName::parse(&payload.target_branch)?;
    let expected_head_sha = payload
        .expected_head_sha
        .as_deref()
        .map(GitSha::parse)
        .transpose()?;

    let mut changes = Vec::with_capacity(payload.changes.len());
    for change in payload.changes {
        changes.push(change.into_core()?);
    }

    let result = repo.create_commit(CommitRequest {
        target_branch,
        expected_head_sha,
        message: payload.message,
        author: payload.author,
        changes,
    })?;

    Ok(Json(CommitBuilderResponse {
        commit: CommitDto {
            sha: result.sha,
            tree_sha: result.tree_sha,
            branch: result.branch.as_str().to_owned(),
        },
        ref_update: RefUpdateDto {
            old_sha: result.ref_update.old_sha,
            new_sha: result.ref_update.new_sha,
            status: result.ref_update.status,
        },
    }))
}

pub async fn get_tree(
    State(state): State<AppState>,
    Path(params): Path<RepoPathParams>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<TreeResponse>, ApiError> {
    let (record, repo) = load_repo(&state, &params.owner, &params.repo).await?;
    let reference = read_reference(&record, query.ref_name.as_deref())?;
    let path = RepoFilePath::parse_tree(query.path.as_deref().unwrap_or(""))?;
    let (commit_sha, nodes) = repo.list_tree(&reference, &path)?;

    Ok(Json(TreeResponse {
        path: path.as_str().to_owned(),
        commit_sha,
        nodes: nodes.into_iter().map(TreeEntryDto::from).collect(),
    }))
}

pub async fn get_blob(
    State(state): State<AppState>,
    Path(params): Path<RepoPathParams>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<BlobResponse>, ApiError> {
    let (record, repo) = load_repo(&state, &params.owner, &params.repo).await?;
    let reference = read_reference(&record, query.ref_name.as_deref())?;
    let path = RepoFilePath::parse_file(query.path.as_deref().ok_or_else(|| {
        ApiError::bad_request(
            "missing_path",
            "Blob reads require a path query parameter.",
            json!({}),
        )
    })?)?;
    let blob = repo.read_blob(&reference, &path, state.inline_blob_limit)?;

    Ok(Json(BlobResponse::from(blob)))
}

pub async fn get_view(
    State(state): State<AppState>,
    Path(params): Path<RepoPathParams>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<RepoViewResponse>, ApiError> {
    let (record, repo) = load_repo(&state, &params.owner, &params.repo).await?;
    let default_branch = record.default_branch.clone();
    let reference = read_reference(&record, query.ref_name.as_deref())?;
    let ref_name = reference.display_name();
    let branches = repo
        .list_branches()?
        .into_iter()
        .map(|branch| BranchDto {
            name: branch.name,
            head_sha: branch.head_sha,
        })
        .collect::<Vec<_>>();

    let resolved_ref = match repo.resolve_ref(&reference) {
        Ok(commit_sha) => Some(commit_sha),
        Err(RepositoryError::BranchMissing(_)) => None,
        Err(error) => return Err(ApiError::from(error)),
    };

    let (tree_nodes, active_file, recent_commits) = match &resolved_ref {
        Some(commit_sha) => {
            let root = RepoFilePath::root();
            let (_, tree) = repo.list_tree(&ValidatedRef::Commit(commit_sha.clone()), &root)?;
            let active_file = match query.path.as_deref() {
                Some(path) if !path.is_empty() => {
                    let file_path = RepoFilePath::parse_file(path)?;
                    Some(BlobResponse::from(repo.read_blob(
                        &ValidatedRef::Commit(commit_sha.clone()),
                        &file_path,
                        state.inline_blob_limit,
                    )?))
                }
                _ => None,
            };
            let recent_commits = repo
                .recent_commits(&ValidatedRef::Commit(commit_sha.clone()), 20)?
                .into_iter()
                .map(CommitSummaryDto::from)
                .collect();
            (
                tree.into_iter().map(TreeEntryDto::from).collect(),
                active_file,
                recent_commits,
            )
        }
        None => (Vec::new(), None, Vec::new()),
    };

    Ok(Json(RepoViewResponse {
        repo: RepositoryDto::from(record),
        reference: RefDto {
            name: ref_name,
            kind: match reference {
                ValidatedRef::Branch(_) => "branch".to_owned(),
                ValidatedRef::Commit(_) => "commit".to_owned(),
            },
            commit_sha: resolved_ref,
        },
        branches: BranchesDto {
            default_branch,
            items: branches,
        },
        tree: TreeNodesDto { nodes: tree_nodes },
        active_file,
        recent_commits,
    }))
}

async fn load_repo(
    state: &AppState,
    owner: &str,
    repo: &str,
) -> Result<(db::RepositoryRecord, BareRepository), ApiError> {
    let record = load_repo_record(state, owner, repo).await?;
    let id = RepoId::parse(&record.owner, &record.name)?;
    let bare = BareRepository::open(id, PathBuf::from(&record.storage_path), state.git.clone())?;
    Ok((record, bare))
}

async fn load_repo_record(
    state: &AppState,
    owner: &str,
    repo: &str,
) -> Result<db::RepositoryRecord, ApiError> {
    depo_core::git::RepoId::parse(owner, repo)?;
    db::get_repository(&state.db, owner, repo)
        .await?
        .ok_or_else(|| {
            ApiError::not_found(
                "repo_not_found",
                format!("Repository {owner}/{repo} does not exist."),
                json!({ "owner": owner, "repo": repo }),
            )
        })
}

fn read_reference(
    record: &db::RepositoryRecord,
    value: Option<&str>,
) -> Result<ValidatedRef, ApiError> {
    ValidatedRef::parse(value.unwrap_or(&record.default_branch)).map_err(ApiError::from)
}

fn language_for_path(path: &str) -> Option<&'static str> {
    let extension = path.rsplit_once('.')?.1;
    match extension {
        "md" | "markdown" => Some("markdown"),
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "json" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "sql" => Some("sql"),
        "html" => Some("html"),
        "css" => Some("css"),
        _ => None,
    }
}

fn blob_etag(blob: &BlobContent) -> String {
    let short = &blob.object_sha.as_str()[..12];
    let path = blob
        .path
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    format!("\"blob-{short}-{path}\"")
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepoRequest {
    pub owner: String,
    pub name: String,
    pub default_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RepoPathParams {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryResponse {
    pub repo: RepositoryDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListResponse {
    pub repos: Vec<RepositoryDto>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDto {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<db::RepositoryRecord> for RepositoryDto {
    fn from(record: db::RepositoryRecord) -> Self {
        Self {
            id: record.id,
            owner: record.owner,
            name: record.name,
            default_branch: record.default_branch,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitBuilderRequest {
    pub target_branch: String,
    pub expected_head_sha: Option<String>,
    pub message: String,
    pub author: CommitAuthor,
    pub changes: Vec<CommitChangeRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum CommitChangeRequest {
    #[serde(rename = "upsert")]
    Upsert {
        path: String,
        #[serde(rename = "contentBase64")]
        content_base64: String,
        #[serde(default = "default_file_mode")]
        mode: String,
    },
    #[serde(rename = "upsertText")]
    UpsertText {
        path: String,
        content: String,
        #[serde(default = "default_file_mode")]
        mode: String,
    },
}

impl CommitChangeRequest {
    fn into_core(self) -> Result<CommitChange, ApiError> {
        match self {
            Self::Upsert {
                path,
                content_base64,
                mode,
            } => Ok(CommitChange::Upsert {
                path: RepoFilePath::parse_file(path)?,
                content: STANDARD.decode(content_base64).map_err(|_| {
                    ApiError::bad_request(
                        "invalid_base64",
                        "Commit change contentBase64 is not valid base64.",
                        json!({}),
                    )
                })?,
                mode,
            }),
            Self::UpsertText {
                path,
                content,
                mode,
            } => Ok(CommitChange::Upsert {
                path: RepoFilePath::parse_file(path)?,
                content: content.into_bytes(),
                mode,
            }),
        }
    }
}

fn default_file_mode() -> String {
    "100644".to_owned()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitBuilderResponse {
    pub commit: CommitDto,
    pub ref_update: RefUpdateDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDto {
    pub sha: GitSha,
    pub tree_sha: GitSha,
    pub branch: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefUpdateDto {
    pub old_sha: GitSha,
    pub new_sha: GitSha,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeResponse {
    pub path: String,
    pub commit_sha: GitSha,
    pub nodes: Vec<TreeEntryDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNodesDto {
    pub nodes: Vec<TreeEntryDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEntryDto {
    pub path: String,
    pub name: String,
    pub kind: TreeEntryKind,
    pub mode: String,
    pub size: u64,
    pub object_sha: GitSha,
}

impl From<TreeEntry> for TreeEntryDto {
    fn from(entry: TreeEntry) -> Self {
        Self {
            path: entry.path,
            name: entry.name,
            kind: entry.kind,
            mode: entry.mode,
            size: entry.size,
            object_sha: entry.object_sha,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobResponse {
    pub path: String,
    pub kind: BlobKind,
    pub language: Option<String>,
    pub mode: String,
    pub size: u64,
    pub encoding: Option<String>,
    pub content: Option<String>,
    pub commit_sha: GitSha,
    pub object_sha: GitSha,
    pub etag: String,
}

impl From<BlobContent> for BlobResponse {
    fn from(blob: BlobContent) -> Self {
        Self {
            language: language_for_path(&blob.path).map(ToOwned::to_owned),
            etag: blob_etag(&blob),
            path: blob.path,
            kind: blob.kind,
            mode: blob.mode,
            size: blob.size,
            encoding: blob.encoding,
            content: blob.content,
            commit_sha: blob.commit_sha,
            object_sha: blob.object_sha,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoViewResponse {
    pub repo: RepositoryDto,
    #[serde(rename = "ref")]
    pub reference: RefDto,
    pub branches: BranchesDto,
    pub tree: TreeNodesDto,
    pub active_file: Option<BlobResponse>,
    pub recent_commits: Vec<CommitSummaryDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefDto {
    pub name: String,
    pub kind: String,
    pub commit_sha: Option<GitSha>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchesDto {
    pub default_branch: String,
    pub items: Vec<BranchDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDto {
    pub name: String,
    pub head_sha: GitSha,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummaryDto {
    pub sha: GitSha,
    pub title: String,
    pub author: CommitAuthor,
    pub committed_at: String,
}

impl From<depo_core::git::CommitSummary> for CommitSummaryDto {
    fn from(commit: depo_core::git::CommitSummary) -> Self {
        Self {
            sha: commit.sha,
            title: commit.title,
            author: commit.author,
            committed_at: commit.committed_at,
        }
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    details: Value,
}

impl ApiError {
    fn bad_request(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
            details,
        }
    }

    fn not_found(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            message: message.into(),
            details,
        }
    }

    fn conflict(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
            details,
        }
    }

    fn internal(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code,
            message: message.into(),
            details,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub details: Value,
}

impl From<depo_core::git::IdError> for ApiError {
    fn from(error: depo_core::git::IdError) -> Self {
        Self::bad_request("invalid_input", error.to_string(), json!({}))
    }
}

impl From<RepositoryError> for ApiError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::Id(error) => ApiError::from(error),
            RepositoryError::RepositoryExists(path) => Self::conflict(
                "repo_storage_exists",
                "Repository storage path already exists.",
                json!({ "path": path.display().to_string() }),
            ),
            RepositoryError::BranchMissing(branch) => Self::not_found(
                "ref_not_found",
                format!("Branch {branch} does not exist."),
                json!({ "branch": branch }),
            ),
            RepositoryError::HeadMismatch { expected, actual } => Self::conflict(
                "head_mismatch",
                "Branch head did not match expectedHeadSha.",
                json!({ "expected": expected, "actual": actual }),
            ),
            RepositoryError::EmptyCommit
            | RepositoryError::EmptyCommitMessage
            | RepositoryError::UnsupportedFileMode(_) => {
                Self::bad_request("invalid_commit", error.to_string(), json!({}))
            }
            RepositoryError::RepositoryMissing(path) => Self::not_found(
                "repo_storage_missing",
                "Repository storage path is missing.",
                json!({ "path": path.display().to_string() }),
            ),
            other => Self::internal(
                "git_error",
                "Git operation failed.",
                json!({
                    "reason": other.to_string()
                }),
            ),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(
            "database_error",
            "Database operation failed.",
            json!({
                "reason": error.to_string()
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request},
    };
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::{AuthMode, config, migrate, router};

    async fn test_app() -> (axum::Router, TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let database_url = format!("sqlite://{}", temp.path().join("depo.db").display());
        let db = config::connect_database(&database_url).await.unwrap();
        migrate(&db).await.unwrap();
        let state = AppState {
            db,
            storage: depo_core::git::StorageRoot::new(temp.path().join("repos")).unwrap(),
            git: depo_core::git::GitCommand::default(),
            inline_blob_limit: 1024 * 1024,
            auth_mode: AuthMode::Local,
        };
        (router(state), temp)
    }

    #[tokio::test]
    async fn creates_repo_commit_and_view_returns_actual_code() {
        let (app, _temp) = test_app().await;

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos",
                json!({
                    "owner": "kian",
                    "name": "depo",
                    "defaultBranch": "main"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos/kian/depo/commits",
                json!({
                    "targetBranch": "main",
                    "message": "Initial commit",
                    "author": { "name": "Kian", "email": "kian@example.com" },
                    "changes": [
                        {
                            "type": "upsertText",
                            "path": "README.md",
                            "content": "# Depo\n"
                        }
                    ]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/repos/kian/depo/view?ref=main&path=README.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["activeFile"]["content"], "# Depo\n");
        assert_eq!(json["tree"]["nodes"][0]["path"], "README.md");
        assert_eq!(json["recentCommits"][0]["title"], "Initial commit");
    }

    fn json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }
}
