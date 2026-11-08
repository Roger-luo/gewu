//! What a reviewer chooses from when starting a review: recent commits,
//! branches, open pull requests, and directories.

use std::process::Command;

use serde::{Deserialize, Serialize};

use super::{GitError, Repository};

/// Directories deeper than this are not offered; a reviewer can type one.
const DIRECTORY_DEPTH: usize = 2;
const COMMITS: &str = "30";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommitSummary {
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    /// False for the repository's first commit, which has no parent to use as Base.
    #[serde(default = "CommitSummary::has_parent_default")]
    pub has_parent: bool,
}

impl CommitSummary {
    fn has_parent_default() -> bool {
        true
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PullRequestSummary {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub head_ref: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepositoryRefs {
    pub branch: Option<String>,
    pub dirty: bool,
    pub commits: Vec<CommitSummary>,
    pub branches: Vec<String>,
    /// `None` when `gh` is not installed or cannot see the repository.
    pub pull_requests: Option<Vec<PullRequestSummary>>,
    pub directories: Vec<String>,
}

impl Repository<'_> {
    /// The checked-out branch, `None` when detached.
    pub fn branch(&self) -> Result<Option<String>, GitError> {
        let branch = self.git_text(["rev-parse", "--abbrev-ref", "HEAD"])?;
        Ok((branch != "HEAD").then_some(branch))
    }

    /// Whether anything outside `.gewu/` is uncommitted.
    pub fn dirty(&self) -> Result<bool, GitError> {
        Ok(!self
            .git_text(["status", "--porcelain", "--", ".", super::EXCLUDE_STATE])?
            .is_empty())
    }

    pub fn refs(&self) -> Result<RepositoryRefs, GitError> {
        let log = self
            .git_text([
                "log",
                "-n",
                COMMITS,
                "--format=%H%x1f%s%x1f%an%x1f%cI%x1f%P",
            ])
            .unwrap_or_default();
        let commits = log
            .lines()
            .filter_map(|line| {
                let mut fields = line.split('\u{1f}');
                Some(CommitSummary {
                    sha: fields.next()?.to_owned(),
                    subject: fields.next()?.to_owned(),
                    author: fields.next()?.to_owned(),
                    date: fields.next()?.to_owned(),
                    has_parent: !fields.next()?.is_empty(),
                })
            })
            .collect();
        let branches = self
            .git_text([
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads",
                "refs/remotes",
            ])?
            .lines()
            .filter(|name| !name.ends_with("/HEAD"))
            .map(str::to_owned)
            .collect();
        let directories = self
            .git_text(["ls-tree", "-r", "-d", "--name-only", "HEAD"])
            .unwrap_or_default()
            .lines()
            .filter(|path| path.split('/').count() <= DIRECTORY_DEPTH && !path.starts_with('.'))
            .map(str::to_owned)
            .collect();
        Ok(RepositoryRefs {
            branch: self.branch().unwrap_or(None),
            dirty: self.dirty()?,
            commits,
            branches,
            pull_requests: self.open_pull_requests(),
            directories,
        })
    }

    fn open_pull_requests(&self) -> Option<Vec<PullRequestSummary>> {
        let output = Command::new("gh")
            .args([
                "pr",
                "list",
                "--limit",
                "30",
                "--json",
                "number,title,author,headRefName",
            ])
            .current_dir(self.root)
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        let list: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).ok()?;
        Some(
            list.iter()
                .filter_map(|request| {
                    Some(PullRequestSummary {
                        number: request["number"].as_u64()?,
                        title: request["title"].as_str()?.to_owned(),
                        author: request["author"]["login"].as_str().unwrap_or("").to_owned(),
                        head_ref: request["headRefName"].as_str().unwrap_or("").to_owned(),
                    })
                })
                .collect(),
        )
    }
}
