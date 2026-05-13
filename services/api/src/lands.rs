use std::collections::{BTreeSet, HashMap};

use depo_core::git::{BareRepository, GitSha, RefHead, ZERO_SHA};
use sqlx::SqlitePool;

use crate::db;

const STORED_COMMIT_LIMIT: usize = 100;
const BRANCH_REF_PREFIX: &str = "refs/heads/";

#[derive(Debug, thiserror::Error)]
pub enum LandRecordError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Repository(#[from] depo_core::git::RepositoryError),
}

struct RefUpdate<'a> {
    repo_id: &'a str,
    actor: &'a str,
    source: &'a str,
    ref_name: &'a str,
    old_sha: GitSha,
    new_sha: GitSha,
    previous_heads: Vec<GitSha>,
}

pub async fn record_ref_changes(
    pool: &SqlitePool,
    repo: &BareRepository,
    repo_id: &str,
    actor: &str,
    source: &str,
    before: &[RefHead],
    after: &[RefHead],
) -> Result<Vec<db::LandRecord>, LandRecordError> {
    let before_by_ref = before
        .iter()
        .map(|head| (head.ref_name.as_str(), &head.sha))
        .collect::<HashMap<_, _>>();
    let after_by_ref = after
        .iter()
        .map(|head| (head.ref_name.as_str(), &head.sha))
        .collect::<HashMap<_, _>>();
    let refs = before_by_ref
        .keys()
        .chain(after_by_ref.keys())
        .filter(|ref_name| ref_name.starts_with(BRANCH_REF_PREFIX))
        .copied()
        .collect::<BTreeSet<_>>();

    let mut lands = Vec::new();
    for ref_name in refs {
        let old_sha = before_by_ref
            .get(ref_name)
            .copied()
            .cloned()
            .unwrap_or_else(GitSha::zero);
        let new_sha = after_by_ref
            .get(ref_name)
            .copied()
            .cloned()
            .unwrap_or_else(GitSha::zero);
        if old_sha == new_sha {
            continue;
        }

        let previous_heads = before
            .iter()
            .filter(|head| head.ref_name != ref_name)
            .map(|head| head.sha.clone())
            .collect::<Vec<_>>();
        lands.push(
            record_ref_update(
                pool,
                repo,
                RefUpdate {
                    repo_id,
                    actor,
                    source,
                    ref_name,
                    old_sha,
                    new_sha,
                    previous_heads,
                },
            )
            .await?,
        );
    }

    Ok(lands)
}

async fn record_ref_update(
    pool: &SqlitePool,
    repo: &BareRepository,
    update: RefUpdate<'_>,
) -> Result<db::LandRecord, LandRecordError> {
    let commits = repo.landed_commits(
        &update.old_sha,
        &update.new_sha,
        &update.previous_heads,
        STORED_COMMIT_LIMIT,
    )?;
    let commit_count =
        repo.landed_commit_count(&update.old_sha, &update.new_sha, &update.previous_heads)? as i64;
    let additions = commits
        .iter()
        .map(|commit| i64::from(commit.additions))
        .sum();
    let removals = commits
        .iter()
        .map(|commit| i64::from(commit.removals))
        .sum();
    let head_title = commits.first().map(|commit| commit.title.clone());

    Ok(db::insert_land(
        pool,
        db::NewLand {
            repo_id: update.repo_id.to_owned(),
            actor: update.actor.to_owned(),
            source: update.source.to_owned(),
            ref_name: update.ref_name.to_owned(),
            short_ref: short_ref(update.ref_name).to_owned(),
            old_sha: update.old_sha.as_str().to_owned(),
            new_sha: update.new_sha.as_str().to_owned(),
            kind: branch_land_kind(&update.old_sha, &update.new_sha).to_owned(),
            status: "received".to_owned(),
            head_title,
            commit_count,
            additions,
            removals,
            commits: commits
                .into_iter()
                .map(|commit| db::NewLandCommit {
                    sha: commit.sha.as_str().to_owned(),
                    title: commit.title,
                    author_name: commit.author.name,
                    author_email: commit.author.email,
                    committed_at: commit.committed_at,
                    additions: i64::from(commit.additions),
                    removals: i64::from(commit.removals),
                })
                .collect(),
        },
    )
    .await?)
}

fn branch_land_kind(old_sha: &GitSha, new_sha: &GitSha) -> &'static str {
    match (old_sha.as_str() == ZERO_SHA, new_sha.as_str() == ZERO_SHA) {
        (true, false) => "branch_created",
        (false, true) => "branch_deleted",
        _ => "branch_updated",
    }
}

fn short_ref(ref_name: &str) -> &str {
    ref_name.strip_prefix(BRANCH_REF_PREFIX).unwrap_or(ref_name)
}
