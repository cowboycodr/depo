use super::{
    GitSha, RepoFilePath, ZERO_SHA,
    types::{DiffFileStatus, DiffLineStats, RawDiffEntry},
};

use super::repository::RepositoryError;

pub(crate) fn utf8_field(value: &[u8]) -> Result<&str, RepositoryError> {
    std::str::from_utf8(value).map_err(|error| {
        RepositoryError::InvalidGitOutput(format!("git output was not valid UTF-8: {error}"))
    })
}

pub(crate) fn parse_optional_sha(value: &str) -> Result<Option<GitSha>, RepositoryError> {
    if value == ZERO_SHA {
        Ok(None)
    } else {
        Ok(Some(GitSha::parse(value)?))
    }
}

pub(crate) fn parse_numstat_count(value: &str) -> Result<u32, RepositoryError> {
    if value == "-" {
        Ok(0)
    } else {
        value.parse::<u32>().map_err(|_| {
            RepositoryError::InvalidGitOutput(format!("invalid numstat count {value:?}"))
        })
    }
}

pub(crate) fn parse_diff_status(value: &str) -> DiffFileStatus {
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

pub(crate) fn mode_for_content(
    mode: String,
    kind: super::types::DiffContentKind,
) -> Option<String> {
    match kind {
        super::types::DiffContentKind::Missing => None,
        _ => Some(mode),
    }
}

pub(crate) fn selected_diff_index(
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

pub(crate) fn diff_tree_args(kind: &str, base: Option<&GitSha>, head: &GitSha) -> Vec<String> {
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

pub(crate) fn parse_raw_diff_entries(output: &[u8]) -> Result<Vec<RawDiffEntry>, RepositoryError> {
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

pub(crate) fn parse_numstat_entries(output: &[u8]) -> Result<Vec<DiffLineStats>, RepositoryError> {
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
