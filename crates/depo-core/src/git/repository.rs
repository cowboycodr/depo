use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{
    BranchName, GitCommand, GitCommandRequest, GitProcessError, GitSha, RepoFilePath, RepoId,
    StorageRoot, StorageRootError, ValidatedRef, ZERO_SHA,
};
use super::{
    commit, diff, tree,
    types::{
        BlobContent, BlobKind, BranchHead, CommitAuthor, CommitChange, CommitDetail, CommitDiff,
        CommitMetadata, CommitRefUpdate, CommitRequest, CommitResult, CommitSummary,
        DiffContentKind, DiffFile, DiffFileContent, DiffStats, TreeEntry, TreeEntryKind,
    },
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
    #[error("commit does not exist: {0}")]
    CommitMissing(String),
    #[error("expected head {expected}, but branch currently points at {actual}")]
    HeadMismatch { expected: String, actual: String },
    #[error("commit request must include at least one change")]
    EmptyCommit,
    #[error("commit message is required")]
    EmptyCommitMessage,
    #[error("unsupported file mode: {0}")]
    UnsupportedFileMode(String),
    #[error("path not found: {0}")]
    PathNotFound(String),
    #[error("path is not a file: {0}")]
    PathNotFile(String),
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

        let path_arg = commit::path_to_arg(&path)?;
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
        let index_arg = commit::path_to_arg(&index_path)?;

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
                    commit::validate_file_mode(mode)?;
                    let mut input = tempfile::NamedTempFile::new_in(index_dir.path())
                        .map_err(RepositoryError::TempWrite)?;
                    input
                        .write_all(content)
                        .map_err(RepositoryError::TempWrite)?;
                    input.flush().map_err(RepositoryError::TempWrite)?;
                    let input_path = commit::path_to_arg(input.path())?;

                    let blob_output = self.git_run(["hash-object", "-w", input_path.as_str()])?;
                    let blob_sha = commit::parse_git_sha_output(blob_output.stdout_string()?)?;

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
        let tree_sha = commit::parse_git_sha_output(tree_output.stdout_string()?)?;

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
        let commit_sha = commit::parse_git_sha_output(commit_output.stdout_string()?)?;
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
        let treeish = tree::treeish_for_path(&commit_sha, path);
        let output = self.git_run(["ls-tree", "-z", "-l", treeish.as_str()])?;
        let entries = tree::parse_tree_entries(&output.stdout, path)?;
        Ok((commit_sha, entries))
    }

    pub fn list_tree_recursive(
        &self,
        reference: &ValidatedRef,
        path: &RepoFilePath,
    ) -> Result<(GitSha, Vec<TreeEntry>), RepositoryError> {
        let commit_sha = self.resolve_ref(reference)?;
        let treeish = tree::treeish_for_path(&commit_sha, path);
        let output = self.git_run(["ls-tree", "-z", "-l", "-r", "-t", treeish.as_str()])?;
        let entries = tree::parse_tree_entries(&output.stdout, path)?;
        Ok((commit_sha, entries))
    }

    pub fn read_blob(
        &self,
        reference: &ValidatedRef,
        path: &RepoFilePath,
        inline_limit: u64,
    ) -> Result<BlobContent, RepositoryError> {
        let commit_sha = self.resolve_ref(reference)?;
        let parent_path = tree::parent_tree_path(path)?;
        let (_, entries) =
            self.list_tree(&ValidatedRef::Commit(commit_sha.clone()), &parent_path)?;
        let entry = entries
            .into_iter()
            .find(|entry| entry.path == path.as_str())
            .ok_or_else(|| RepositoryError::PathNotFound(path.as_str().to_owned()))?;

        if entry.kind != TreeEntryKind::File {
            return Err(RepositoryError::PathNotFile(path.as_str().to_owned()));
        }

        let last_commit = self.last_commit_for_file(&commit_sha, path).ok();

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
                last_commit,
            });
        }

        let blob_ref = tree::treeish_for_path(&commit_sha, path);
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
            last_commit,
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
        let limit_str = limit.clamp(1, 100).to_string();
        let output = self.git_run_owned_with_env(
            vec![
                "log".to_owned(),
                "-n".to_owned(),
                limit_str,
                "--format=%x00%H%x09%P%x09%an%x09%ae%x09%cI%x09%s".to_owned(),
                "--shortstat".to_owned(),
                commit_sha.as_str().to_owned(),
            ],
            std::iter::empty::<(&str, &str)>(),
        )?;

        let stdout = String::from_utf8(output.stdout).map_err(RepositoryError::GitUtf8)?;
        let mut commits = Vec::new();
        for record in stdout.split('\x00') {
            let record = record.trim();
            if record.is_empty() {
                continue;
            }
            commits.push(commit::parse_commit_with_stats(record)?);
        }

        Ok(commits)
    }

    fn last_commit_for_file(
        &self,
        commit_sha: &GitSha,
        path: &RepoFilePath,
    ) -> Result<CommitSummary, RepositoryError> {
        let output = self.git_run_owned_with_env(
            vec![
                "log".to_owned(),
                "-n".to_owned(),
                "1".to_owned(),
                "--format=%H%x00%an%x00%ae%x00%cI%x00%s%x00%P%x00%b%x00".to_owned(),
                commit_sha.as_str().to_owned(),
                "--".to_owned(),
                path.as_str().to_owned(),
            ],
            std::iter::empty::<(&str, &str)>(),
        )?;

        let fields = output.stdout.split(|b| *b == 0).collect::<Vec<_>>();
        if fields.len() < 6 || fields[0].is_empty() {
            return Err(RepositoryError::PathNotFound(path.as_str().to_owned()));
        }

        let sha = diff::utf8_field(fields[0])?.trim();
        if sha.is_empty() {
            return Err(RepositoryError::PathNotFound(path.as_str().to_owned()));
        }

        let parents = diff::utf8_field(fields[5])?
            .split_whitespace()
            .map(GitSha::parse)
            .collect::<Result<Vec<_>, _>>()?;

        let description = if fields.len() > 6 {
            let body = diff::utf8_field(fields[6])?.trim();
            if body.is_empty() {
                None
            } else {
                Some(body.to_owned())
            }
        } else {
            None
        };

        Ok(CommitSummary {
            sha: GitSha::parse(sha)?,
            title: diff::utf8_field(fields[4])?.trim_end().to_owned(),
            author: CommitAuthor {
                name: diff::utf8_field(fields[1])?.to_owned(),
                email: diff::utf8_field(fields[2])?.to_owned(),
            },
            committed_at: diff::utf8_field(fields[3])?.trim_end().to_owned(),
            additions: 0,
            removals: 0,
            parents,
            description,
        })
    }

    pub fn commit_detail(
        &self,
        sha: &GitSha,
        inline_limit: u64,
        content_path: Option<&RepoFilePath>,
    ) -> Result<CommitDetail, RepositoryError> {
        let commit_sha = self.resolve_ref(&ValidatedRef::Commit(sha.clone()))?;
        let metadata = self.read_commit_metadata(&commit_sha)?;
        let base_sha = metadata.parents.first().cloned();
        let files = self.diff_files(base_sha.as_ref(), &commit_sha, inline_limit, content_path)?;
        let stats = DiffStats::from_files(&files);

        Ok(CommitDetail {
            sha: metadata.sha,
            tree_sha: metadata.tree_sha,
            parents: metadata.parents,
            author: metadata.author,
            authored_at: metadata.authored_at,
            committer: metadata.committer,
            committed_at: metadata.committed_at,
            title: metadata.title,
            message: metadata.message,
            diff: CommitDiff {
                base_sha,
                head_sha: commit_sha,
                stats,
                files,
            },
        })
    }

    pub fn diff_between(
        &self,
        base: Option<&GitSha>,
        head: &GitSha,
        inline_limit: u64,
        content_path: Option<&RepoFilePath>,
    ) -> Result<CommitDiff, RepositoryError> {
        let head_sha = self.resolve_ref(&ValidatedRef::Commit(head.clone()))?;
        let metadata = self.read_commit_metadata(&head_sha)?;
        let base_sha = match base {
            Some(sha) => Some(self.resolve_ref(&ValidatedRef::Commit(sha.clone()))?),
            None => metadata.parents.first().cloned(),
        };
        let files = self.diff_files(base_sha.as_ref(), &head_sha, inline_limit, content_path)?;
        let stats = DiffStats::from_files(&files);

        Ok(CommitDiff {
            base_sha,
            head_sha,
            stats,
            files,
        })
    }

    pub fn resolve_ref(&self, reference: &ValidatedRef) -> Result<GitSha, RepositoryError> {
        match reference {
            ValidatedRef::Branch(branch) => self
                .branch_head(branch)?
                .ok_or_else(|| RepositoryError::BranchMissing(branch.as_str().to_owned())),
            ValidatedRef::Commit(sha) => {
                let rev = format!("{}^{{commit}}", sha.as_str());
                match self.git_run(["rev-parse", "--verify", rev.as_str()]) {
                    Ok(output) => commit::parse_git_sha_output(output.stdout_string()?),
                    Err(RepositoryError::Git(GitProcessError::Failed { .. })) => {
                        Err(RepositoryError::CommitMissing(sha.as_str().to_owned()))
                    }
                    Err(error) => Err(error),
                }
            }
        }
    }

    fn read_commit_metadata(&self, sha: &GitSha) -> Result<CommitMetadata, RepositoryError> {
        let format = "%H%x00%T%x00%P%x00%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00%B%x00";
        let output = self.git_run_owned_with_env(
            vec![
                "show".to_owned(),
                "-s".to_owned(),
                format!("--format={format}"),
                sha.as_str().to_owned(),
            ],
            std::iter::empty::<(&str, &str)>(),
        )?;
        let fields = output.stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
        if fields.len() < 10 {
            return Err(RepositoryError::InvalidGitOutput(
                "invalid commit metadata output".to_owned(),
            ));
        }

        let parents = diff::utf8_field(fields[2])?
            .split_whitespace()
            .map(GitSha::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let message = diff::utf8_field(fields[9])?.to_owned();
        let title = message.lines().next().unwrap_or("").to_owned();

        Ok(CommitMetadata {
            sha: GitSha::parse(diff::utf8_field(fields[0])?)?,
            tree_sha: GitSha::parse(diff::utf8_field(fields[1])?)?,
            parents,
            author: CommitAuthor {
                name: diff::utf8_field(fields[3])?.to_owned(),
                email: diff::utf8_field(fields[4])?.to_owned(),
            },
            authored_at: diff::utf8_field(fields[5])?.to_owned(),
            committer: CommitAuthor {
                name: diff::utf8_field(fields[6])?.to_owned(),
                email: diff::utf8_field(fields[7])?.to_owned(),
            },
            committed_at: diff::utf8_field(fields[8])?.to_owned(),
            title,
            message,
        })
    }

    fn diff_files(
        &self,
        base: Option<&GitSha>,
        head: &GitSha,
        inline_limit: u64,
        content_path: Option<&RepoFilePath>,
    ) -> Result<Vec<DiffFile>, RepositoryError> {
        let raw_output = self.git_run_owned_with_env(
            diff::diff_tree_args("--raw", base, head),
            std::iter::empty::<(&str, &str)>(),
        )?;
        let raw_entries = diff::parse_raw_diff_entries(&raw_output.stdout)?;

        let numstat_output = self.git_run_owned_with_env(
            diff::diff_tree_args("--numstat", base, head),
            std::iter::empty::<(&str, &str)>(),
        )?;
        let numstat_entries = diff::parse_numstat_entries(&numstat_output.stdout)?;
        if raw_entries.len() != numstat_entries.len() {
            return Err(RepositoryError::InvalidGitOutput(format!(
                "diff raw entry count {} did not match numstat count {}",
                raw_entries.len(),
                numstat_entries.len()
            )));
        }

        let selected_index = diff::selected_diff_index(&raw_entries, content_path);
        raw_entries
            .into_iter()
            .zip(numstat_entries)
            .enumerate()
            .map(|(index, (entry, line_stats))| {
                let include_content = selected_index == Some(index);
                let old_file = self.diff_blob_content(
                    entry.old_object_sha.as_ref(),
                    Some(entry.old_mode.as_str()),
                    inline_limit,
                    include_content,
                )?;
                let new_file = self.diff_blob_content(
                    entry.new_object_sha.as_ref(),
                    Some(entry.new_mode.as_str()),
                    inline_limit,
                    include_content,
                )?;
                let binary = line_stats.binary
                    || old_file.kind == DiffContentKind::Binary
                    || new_file.kind == DiffContentKind::Binary;

                Ok(DiffFile {
                    path: entry
                        .new_path
                        .as_ref()
                        .or(entry.old_path.as_ref())
                        .cloned()
                        .unwrap_or_default(),
                    old_path: entry.old_path,
                    new_path: entry.new_path,
                    status: entry.status,
                    old_mode: diff::mode_for_content(entry.old_mode, old_file.kind),
                    new_mode: diff::mode_for_content(entry.new_mode, new_file.kind),
                    additions: line_stats.additions,
                    removals: line_stats.removals,
                    binary,
                    old_file,
                    new_file,
                })
            })
            .collect()
    }

    fn diff_blob_content(
        &self,
        object_sha: Option<&GitSha>,
        mode: Option<&str>,
        inline_limit: u64,
        include_content: bool,
    ) -> Result<DiffFileContent, RepositoryError> {
        let Some(object_sha) = object_sha else {
            return Ok(DiffFileContent::missing());
        };

        if !include_content {
            return Ok(DiffFileContent::unloaded(object_sha, mode));
        }

        if mode == Some(super::FILE_MODE_SUBMODULE) {
            return Ok(DiffFileContent {
                kind: DiffContentKind::Binary,
                mode: mode.map(ToOwned::to_owned),
                size: Some(0),
                encoding: None,
                content: None,
                object_sha: Some(object_sha.clone()),
            });
        }

        let object_type = self.git_run(["cat-file", "-t", object_sha.as_str()])?;
        if object_type.stdout_string()?.trim() != "blob" {
            return Ok(DiffFileContent {
                kind: DiffContentKind::Binary,
                mode: mode.map(ToOwned::to_owned),
                size: Some(0),
                encoding: None,
                content: None,
                object_sha: Some(object_sha.clone()),
            });
        }

        let size_output = self.git_run(["cat-file", "-s", object_sha.as_str()])?;
        let size = size_output
            .stdout_string()?
            .trim()
            .parse::<u64>()
            .map_err(|_| {
                RepositoryError::InvalidGitOutput("invalid blob size output".to_owned())
            })?;

        if size > inline_limit {
            return Ok(DiffFileContent {
                kind: DiffContentKind::TooLarge,
                mode: mode.map(ToOwned::to_owned),
                size: Some(size),
                encoding: None,
                content: None,
                object_sha: Some(object_sha.clone()),
            });
        }

        let output = self.git_run(["cat-file", "-p", object_sha.as_str()])?;
        if output.stdout.contains(&0) {
            return Ok(DiffFileContent {
                kind: DiffContentKind::Binary,
                mode: mode.map(ToOwned::to_owned),
                size: Some(size),
                encoding: None,
                content: None,
                object_sha: Some(object_sha.clone()),
            });
        }

        match String::from_utf8(output.stdout) {
            Ok(content) => Ok(DiffFileContent {
                kind: DiffContentKind::Text,
                mode: mode.map(ToOwned::to_owned),
                size: Some(size),
                encoding: Some("utf-8".to_owned()),
                content: Some(content),
                object_sha: Some(object_sha.clone()),
            }),
            Err(_) => Ok(DiffFileContent {
                kind: DiffContentKind::Binary,
                mode: mode.map(ToOwned::to_owned),
                size: Some(size),
                encoding: None,
                content: None,
                object_sha: Some(object_sha.clone()),
            }),
        }
    }

    fn branch_head(&self, branch: &BranchName) -> Result<Option<GitSha>, RepositoryError> {
        let branch_ref = branch.ref_name();
        match self.git_run(["show-ref", "--verify", "--hash", branch_ref.as_str()]) {
            Ok(output) => Ok(Some(commit::parse_git_sha_output(output.stdout_string()?)?)),
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
        Ok(vec![
            "--git-dir".to_owned(),
            commit::path_to_arg(&self.path)?,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::DiffFileStatus;
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
    fn lists_recursive_tree_entries_with_full_paths() {
        let (_temp, repo) = test_repo();
        repo.create_commit(CommitRequest {
            target_branch: BranchName::parse("main").unwrap(),
            expected_head_sha: None,
            message: "Initial commit".to_owned(),
            author: CommitAuthor {
                name: "Kian".to_owned(),
                email: "kian@example.com".to_owned(),
            },
            changes: vec![
                CommitChange::Upsert {
                    path: RepoFilePath::parse_file("README.md").unwrap(),
                    content: b"# Depo\n".to_vec(),
                    mode: "100644".to_owned(),
                },
                CommitChange::Upsert {
                    path: RepoFilePath::parse_file("src/main.rs").unwrap(),
                    content: b"fn main() {}\n".to_vec(),
                    mode: "100644".to_owned(),
                },
                CommitChange::Upsert {
                    path: RepoFilePath::parse_file("src/lib/mod.rs").unwrap(),
                    content: b"pub mod storage;\n".to_vec(),
                    mode: "100644".to_owned(),
                },
            ],
        })
        .unwrap();

        let reference = ValidatedRef::Branch(BranchName::parse("main").unwrap());
        let (_, tree) = repo
            .list_tree_recursive(&reference, &RepoFilePath::root())
            .unwrap();
        let entries = tree
            .iter()
            .map(|entry| (entry.path.as_str(), entry.kind))
            .collect::<Vec<_>>();

        assert!(entries.contains(&("README.md", TreeEntryKind::File)));
        assert!(entries.contains(&("src", TreeEntryKind::Directory)));
        assert!(entries.contains(&("src/main.rs", TreeEntryKind::File)));
        assert!(entries.contains(&("src/lib", TreeEntryKind::Directory)));
        assert!(entries.contains(&("src/lib/mod.rs", TreeEntryKind::File)));
    }

    #[test]
    fn returns_root_commit_detail_with_inline_diff() {
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

        let detail = repo.commit_detail(&result.sha, 1024 * 1024, None).unwrap();
        assert_eq!(detail.sha, result.sha);
        assert!(detail.parents.is_empty());
        assert_eq!(detail.title, "Initial commit");
        assert_eq!(detail.diff.base_sha, None);
        assert_eq!(detail.diff.stats.files_changed, 1);
        assert_eq!(detail.diff.stats.additions, 1);

        let file = &detail.diff.files[0];
        assert_eq!(file.status, DiffFileStatus::Added);
        assert_eq!(file.path, "README.md");
        assert_eq!(file.old_file.kind, DiffContentKind::Missing);
        assert_eq!(file.new_file.kind, DiffContentKind::Text);
        assert_eq!(file.new_file.content.as_deref(), Some("# Depo\n"));
    }

    #[test]
    fn returns_first_parent_file_diff_for_commit_update() {
        let (_temp, repo) = test_repo();
        let first = repo
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
        let second = repo
            .create_commit(CommitRequest {
                target_branch: BranchName::parse("main").unwrap(),
                expected_head_sha: Some(first.sha.clone()),
                message: "Expand README".to_owned(),
                author: CommitAuthor {
                    name: "Kian".to_owned(),
                    email: "kian@example.com".to_owned(),
                },
                changes: vec![
                    CommitChange::Upsert {
                        path: RepoFilePath::parse_file("README.md").unwrap(),
                        content: b"# Depo\n\nReal code hosting.\n".to_vec(),
                        mode: "100644".to_owned(),
                    },
                    CommitChange::Upsert {
                        path: RepoFilePath::parse_file("src/main.rs").unwrap(),
                        content: b"fn main() {}\n".to_vec(),
                        mode: "100644".to_owned(),
                    },
                ],
            })
            .unwrap();

        let detail = repo.commit_detail(&second.sha, 1024 * 1024, None).unwrap();
        assert_eq!(detail.parents, vec![first.sha.clone()]);
        assert_eq!(detail.diff.base_sha, Some(first.sha.clone()));
        assert_eq!(detail.diff.head_sha, second.sha);

        let file = &detail.diff.files[0];
        assert_eq!(file.status, DiffFileStatus::Modified);
        assert_eq!(file.old_file.content.as_deref(), Some("# Depo\n"));
        assert_eq!(
            file.new_file.content.as_deref(),
            Some("# Depo\n\nReal code hosting.\n")
        );
        let unloaded_file = detail
            .diff
            .files
            .iter()
            .find(|file| file.path == "src/main.rs")
            .unwrap();
        assert_eq!(unloaded_file.new_file.kind, DiffContentKind::Unloaded);

        let selected_path = RepoFilePath::parse_file("src/main.rs").unwrap();
        let selected_detail = repo
            .commit_detail(&detail.sha, 1024 * 1024, Some(&selected_path))
            .unwrap();
        let selected_file = selected_detail
            .diff
            .files
            .iter()
            .find(|file| file.path == "src/main.rs")
            .unwrap();
        assert_eq!(selected_file.new_file.kind, DiffContentKind::Text);
        assert_eq!(
            selected_file.new_file.content.as_deref(),
            Some("fn main() {}\n")
        );

        let diff = repo
            .diff_between(Some(&first.sha), &detail.sha, 1024 * 1024, None)
            .unwrap();
        assert_eq!(diff.base_sha, Some(first.sha));
        assert_eq!(diff.files[0].path, "README.md");
    }

    #[test]
    fn returns_commit_missing_for_unknown_commit() {
        let (_temp, repo) = test_repo();
        let missing = GitSha::parse("1111111111111111111111111111111111111111").unwrap();
        let error = repo.commit_detail(&missing, 1024 * 1024, None).unwrap_err();

        assert!(matches!(error, RepositoryError::CommitMissing(sha) if sha == missing.as_str()));
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
