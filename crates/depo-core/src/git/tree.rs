use super::{GitSha, RepoFilePath, types::TreeEntry, types::TreeEntryKind};
use super::repository::RepositoryError;

pub(crate) fn treeish_for_path(commit_sha: &GitSha, path: &RepoFilePath) -> String {
    if path.is_root() {
        commit_sha.as_str().to_owned()
    } else {
        format!("{}:{}", commit_sha.as_str(), path.as_str())
    }
}

pub(crate) fn parent_tree_path(path: &RepoFilePath) -> Result<RepoFilePath, RepositoryError> {
    match path.as_str().rsplit_once('/') {
        Some((parent, _)) => Ok(RepoFilePath::parse_tree(parent)?),
        None => Ok(RepoFilePath::root()),
    }
}

pub(crate) fn parse_tree_entries(
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
