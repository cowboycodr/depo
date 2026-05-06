use serde::{Deserialize, Serialize};
use std::fmt;

pub const ZERO_SHA: &str = "0000000000000000000000000000000000000000";

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum IdError {
    #[error("{field} is required")]
    Empty { field: &'static str },
    #[error("{field} is too long; max length is {max}")]
    TooLong { field: &'static str, max: usize },
    #[error("{field} has an unsupported value: {reason}")]
    Invalid {
        field: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoOwner(String);

impl RepoOwner {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, IdError> {
        validate_slug("owner", value.as_ref(), 64)?;
        Ok(Self(value.as_ref().to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoName(String);

impl RepoName {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, IdError> {
        validate_slug("repo", value.as_ref(), 100)?;
        Ok(Self(value.as_ref().to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoId {
    owner: RepoOwner,
    name: RepoName,
}

impl RepoId {
    pub fn new(owner: RepoOwner, name: RepoName) -> Self {
        Self { owner, name }
    }

    pub fn parse(owner: impl AsRef<str>, name: impl AsRef<str>) -> Result<Self, IdError> {
        Ok(Self {
            owner: RepoOwner::parse(owner)?,
            name: RepoName::parse(name)?,
        })
    }

    pub fn parse_full(value: impl AsRef<str>) -> Result<Self, IdError> {
        let value = value.as_ref();
        let (owner, name) = value.split_once('/').ok_or(IdError::Invalid {
            field: "repo_id",
            reason: "expected owner/repo format",
        })?;

        if name.contains('/') {
            return Err(IdError::Invalid {
                field: "repo_id",
                reason: "expected exactly one slash",
            });
        }

        Self::parse(owner, name)
    }

    pub fn owner(&self) -> &RepoOwner {
        &self.owner
    }

    pub fn name(&self) -> &RepoName {
        &self.name
    }

    pub fn as_full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

impl fmt::Display for RepoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchName(String);

impl BranchName {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, IdError> {
        validate_branch_name(value.as_ref())?;
        Ok(Self(value.as_ref().to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn ref_name(&self) -> String {
        format!("refs/heads/{}", self.0)
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GitSha(String);

impl GitSha {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, IdError> {
        let value = value.as_ref();
        if value.len() != 40 {
            return Err(IdError::Invalid {
                field: "sha",
                reason: "expected a full 40 character SHA-1",
            });
        }
        if !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(IdError::Invalid {
                field: "sha",
                reason: "expected hexadecimal characters only",
            });
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn zero() -> Self {
        Self(ZERO_SHA.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoFilePath(String);

impl RepoFilePath {
    pub fn parse_file(value: impl AsRef<str>) -> Result<Self, IdError> {
        validate_repo_path(value.as_ref(), false)?;
        Ok(Self(value.as_ref().to_owned()))
    }

    pub fn parse_tree(value: impl AsRef<str>) -> Result<Self, IdError> {
        validate_repo_path(value.as_ref(), true)?;
        Ok(Self(value.as_ref().to_owned()))
    }

    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn join_child(&self, child: &str) -> String {
        if self.0.is_empty() {
            child.to_owned()
        } else {
            format!("{}/{}", self.0, child)
        }
    }
}

impl fmt::Display for RepoFilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatedRef {
    Branch(BranchName),
    Commit(GitSha),
}

impl ValidatedRef {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, IdError> {
        let value = value.as_ref();
        if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(Self::Commit(GitSha::parse(value)?));
        }
        Ok(Self::Branch(BranchName::parse(value)?))
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Branch(branch) => branch.as_str().to_owned(),
            Self::Commit(sha) => sha.as_str().to_owned(),
        }
    }
}

fn validate_slug(field: &'static str, value: &str, max: usize) -> Result<(), IdError> {
    if value.is_empty() {
        return Err(IdError::Empty { field });
    }
    if value.len() > max {
        return Err(IdError::TooLong { field, max });
    }
    if value == "." || value == ".." {
        return Err(IdError::Invalid {
            field,
            reason: "reserved dot segment",
        });
    }
    if value.starts_with('.') || value.ends_with('.') {
        return Err(IdError::Invalid {
            field,
            reason: "must not start or end with a dot",
        });
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(IdError::Invalid {
            field,
            reason: "allowed characters are ASCII letters, numbers, dash, underscore, and dot",
        });
    }
    Ok(())
}

fn validate_branch_name(value: &str) -> Result<(), IdError> {
    if value.is_empty() {
        return Err(IdError::Empty { field: "branch" });
    }
    if value.len() > 255 {
        return Err(IdError::TooLong {
            field: "branch",
            max: 255,
        });
    }
    if value.starts_with("refs/") {
        return Err(IdError::Invalid {
            field: "branch",
            reason: "expected a branch name, not a full ref",
        });
    }
    if value.starts_with('-') || value.starts_with('/') || value.ends_with('/') {
        return Err(IdError::Invalid {
            field: "branch",
            reason: "must not start with dash or slash, or end with slash",
        });
    }
    if value.ends_with('.') || value.ends_with(".lock") {
        return Err(IdError::Invalid {
            field: "branch",
            reason: "must not end with dot or .lock",
        });
    }
    if value.contains("..") || value.contains("//") || value.contains("@{") {
        return Err(IdError::Invalid {
            field: "branch",
            reason: "contains a forbidden Git ref sequence",
        });
    }
    if value.bytes().any(|b| {
        b.is_ascii_control()
            || b.is_ascii_whitespace()
            || matches!(b, b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
    }) {
        return Err(IdError::Invalid {
            field: "branch",
            reason: "contains characters that are unsafe in Git refs",
        });
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment.starts_with('.') || segment.ends_with(".lock"))
    {
        return Err(IdError::Invalid {
            field: "branch",
            reason: "contains an invalid ref path segment",
        });
    }
    Ok(())
}

fn validate_repo_path(value: &str, allow_empty: bool) -> Result<(), IdError> {
    if value.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(IdError::Empty { field: "path" })
        };
    }
    if value.len() > 4096 {
        return Err(IdError::TooLong {
            field: "path",
            max: 4096,
        });
    }
    if value.starts_with('/') || value.ends_with('/') {
        return Err(IdError::Invalid {
            field: "path",
            reason: "must be a relative file path without trailing slash",
        });
    }
    if value.contains('\\') || value.contains('\0') || value.contains(':') {
        return Err(IdError::Invalid {
            field: "path",
            reason: "contains a character reserved by Depo's Git path boundary",
        });
    }
    if value.bytes().any(|b| b.is_ascii_control()) {
        return Err(IdError::Invalid {
            field: "path",
            reason: "contains control characters",
        });
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(IdError::Invalid {
            field: "path",
            reason: "must not contain empty, dot, or parent directory segments",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_repo_identity() {
        let id = RepoId::parse("kian", "depo.api").unwrap();
        assert_eq!(id.as_full_name(), "kian/depo.api");

        assert!(RepoId::parse("../kian", "depo").is_err());
        assert!(RepoId::parse("kian", "depo/name").is_err());
        assert!(RepoId::parse_full("kian/depo/extra").is_err());
    }

    #[test]
    fn validates_branch_names() {
        assert!(BranchName::parse("main").is_ok());
        assert!(BranchName::parse("feature/sidebar").is_ok());
        assert!(BranchName::parse("-bad").is_err());
        assert!(BranchName::parse("bad..branch").is_err());
        assert!(BranchName::parse("bad branch").is_err());
        assert!(BranchName::parse("refs/heads/main").is_err());
    }

    #[test]
    fn validates_full_sha() {
        let sha = GitSha::parse("0123456789abcdef0123456789ABCDEF01234567").unwrap();
        assert_eq!(sha.as_str(), "0123456789abcdef0123456789abcdef01234567");
        assert!(GitSha::parse("abc").is_err());
        assert!(GitSha::parse("zzzz456789abcdef0123456789abcdef01234567").is_err());
    }

    #[test]
    fn validates_repo_file_paths() {
        assert!(RepoFilePath::parse_file("src/main.rs").is_ok());
        assert!(RepoFilePath::parse_tree("").is_ok());
        assert!(RepoFilePath::parse_file("").is_err());
        assert!(RepoFilePath::parse_file("../main.rs").is_err());
        assert!(RepoFilePath::parse_file("/src/main.rs").is_err());
        assert!(RepoFilePath::parse_file("src//main.rs").is_err());
        assert!(RepoFilePath::parse_file("src:main.rs").is_err());
    }
}
