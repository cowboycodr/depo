use std::path::Path;

use super::repository::RepositoryError;
use super::{GitSha, types::CommitAuthor, types::CommitSummary};

pub(crate) fn path_to_arg(path: &Path) -> Result<String, RepositoryError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| RepositoryError::NonUtf8Path(path.to_path_buf()))
}

pub(crate) fn parse_git_sha_output(output: String) -> Result<GitSha, RepositoryError> {
    GitSha::parse(output.trim()).map_err(RepositoryError::Id)
}

pub(crate) fn validate_file_mode(mode: &str) -> Result<(), RepositoryError> {
    match mode {
        super::FILE_MODE_REGULAR | super::FILE_MODE_EXECUTABLE => Ok(()),
        other => Err(RepositoryError::UnsupportedFileMode(other.to_owned())),
    }
}

pub(crate) fn parse_commit_with_stats(record: &str) -> Result<CommitSummary, RepositoryError> {
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
