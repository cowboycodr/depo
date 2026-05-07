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

    pub fn list_tree_recursive(
        &self,
        reference: &ValidatedRef,
        path: &RepoFilePath,
    ) -> Result<(GitSha, Vec<TreeEntry>), RepositoryError> {
        let commit_sha = self.resolve_ref(reference)?;
        let treeish = treeish_for_path(&commit_sha, path);
        let output = self.git_run(["ls-tree", "-z", "-l", "-r", "-t", treeish.as_str()])?;
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
                "--format=%x00%H%x09%an%x09%ae%x09%cI%x09%s".to_owned(),
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
            commits.push(parse_commit_with_stats(record)?);
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
                "--format=%H%x00%an%x00%ae%x00%cI%x00%s%x00%b%x00".to_owned(),
                commit_sha.as_str().to_owned(),
                "--".to_owned(),
                path.as_str().to_owned(),
            ],
            std::iter::empty::<(&str, &str)>(),
        )?;

        let fields = output.stdout.split(|b| *b == 0).collect::<Vec<_>>();
        if fields.len() < 5 || fields[0].is_empty() {
            return Err(RepositoryError::PathNotFound(path.as_str().to_owned()));
        }

        let sha = utf8_field(fields[0])?.trim();
        if sha.is_empty() {
            return Err(RepositoryError::PathNotFound(path.as_str().to_owned()));
        }

        let description = if fields.len() > 5 {
            let body = utf8_field(fields[5])?.trim();
            if body.is_empty() { None } else { Some(body.to_owned()) }
        } else {
            None
        };

        Ok(CommitSummary {
            sha: GitSha::parse(sha)?,
            title: utf8_field(fields[4])?.trim_end().to_owned(),
            author: CommitAuthor {
                name: utf8_field(fields[1])?.to_owned(),
                email: utf8_field(fields[2])?.to_owned(),
            },
            committed_at: utf8_field(fields[3])?.trim_end().to_owned(),
            additions: 0,
            removals: 0,
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
                    Ok(output) => parse_git_sha_output(output.stdout_string()?),
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

        let parents = utf8_field(fields[2])?
            .split_whitespace()
            .map(GitSha::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let message = utf8_field(fields[9])?.to_owned();
        let title = message.lines().next().unwrap_or("").to_owned();

        Ok(CommitMetadata {
            sha: GitSha::parse(utf8_field(fields[0])?)?,
            tree_sha: GitSha::parse(utf8_field(fields[1])?)?,
            parents,
            author: CommitAuthor {
                name: utf8_field(fields[3])?.to_owned(),
                email: utf8_field(fields[4])?.to_owned(),
            },
            authored_at: utf8_field(fields[5])?.to_owned(),
            committer: CommitAuthor {
                name: utf8_field(fields[6])?.to_owned(),
                email: utf8_field(fields[7])?.to_owned(),
            },
            committed_at: utf8_field(fields[8])?.to_owned(),
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
            diff_tree_args("--raw", base, head),
            std::iter::empty::<(&str, &str)>(),
        )?;
        let raw_entries = parse_raw_diff_entries(&raw_output.stdout)?;

        let numstat_output = self.git_run_owned_with_env(
            diff_tree_args("--numstat", base, head),
            std::iter::empty::<(&str, &str)>(),
        )?;
        let numstat_entries = parse_numstat_entries(&numstat_output.stdout)?;
        if raw_entries.len() != numstat_entries.len() {
            return Err(RepositoryError::InvalidGitOutput(format!(
                "diff raw entry count {} did not match numstat count {}",
                raw_entries.len(),
                numstat_entries.len()
            )));
        }

        let selected_index = selected_diff_index(&raw_entries, content_path);
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
                    old_mode: mode_for_content(entry.old_mode, &old_file),
                    new_mode: mode_for_content(entry.new_mode, &new_file),
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

        if mode == Some("160000") {
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
struct CommitMetadata {
    sha: GitSha,
    tree_sha: GitSha,
    parents: Vec<GitSha>,
    author: CommitAuthor,
    authored_at: String,
    committer: CommitAuthor,
    committed_at: String,
    title: String,
    message: String,
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
    fn from_files(files: &[DiffFile]) -> Self {
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
    fn missing() -> Self {
        Self {
            kind: DiffContentKind::Missing,
            mode: None,
            size: None,
            encoding: None,
            content: None,
            object_sha: None,
        }
    }

    fn unloaded(object_sha: &GitSha, mode: Option<&str>) -> Self {
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
struct RawDiffEntry {
    old_mode: String,
    new_mode: String,
    old_object_sha: Option<GitSha>,
    new_object_sha: Option<GitSha>,
    status: DiffFileStatus,
    old_path: Option<String>,
    new_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DiffLineStats {
    additions: u32,
    removals: u32,
    binary: bool,
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

fn diff_tree_args(kind: &str, base: Option<&GitSha>, head: &GitSha) -> Vec<String> {
    let mut args = vec![
        "diff-tree".to_owned(),
        "-r".to_owned(),
        "-z".to_owned(),
        kind.to_owned(),
        "--no-commit-id".to_owned(),
        "--find-renames".to_owned(),
        "--find-copies".to_owned(),
    ];

    if kind == "--raw" {
        args.push("--full-index".to_owned());
        args.push("--abbrev=40".to_owned());
    }

    match base {
        Some(base) => {
            args.push(base.as_str().to_owned());
            args.push(head.as_str().to_owned());
        }
        None => {
            args.push("--root".to_owned());
            args.push(head.as_str().to_owned());
        }
    }

    args
}

fn parse_raw_diff_entries(output: &[u8]) -> Result<Vec<RawDiffEntry>, RepositoryError> {
    let mut records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());
    let mut entries = Vec::new();

    while let Some(metadata_record) = records.next() {
        let metadata = utf8_field(metadata_record)?;
        let Some(metadata) = metadata.strip_prefix(':') else {
            return Err(RepositoryError::InvalidGitOutput(
                "diff raw metadata did not start with ':'".to_owned(),
            ));
        };
        let fields = metadata.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(RepositoryError::InvalidGitOutput(format!(
                "expected 5 diff raw fields in {metadata:?}"
            )));
        }

        let status = parse_diff_status(fields[4]);
        let path = records
            .next()
            .ok_or_else(|| RepositoryError::InvalidGitOutput("missing diff path".to_owned()))
            .and_then(|record| Ok(utf8_field(record)?.to_owned()))?;

        let (old_path, new_path) = match status {
            DiffFileStatus::Added => (None, Some(path)),
            DiffFileStatus::Deleted => (Some(path), None),
            DiffFileStatus::Renamed | DiffFileStatus::Copied => {
                let new_path = records
                    .next()
                    .ok_or_else(|| {
                        RepositoryError::InvalidGitOutput(
                            "missing rename or copy destination path".to_owned(),
                        )
                    })
                    .and_then(|record| Ok(utf8_field(record)?.to_owned()))?;
                (Some(path), Some(new_path))
            }
            DiffFileStatus::Modified | DiffFileStatus::TypeChanged | DiffFileStatus::Unknown => {
                (Some(path.clone()), Some(path))
            }
        };

        entries.push(RawDiffEntry {
            old_mode: fields[0].to_owned(),
            new_mode: fields[1].to_owned(),
            old_object_sha: parse_optional_sha(fields[2])?,
            new_object_sha: parse_optional_sha(fields[3])?,
            status,
            old_path,
            new_path,
        });
    }

    Ok(entries)
}

fn parse_numstat_entries(output: &[u8]) -> Result<Vec<DiffLineStats>, RepositoryError> {
    let mut records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());
    let mut entries = Vec::new();

    while let Some(record) = records.next() {
        let text = utf8_field(record)?;
        let fields = text.split('\t').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(RepositoryError::InvalidGitOutput(format!(
                "expected 3 numstat fields in {text:?}"
            )));
        }

        if fields[2].is_empty() {
            let _old_path = records.next().ok_or_else(|| {
                RepositoryError::InvalidGitOutput("missing numstat old path".to_owned())
            })?;
            let _new_path = records.next().ok_or_else(|| {
                RepositoryError::InvalidGitOutput("missing numstat new path".to_owned())
            })?;
        }

        let binary = fields[0] == "-" || fields[1] == "-";
        entries.push(DiffLineStats {
            additions: parse_numstat_count(fields[0])?,
            removals: parse_numstat_count(fields[1])?,
            binary,
        });
    }

    Ok(entries)
}

fn selected_diff_index(
    entries: &[RawDiffEntry],
    content_path: Option<&RepoFilePath>,
) -> Option<usize> {
    if entries.is_empty() {
        return None;
    }

    let Some(content_path) = content_path else {
        return Some(0);
    };

    entries
        .iter()
        .position(|entry| {
            entry.old_path.as_deref() == Some(content_path.as_str())
                || entry.new_path.as_deref() == Some(content_path.as_str())
        })
        .or(Some(0))
}

fn parse_diff_status(value: &str) -> DiffFileStatus {
    match value.chars().next() {
        Some('A') => DiffFileStatus::Added,
        Some('M') => DiffFileStatus::Modified,
        Some('D') => DiffFileStatus::Deleted,
        Some('R') => DiffFileStatus::Renamed,
        Some('C') => DiffFileStatus::Copied,
        Some('T') => DiffFileStatus::TypeChanged,
        _ => DiffFileStatus::Unknown,
    }
}

fn parse_optional_sha(value: &str) -> Result<Option<GitSha>, RepositoryError> {
    if value == ZERO_SHA {
        Ok(None)
    } else {
        Ok(Some(GitSha::parse(value)?))
    }
}

fn parse_numstat_count(value: &str) -> Result<u32, RepositoryError> {
    if value == "-" {
        Ok(0)
    } else {
        value.parse::<u32>().map_err(|_| {
            RepositoryError::InvalidGitOutput(format!("invalid numstat count {value:?}"))
        })
    }
}

fn utf8_field(value: &[u8]) -> Result<&str, RepositoryError> {
    std::str::from_utf8(value).map_err(|error| {
        RepositoryError::InvalidGitOutput(format!("git output was not valid UTF-8: {error}"))
    })
}

fn mode_for_content(mode: String, content: &DiffFileContent) -> Option<String> {
    match content.kind {
        DiffContentKind::Missing => None,
        _ => Some(mode),
    }
}

fn parse_commit_with_stats(record: &str) -> Result<CommitSummary, RepositoryError> {
    let mut lines = record.lines();

    let header = lines.next().unwrap_or("").trim();
    if header.is_empty() {
        return Err(RepositoryError::InvalidGitOutput(
            "empty commit record".to_owned(),
        ));
    }

    let fields: Vec<&str> = header.splitn(5, '\t').collect();
    if fields.len() != 5 {
        return Err(RepositoryError::InvalidGitOutput(format!(
            "invalid commit fields in {:?}",
            header
        )));
    }

    let mut additions = 0u32;
    let mut removals = 0u32;
    for line in lines {
        if line.contains("changed") {
            let (a, r) = parse_shortstat_line(line);
            additions = a;
            removals = r;
            break;
        }
    }

    Ok(CommitSummary {
        sha: GitSha::parse(fields[0])?,
        title: fields[4].trim_end().to_owned(),
        author: CommitAuthor {
            name: fields[1].to_owned(),
            email: fields[2].to_owned(),
        },
        committed_at: fields[3].to_owned(),
        additions,
        removals,
        description: None,
    })
}

fn parse_shortstat_line(line: &str) -> (u32, u32) {
    let mut additions = 0u32;
    let mut removals = 0u32;

    if let Some(pos) = line.find(" insertion") {
        let start = line[..pos].rfind(' ').map_or(0, |i| i + 1);
        additions = line[start..pos].trim().parse().unwrap_or(0);
    }
    if let Some(pos) = line.find(" deletion") {
        let start = line[..pos].rfind(' ').map_or(0, |i| i + 1);
        removals = line[start..pos].trim().parse().unwrap_or(0);
    }

    (additions, removals)
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
