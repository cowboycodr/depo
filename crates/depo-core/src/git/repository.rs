use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{
    BranchName, GitCommand, GitCommandRequest, GitProcessError, GitSha, RepoFilePath, RepoId,
    StorageRoot, StorageRootError, ValidatedRef, ZERO_SHA,
};

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error(transparent)]
    Id(#[from] super::ids::IdError),
    #[error(transparent)]
    Storage(#[from] StorageRootError),
    #[error(transparent)]
    Git(#[from] GitProcessError),
    #[error("repository already exists at {0:?}")]
    RepositoryExists(PathBuf),
    #[error("repository path does not exist: {0:?}")]
    RepositoryMissing(PathBuf),
    #[error("path is not valid UTF-8: {0:?}")]
    NonUtf8Path(PathBuf),
    #[error("failed to write temporary Git object input: {0}")]
    TempWrite(std::io::Error),
    #[error("git returned invalid UTF-8: {0}")]
    GitUtf8(#[from] std::string::FromUtf8Error),
    #[error("git returned invalid output: {0}")]
    InvalidGitOutput(String),
    #[error("branch does not exist: {0}")]
    BranchMissing(String),
    #[error("expected head {expected}, but branch currently points at {actual}")]
    HeadMismatch { expected: String, actual: String },
    #[error("commit request must include at least one change")]
    EmptyCommit,
    #[error("commit message is required")]
    EmptyCommitMessage,
    #[error("unsupported file mode: {0}")]
    UnsupportedFileMode(String),
}

#[derive(Debug, Clone)]
pub struct BareRepository {
    id: RepoId,
    path: PathBuf,
    git: GitCommand,
}

impl BareRepository {
    pub fn create(
        storage: &StorageRoot,
        id: RepoId,
        default_branch: BranchName,
        git: GitCommand,
    ) -> Result<Self, RepositoryError> {
        storage.ensure_exists()?;
        let owner_dir = storage.owner_dir(&id);
        fs::create_dir_all(&owner_dir).map_err(|source| StorageRootError::CreateDir {
            path: owner_dir.clone(),
            source,
        })?;

        let path = storage.bare_repo_path(&id);
        if path.exists() {
            return Err(RepositoryError::RepositoryExists(path));
        }

        let path_arg = path_to_arg(&path)?;
        git.run(
            GitCommandRequest::new(["init", "--bare", path_arg.as_str()])
                .timeout(Duration::from_secs(15)),
        )?;

        let repo = Self { id, path, git };
        let default_ref = default_branch.ref_name();
        repo.git_run(["symbolic-ref", "HEAD", default_ref.as_str()])?;
        Ok(repo)
    }

    pub fn open(id: RepoId, path: PathBuf, git: GitCommand) -> Result<Self, RepositoryError> {
        if !path.exists() {
            return Err(RepositoryError::RepositoryMissing(path));
        }
        Ok(Self { id, path, git })
    }

    pub fn id(&self) -> &RepoId {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn create_commit(&self, request: CommitRequest) -> Result<CommitResult, RepositoryError> {
        if request.message.trim().is_empty() {
            return Err(RepositoryError::EmptyCommitMessage);
        }
        if request.changes.is_empty() {
            return Err(RepositoryError::EmptyCommit);
        }

        let current_head = self.branch_head(&request.target_branch)?;
        if let Some(expected) = &request.expected_head_sha {
            let actual = current_head
                .as_ref()
                .map(GitSha::as_str)
                .unwrap_or(ZERO_SHA)
                .to_owned();
            if actual != expected.as_str() {
                return Err(RepositoryError::HeadMismatch {
                    expected: expected.as_str().to_owned(),
                    actual,
                });
            }
        }

        let old_sha = current_head.unwrap_or_else(GitSha::zero);
        let index_dir = tempfile::Builder::new()
            .prefix("depo-index-")
            .tempdir()
            .map_err(RepositoryError::TempWrite)?;
        let index_path = index_dir.path().join("index");
        let index_arg = path_to_arg(&index_path)?;

        if old_sha.as_str() != ZERO_SHA {
            self.git_run_with_env(
                ["read-tree", old_sha.as_str()],
                [("GIT_INDEX_FILE", index_arg.as_str())],
            )?;
        }

        for change in &request.changes {
            match change {
                CommitChange::Upsert {
                    path,
                    content,
                    mode,
                } => {
                    validate_file_mode(mode)?;
                    let mut input = tempfile::NamedTempFile::new_in(index_dir.path())
                        .map_err(RepositoryError::TempWrite)?;
                    input
                        .write_all(content)
                        .map_err(RepositoryError::TempWrite)?;
                    input.flush().map_err(RepositoryError::TempWrite)?;
                    let input_path = path_to_arg(input.path())?;

                    let blob_output = self.git_run(["hash-object", "-w", input_path.as_str()])?;
                    let blob_sha = parse_git_sha_output(blob_output.stdout_string()?)?;

                    let cacheinfo = format!("{mode},{blob_sha},{}", path.as_str());
                    self.git_run_owned_with_env(
                        vec![
                            "update-index".to_owned(),
                            "--add".to_owned(),
                            "--cacheinfo".to_owned(),
                            cacheinfo,
                        ],
                        [("GIT_INDEX_FILE", index_arg.as_str())],
                    )?;
                }
            }
        }

        let tree_output =
            self.git_run_with_env(["write-tree"], [("GIT_INDEX_FILE", index_arg.as_str())])?;
        let tree_sha = parse_git_sha_output(tree_output.stdout_string()?)?;

        let mut commit_args = vec!["commit-tree".to_owned(), tree_sha.as_str().to_owned()];
        if old_sha.as_str() != ZERO_SHA {
            commit_args.push("-p".to_owned());
            commit_args.push(old_sha.as_str().to_owned());
        }
        commit_args.push("-m".to_owned());
        commit_args.push(request.message.clone());

        let commit_output = self.git_run_owned_with_env(
            commit_args,
            [
                ("GIT_AUTHOR_NAME", request.author.name.as_str()),
                ("GIT_AUTHOR_EMAIL", request.author.email.as_str()),
                ("GIT_COMMITTER_NAME", request.author.name.as_str()),
                ("GIT_COMMITTER_EMAIL", request.author.email.as_str()),
            ],
        )?;
        let commit_sha = parse_git_sha_output(commit_output.stdout_string()?)?;
        let branch_ref = request.target_branch.ref_name();
        self.git_run([
            "update-ref",
            branch_ref.as_str(),
            commit_sha.as_str(),
            old_sha.as_str(),
        ])?;

        Ok(CommitResult {
            sha: commit_sha.clone(),
            tree_sha,
            branch: request.target_branch,
            ref_update: CommitRefUpdate {
                old_sha,
                new_sha: commit_sha,
                status: "updated".to_owned(),
            },
        })
    }

    pub fn list_tree(
        &self,
        reference: &ValidatedRef,
        path: &RepoFilePath,
    ) -> Result<(GitSha, Vec<TreeEntry>), RepositoryError> {
        let commit_sha = self.resolve_ref(reference)?;
        let treeish = treeish_for_path(&commit_sha, path);
        let output = self.git_run(["ls-tree", "-z", "-l", treeish.as_str()])?;
        let entries = parse_tree_entries(&output.stdout, path)?;
        Ok((commit_sha, entries))
    }

    pub fn read_blob(
        &self,
        reference: &ValidatedRef,
        path: &RepoFilePath,
        inline_limit: u64,
    ) -> Result<BlobContent, RepositoryError> {
        let commit_sha = self.resolve_ref(reference)?;
        let parent_path = parent_tree_path(path)?;
        let (_, entries) =
            self.list_tree(&ValidatedRef::Commit(commit_sha.clone()), &parent_path)?;
        let entry = entries
            .into_iter()
            .find(|entry| entry.path == path.as_str())
            .ok_or_else(|| RepositoryError::InvalidGitOutput(format!("blob not found: {path}")))?;

        if entry.kind != TreeEntryKind::File {
            return Err(RepositoryError::InvalidGitOutput(format!(
                "path is not a file: {path}"
            )));
        }

        if entry.size > inline_limit {
            return Ok(BlobContent {
                path: path.as_str().to_owned(),
                kind: BlobKind::TooLarge,
                mode: entry.mode,
                size: entry.size,
                encoding: None,
                content: None,
                commit_sha,
                object_sha: entry.object_sha,
            });
        }

        let blob_ref = treeish_for_path(&commit_sha, path);
        let output = self.git_run(["cat-file", "-p", blob_ref.as_str()])?;
        let kind = if output.stdout.contains(&0) {
            BlobKind::Binary
        } else {
            BlobKind::Text
        };

        let (encoding, content) = match kind {
            BlobKind::Text => (
                Some("utf-8".to_owned()),
                Some(String::from_utf8(output.stdout).map_err(RepositoryError::GitUtf8)?),
            ),
            BlobKind::Binary | BlobKind::TooLarge => (None, None),
        };

        Ok(BlobContent {
            path: path.as_str().to_owned(),
            kind,
            mode: entry.mode,
            size: entry.size,
            encoding,
            content,
            commit_sha,
            object_sha: entry.object_sha,
        })
    }

    pub fn list_branches(&self) -> Result<Vec<BranchHead>, RepositoryError> {
        let output = self.git_run([
            "for-each-ref",
            "--format=%(refname:short)%09%(objectname)",
            "refs/heads",
        ])?;

        let mut branches = Vec::new();
        let stdout = String::from_utf8(output.stdout).map_err(RepositoryError::GitUtf8)?;
        for record in stdout.lines() {
            if record.is_empty() {
                continue;
            }
            let (name, sha) = record.split_once('\t').ok_or_else(|| {
                RepositoryError::InvalidGitOutput("invalid branch listing".to_owned())
            })?;
            if name.is_empty() || sha.is_empty() {
                return Err(RepositoryError::InvalidGitOutput(
                    "invalid branch listing".to_owned(),
                ));
            }
            branches.push(BranchHead {
                name: name.to_owned(),
                head_sha: GitSha::parse(sha)?,
            });
        }
        Ok(branches)
    }

    pub fn recent_commits(
        &self,
        reference: &ValidatedRef,
        limit: usize,
    ) -> Result<Vec<CommitSummary>, RepositoryError> {
        let commit_sha = self.resolve_ref(reference)?;
        let limit = limit.clamp(1, 100).to_string();
        let output = self.git_run([
            "log",
            "-n",
            limit.as_str(),
            "--format=%H%x09%an%x09%ae%x09%cI%x09%s",
            commit_sha.as_str(),
        ])?;

        let mut commits = Vec::new();
        let stdout = String::from_utf8(output.stdout).map_err(RepositoryError::GitUtf8)?;
        for record in stdout.lines() {
            if record.is_empty() {
                continue;
            }
            let fields: Vec<&str> = record.splitn(5, '\t').collect();
            if fields.len() != 5 {
                return Err(RepositoryError::InvalidGitOutput(
                    "invalid commit log output".to_owned(),
                ));
            }
            commits.push(CommitSummary {
                sha: GitSha::parse(fields[0])?,
                title: fields[4].to_owned(),
                author: CommitAuthor {
                    name: fields[1].to_owned(),
                    email: fields[2].to_owned(),
                },
                committed_at: fields[3].to_owned(),
            });
        }

        Ok(commits)
    }

    pub fn resolve_ref(&self, reference: &ValidatedRef) -> Result<GitSha, RepositoryError> {
        match reference {
            ValidatedRef::Branch(branch) => self
                .branch_head(branch)?
                .ok_or_else(|| RepositoryError::BranchMissing(branch.as_str().to_owned())),
            ValidatedRef::Commit(sha) => {
                let rev = format!("{}^{{commit}}", sha.as_str());
                let output = self.git_run(["rev-parse", "--verify", rev.as_str()])?;
                parse_git_sha_output(output.stdout_string()?)
            }
        }
    }

    fn branch_head(&self, branch: &BranchName) -> Result<Option<GitSha>, RepositoryError> {
        let branch_ref = branch.ref_name();
        match self.git_run(["show-ref", "--verify", "--hash", branch_ref.as_str()]) {
            Ok(output) => Ok(Some(parse_git_sha_output(output.stdout_string()?)?)),
            Err(RepositoryError::Git(GitProcessError::Failed { .. })) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn git_run<I, S>(&self, args: I) -> Result<super::GitCommandOutput, RepositoryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut owned = self.git_prefix_args()?;
        owned.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self.git
            .run(GitCommandRequest::new(owned).timeout(Duration::from_secs(20)))
            .map_err(RepositoryError::Git)
    }

    fn git_run_with_env<I, S, E, K, V>(
        &self,
        args: I,
        env: E,
    ) -> Result<super::GitCommandOutput, RepositoryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut owned = self.git_prefix_args()?;
        owned.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        let mut request = GitCommandRequest::new(owned).timeout(Duration::from_secs(20));
        for (key, value) in env {
            request = request.env(key.as_ref(), value.as_ref());
        }
        self.git.run(request).map_err(RepositoryError::Git)
    }

    fn git_run_owned_with_env<E, K, V>(
        &self,
        args: Vec<String>,
        env: E,
    ) -> Result<super::GitCommandOutput, RepositoryError>
    where
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut owned = self.git_prefix_args()?;
        owned.extend(args);
        let mut request = GitCommandRequest::new(owned).timeout(Duration::from_secs(20));
        for (key, value) in env {
            request = request.env(key.as_ref(), value.as_ref());
        }
        self.git.run(request).map_err(RepositoryError::Git)
    }

    fn git_prefix_args(&self) -> Result<Vec<String>, RepositoryError> {
        Ok(vec!["--git-dir".to_owned(), path_to_arg(&self.path)?])
    }
}

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
}

fn path_to_arg(path: &Path) -> Result<String, RepositoryError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| RepositoryError::NonUtf8Path(path.to_path_buf()))
}

fn validate_file_mode(mode: &str) -> Result<(), RepositoryError> {
    match mode {
        "100644" | "100755" => Ok(()),
        other => Err(RepositoryError::UnsupportedFileMode(other.to_owned())),
    }
}

fn parse_git_sha_output(output: String) -> Result<GitSha, RepositoryError> {
    GitSha::parse(output.trim()).map_err(RepositoryError::Id)
}

fn treeish_for_path(commit_sha: &GitSha, path: &RepoFilePath) -> String {
    if path.is_root() {
        commit_sha.as_str().to_owned()
    } else {
        format!("{}:{}", commit_sha.as_str(), path.as_str())
    }
}

fn parent_tree_path(path: &RepoFilePath) -> Result<RepoFilePath, RepositoryError> {
    match path.as_str().rsplit_once('/') {
        Some((parent, _)) => Ok(RepoFilePath::parse_tree(parent)?),
        None => Ok(RepoFilePath::root()),
    }
}

fn parse_tree_entries(
    output: &[u8],
    base_path: &RepoFilePath,
) -> Result<Vec<TreeEntry>, RepositoryError> {
    let mut entries = Vec::new();
    for record in output.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let record = String::from_utf8(record.to_vec()).map_err(RepositoryError::GitUtf8)?;
        let (metadata, name) = record.split_once('\t').ok_or_else(|| {
            RepositoryError::InvalidGitOutput(format!("missing tree separator in {record:?}"))
        })?;
        let fields: Vec<&str> = metadata.split_whitespace().collect();
        if fields.len() != 4 {
            return Err(RepositoryError::InvalidGitOutput(format!(
                "expected 4 tree metadata fields in {record:?}"
            )));
        }

        let mode = fields[0].to_owned();
        let kind = match fields[1] {
            "blob" => TreeEntryKind::File,
            "tree" => TreeEntryKind::Directory,
            other => {
                return Err(RepositoryError::InvalidGitOutput(format!(
                    "unsupported tree entry kind {other:?}"
                )));
            }
        };
        let object_sha = GitSha::parse(fields[2])?;
        let size = match fields[3] {
            "-" => 0,
            value => value.parse::<u64>().map_err(|_| {
                RepositoryError::InvalidGitOutput(format!("invalid tree entry size {value:?}"))
            })?,
        };
        let full_path = base_path.join_child(name);

        entries.push(TreeEntry {
            path: full_path,
            name: name.to_owned(),
            kind,
            mode,
            size,
            object_sha,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{RepoId, StorageRoot};

    fn test_repo() -> (tempfile::TempDir, BareRepository) {
        let temp = tempfile::tempdir().unwrap();
        let storage = StorageRoot::new(temp.path().join("repos")).unwrap();
        let id = RepoId::parse("kian", "depo").unwrap();
        let repo = BareRepository::create(
            &storage,
            id,
            BranchName::parse("main").unwrap(),
            GitCommand::default(),
        )
        .unwrap();

        (temp, repo)
    }

    #[test]
    fn creates_bare_repository_with_default_head() {
        let (_temp, repo) = test_repo();
        assert!(repo.path().join("objects").exists());

        let head = repo.git_run(["symbolic-ref", "HEAD"]).unwrap();
        assert_eq!(head.stdout_string().unwrap().trim(), "refs/heads/main");
    }

    #[test]
    fn creates_commit_and_reads_actual_file_content() {
        let (_temp, repo) = test_repo();
        let result = repo
            .create_commit(CommitRequest {
                target_branch: BranchName::parse("main").unwrap(),
                expected_head_sha: None,
                message: "Initial commit".to_owned(),
                author: CommitAuthor {
                    name: "Kian".to_owned(),
                    email: "kian@example.com".to_owned(),
                },
                changes: vec![CommitChange::Upsert {
                    path: RepoFilePath::parse_file("README.md").unwrap(),
                    content: b"# Depo\n".to_vec(),
                    mode: "100644".to_owned(),
                }],
            })
            .unwrap();

        assert_ne!(result.sha.as_str(), ZERO_SHA);
        assert_eq!(result.ref_update.old_sha.as_str(), ZERO_SHA);

        let reference = ValidatedRef::Branch(BranchName::parse("main").unwrap());
        let (_, tree) = repo
            .list_tree(&reference, &RepoFilePath::parse_tree("").unwrap())
            .unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].path, "README.md");

        let blob = repo
            .read_blob(
                &reference,
                &RepoFilePath::parse_file("README.md").unwrap(),
                1024 * 1024,
            )
            .unwrap();
        assert_eq!(blob.kind, BlobKind::Text);
        assert_eq!(blob.content.as_deref(), Some("# Depo\n"));
    }

    #[test]
    fn rejects_stale_expected_head() {
        let (_temp, repo) = test_repo();
        let stale = GitSha::parse("1111111111111111111111111111111111111111").unwrap();
        let error = repo
            .create_commit(CommitRequest {
                target_branch: BranchName::parse("main").unwrap(),
                expected_head_sha: Some(stale),
                message: "Initial commit".to_owned(),
                author: CommitAuthor {
                    name: "Kian".to_owned(),
                    email: "kian@example.com".to_owned(),
                },
                changes: vec![CommitChange::Upsert {
                    path: RepoFilePath::parse_file("README.md").unwrap(),
                    content: b"# Depo\n".to_vec(),
                    mode: "100644".to_owned(),
                }],
            })
            .unwrap_err();

        assert!(matches!(error, RepositoryError::HeadMismatch { .. }));
    }
}
