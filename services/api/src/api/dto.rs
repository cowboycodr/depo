use depo_core::git::{
    BlobContent, BlobKind, BranchHead, CommitAuthor, CommitChange, CommitDiff, CommitSummary,
    DiffFile, DiffFileContent, DiffFileStatus, DiffStats, GitSha, RepoFilePath, TreeEntry,
    TreeEntryKind,
};
use serde::{Deserialize, Serialize};

use crate::db;

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
pub struct CommitsListQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitListResponse {
    pub commits: Vec<CommitSummaryDto>,
}

#[derive(Debug, Deserialize)]
pub struct RepoPathParams {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Deserialize)]
pub struct CommitPathParams {
    pub owner: String,
    pub repo: String,
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct CommitDetailQuery {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiffQuery {
    pub base: Option<String>,
    pub head: String,
    pub path: Option<String>,
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
    pub fn into_core(self) -> Result<CommitChange, crate::api::ApiError> {
        match self {
            Self::Upsert {
                path,
                content_base64,
                mode,
            } => Ok(CommitChange::Upsert {
                path: RepoFilePath::parse_file(path)?,
                content: base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    content_base64,
                )
                .map_err(|_| {
                    crate::api::ApiError::bad_request(
                        "invalid_base64",
                        "Commit change contentBase64 is not valid base64.",
                        serde_json::json!({}),
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

pub fn default_file_mode() -> String {
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
    pub last_commit: Option<CommitSummaryDto>,
}

impl From<BlobContent> for BlobResponse {
    fn from(blob: BlobContent) -> Self {
        Self {
            language: language_for_path(&blob.path).map(ToOwned::to_owned),
            etag: blob_etag(&blob),
            last_commit: blob.last_commit.map(CommitSummaryDto::from),
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

impl From<BranchHead> for BranchDto {
    fn from(branch: BranchHead) -> Self {
        Self {
            name: branch.name,
            head_sha: branch.head_sha,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummaryDto {
    pub sha: GitSha,
    pub title: String,
    pub author: CommitAuthor,
    pub committed_at: String,
    pub additions: u32,
    pub removals: u32,
    pub parents: Vec<GitSha>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contained_commits: Vec<CommitSummaryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl From<CommitSummary> for CommitSummaryDto {
    fn from(commit: CommitSummary) -> Self {
        Self {
            sha: commit.sha,
            title: commit.title,
            author: commit.author,
            committed_at: commit.committed_at,
            additions: commit.additions,
            removals: commit.removals,
            parents: commit.parents,
            contained_commits: commit
                .contained_commits
                .into_iter()
                .map(CommitSummaryDto::from)
                .collect(),
            description: commit.description,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetailResponse {
    pub repo: RepositoryDto,
    pub commit: CommitDetailDto,
    pub diff: DiffDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResponse {
    pub repo: RepositoryDto,
    pub diff: DiffDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetailDto {
    pub sha: GitSha,
    pub tree_sha: GitSha,
    pub parents: Vec<GitSha>,
    pub author: CommitAuthor,
    pub authored_at: String,
    pub committer: CommitAuthor,
    pub committed_at: String,
    pub title: String,
    pub message: String,
}

impl From<&depo_core::git::CommitDetail> for CommitDetailDto {
    fn from(commit: &depo_core::git::CommitDetail) -> Self {
        Self {
            sha: commit.sha.clone(),
            tree_sha: commit.tree_sha.clone(),
            parents: commit.parents.clone(),
            author: commit.author.clone(),
            authored_at: commit.authored_at.clone(),
            committer: commit.committer.clone(),
            committed_at: commit.committed_at.clone(),
            title: commit.title.clone(),
            message: commit.message.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffDto {
    pub base_sha: Option<GitSha>,
    pub head_sha: GitSha,
    pub stats: DiffStats,
    pub files: Vec<DiffFileDto>,
}

impl From<CommitDiff> for DiffDto {
    fn from(diff: CommitDiff) -> Self {
        Self {
            base_sha: diff.base_sha,
            head_sha: diff.head_sha,
            stats: diff.stats,
            files: diff.files.into_iter().map(DiffFileDto::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFileDto {
    pub path: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: DiffFileStatus,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub additions: u32,
    pub removals: u32,
    pub binary: bool,
    pub old_file: DiffFileContentDto,
    pub new_file: DiffFileContentDto,
}

impl From<DiffFile> for DiffFileDto {
    fn from(file: DiffFile) -> Self {
        let old_path = file.old_path.clone();
        let new_path = file.new_path.clone();

        Self {
            path: file.path,
            old_path: file.old_path,
            new_path: file.new_path,
            status: file.status,
            old_mode: file.old_mode,
            new_mode: file.new_mode,
            additions: file.additions,
            removals: file.removals,
            binary: file.binary,
            old_file: DiffFileContentDto::from_parts(old_path.as_deref(), file.old_file),
            new_file: DiffFileContentDto::from_parts(new_path.as_deref(), file.new_file),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFileContentDto {
    pub path: Option<String>,
    pub kind: depo_core::git::DiffContentKind,
    pub language: Option<String>,
    pub mode: Option<String>,
    pub size: Option<u64>,
    pub encoding: Option<String>,
    pub content: Option<String>,
    pub object_sha: Option<GitSha>,
}

impl DiffFileContentDto {
    pub fn from_parts(path: Option<&str>, content: DiffFileContent) -> Self {
        Self {
            path: path.map(ToOwned::to_owned),
            language: path.and_then(language_for_path).map(ToOwned::to_owned),
            kind: content.kind,
            mode: content.mode,
            size: content.size,
            encoding: content.encoding,
            content: content.content,
            object_sha: content.object_sha,
        }
    }
}

pub fn language_for_path(path: &str) -> Option<&'static str> {
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
