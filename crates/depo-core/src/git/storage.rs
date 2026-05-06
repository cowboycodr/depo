use std::fs;
use std::path::{Path, PathBuf};

use super::RepoId;

#[derive(Debug, thiserror::Error)]
pub enum StorageRootError {
    #[error("storage root path is empty")]
    Empty,
    #[error("failed to resolve current directory: {0}")]
    CurrentDir(std::io::Error),
    #[error("failed to create storage directory {path:?}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct StorageRoot {
    root: PathBuf,
}

impl StorageRoot {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StorageRootError> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err(StorageRootError::Empty);
        }

        let absolute = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()
                .map_err(StorageRootError::CurrentDir)?
                .join(root)
        };

        Ok(Self { root: absolute })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn ensure_exists(&self) -> Result<(), StorageRootError> {
        fs::create_dir_all(&self.root).map_err(|source| StorageRootError::CreateDir {
            path: self.root.clone(),
            source,
        })
    }

    pub fn owner_dir(&self, id: &RepoId) -> PathBuf {
        self.root.join(id.owner().as_str())
    }

    pub fn bare_repo_path(&self, id: &RepoId) -> PathBuf {
        self.owner_dir(id)
            .join(format!("{}.git", id.name().as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::RepoId;

    #[test]
    fn builds_path_safe_storage_location() {
        let temp = tempfile::tempdir().unwrap();
        let root = StorageRoot::new(temp.path()).unwrap();
        let id = RepoId::parse("kian", "depo").unwrap();

        assert_eq!(root.bare_repo_path(&id), temp.path().join("kian/depo.git"));
    }
}
