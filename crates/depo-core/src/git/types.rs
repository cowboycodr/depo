use serde::{Deserialize, Serialize};

use super::{GitSha, RepoFilePath, BranchName};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitChange {
    Upsert {
        path: RepoFilePath,
        content: Vec<u8>,
        mode: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRequest {
    pub target_branch: BranchName,
    pub expected_head_sha: Option<GitSha>,
    pub message: String,
    pub author: CommitAuthor,
    pub changes: Vec<CommitChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitResult {
    pub sha: GitSha,
    pub tree_sha: GitSha,
    pub branch: BranchName,
    pub ref_update: CommitRefUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRefUpdate {
    pub old_sha: GitSha,
    pub new_sha: GitSha,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    pub path: String,
    pub name: String,
    pub kind: TreeEntryKind,
    pub mode: String,
    pub size: u64,
    pub object_sha: GitSha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TreeEntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobContent {
    pub path: String,
    pub kind: BlobKind,
    pub mode: String,
    pub size: u64,
    pub encoding: Option<String>,
    pub content: Option<String>,
    pub commit_sha: GitSha,
    pub object_sha: GitSha,
    pub last_commit: Option<CommitSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobKind {
    Text,
    Binary,
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchHead {
    pub name: String,
    pub head_sha: GitSha,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSummary {
    pub sha: GitSha,
    pub title: String,
    pub author: CommitAuthor,
    pub committed_at: String,
    pub additions: u32,
    pub removals: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitDetail {
    pub sha: GitSha,
    pub tree_sha: GitSha,
    pub parents: Vec<GitSha>,
    pub author: CommitAuthor,
    pub authored_at: String,
    pub committer: CommitAuthor,
    pub committed_at: String,
    pub title: String,
    pub message: String,
    pub diff: CommitDiff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitMetadata {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitDiff {
    pub base_sha: Option<GitSha>,
    pub head_sha: GitSha,
    pub stats: DiffStats,
    pub files: Vec<DiffFile>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffStats {
    pub files_changed: usize,
    pub additions: u32,
    pub removals: u32,
}

impl DiffStats {
    pub fn from_files(files: &[DiffFile]) -> Self {
        Self {
            files_changed: files.len(),
            additions: files.iter().map(|file| file.additions).sum(),
            removals: files.iter().map(|file| file.removals).sum(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: DiffFileStatus,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub additions: u32,
    pub removals: u32,
    pub binary: bool,
    pub old_file: DiffFileContent,
    pub new_file: DiffFileContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFileContent {
    pub kind: DiffContentKind,
    pub mode: Option<String>,
    pub size: Option<u64>,
    pub encoding: Option<String>,
    pub content: Option<String>,
    pub object_sha: Option<GitSha>,
}

impl DiffFileContent {
    pub fn missing() -> Self {
        Self {
            kind: DiffContentKind::Missing,
            mode: None,
            size: None,
            encoding: None,
            content: None,
            object_sha: None,
        }
    }

    pub fn unloaded(object_sha: &GitSha, mode: Option<&str>) -> Self {
        Self {
            kind: DiffContentKind::Unloaded,
            mode: mode.map(ToOwned::to_owned),
            size: None,
            encoding: None,
            content: None,
            object_sha: Some(object_sha.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffContentKind {
    Text,
    Binary,
    TooLarge,
    Missing,
    Unloaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawDiffEntry {
    pub old_mode: String,
    pub new_mode: String,
    pub old_object_sha: Option<GitSha>,
    pub new_object_sha: Option<GitSha>,
    pub status: DiffFileStatus,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DiffLineStats {
    pub additions: u32,
    pub removals: u32,
    pub binary: bool,
}
