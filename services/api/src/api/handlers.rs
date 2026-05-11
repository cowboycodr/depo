use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use depo_core::git::{
    BareRepository, BranchName, CommitRequest, GitSha, RepoFilePath, RepoId, RepositoryError,
    ValidatedRef,
};
use serde_json::json;

use super::ApiError;
use crate::{
    AppState,
    api::dto::{
        BlobResponse, BranchDto, BranchesDto, CommitBuilderRequest, CommitBuilderResponse,
        CommitDetailQuery, CommitDetailResponse, CommitDto, CommitListResponse, CommitPathParams,
        CommitSummaryDto, CommitsListQuery, CreateRepoRequest, DiffDto, DiffQuery, DiffResponse,
        HealthResponse, ReadQuery, RefDto, RefUpdateDto, RepoPathParams, RepoViewResponse,
        RepositoryDto, RepositoryListResponse, RepositoryResponse, TreeEntryDto, TreeNodesDto,
        TreeResponse,
    },
    db,
};

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
            if let Err(cleanup_err) = std::fs::remove_dir_all(repo.path()) {
                eprintln!(
                    "depo-api: failed to clean up bare repository at {storage_path}: {cleanup_err}"
                );
            }
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

pub async fn list_commits(
    State(state): State<AppState>,
    Path(params): Path<RepoPathParams>,
    Query(query): Query<CommitsListQuery>,
) -> Result<Json<CommitListResponse>, ApiError> {
    let (record, repo) = load_repo(&state, &params.owner, &params.repo).await?;
    let reference = read_reference(&record, query.ref_name.as_deref())?;
    let limit = query.limit.unwrap_or(100).min(500) as usize;

    let commits = match repo.resolve_ref(&reference) {
        Ok(commit_sha) => repo
            .recent_commits(&ValidatedRef::Commit(commit_sha), limit)?
            .into_iter()
            .map(CommitSummaryDto::from)
            .collect(),
        Err(RepositoryError::BranchMissing(_)) => Vec::new(),
        Err(error) => return Err(ApiError::from(error)),
    };

    Ok(Json(CommitListResponse { commits }))
}

pub async fn get_commit(
    State(state): State<AppState>,
    Path(params): Path<CommitPathParams>,
    Query(query): Query<CommitDetailQuery>,
) -> Result<Json<CommitDetailResponse>, ApiError> {
    let (record, repo) = load_repo(&state, &params.owner, &params.repo).await?;
    let sha = GitSha::parse(&params.sha)?;
    let content_path = query
        .path
        .as_deref()
        .map(RepoFilePath::parse_file)
        .transpose()?;
    let detail = repo.commit_detail(&sha, state.inline_blob_limit, content_path.as_ref())?;

    Ok(Json(CommitDetailResponse {
        repo: RepositoryDto::from(record),
        commit: super::dto::CommitDetailDto::from(&detail),
        diff: DiffDto::from(detail.diff),
    }))
}

pub async fn get_diff(
    State(state): State<AppState>,
    Path(params): Path<RepoPathParams>,
    Query(query): Query<DiffQuery>,
) -> Result<Json<DiffResponse>, ApiError> {
    let (record, repo) = load_repo(&state, &params.owner, &params.repo).await?;
    let head = GitSha::parse(&query.head)?;
    let base = query.base.as_deref().map(GitSha::parse).transpose()?;
    let content_path = query
        .path
        .as_deref()
        .map(RepoFilePath::parse_file)
        .transpose()?;
    let diff = repo.diff_between(
        base.as_ref(),
        &head,
        state.inline_blob_limit,
        content_path.as_ref(),
    )?;

    Ok(Json(DiffResponse {
        repo: RepositoryDto::from(record),
        diff: DiffDto::from(diff),
    }))
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
        .map(BranchDto::from)
        .collect::<Vec<_>>();

    let resolved_ref = match repo.resolve_ref(&reference) {
        Ok(commit_sha) => Some(commit_sha),
        Err(RepositoryError::BranchMissing(_)) => None,
        Err(error) => return Err(ApiError::from(error)),
    };

    let (tree_nodes, active_file, recent_commits) = match &resolved_ref {
        Some(commit_sha) => {
            let root = RepoFilePath::root();
            let (_, tree) =
                repo.list_tree_recursive(&ValidatedRef::Commit(commit_sha.clone()), &root)?;
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

pub(crate) async fn load_repo(
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
