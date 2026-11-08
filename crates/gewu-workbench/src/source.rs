use std::path::{Path, PathBuf};

use gewu_review::{DEFAULT_MAX_CHUNK_LINES, ReviewInput, harness::ReviewOrigin};

use crate::{Source, WorkbenchError, repository::RepositoryRecord};

/// A source made concrete: where the review lives and what it covers.
pub(crate) struct Prepared {
    pub(crate) root: PathBuf,
    pub(crate) isolated: bool,
    pub(crate) input: ReviewInput,
    pub(crate) origin: ReviewOrigin,
    /// A better title than the source alone gives, e.g. a pull request's.
    pub(crate) title: Option<String>,
}

impl Source {
    /// A short name for the task list.
    pub(crate) fn title(&self) -> String {
        match self {
            Self::CommitRange { base, head } => format!("{base}..{head}"),
            Self::WorkingTree { base } => format!("Uncommitted changes against {base}"),
            Self::PullRequest { number, .. } => format!("Pull request #{number}"),
            Self::Directory { path, reference } => {
                let path = if path.trim_matches('/').is_empty() {
                    "/"
                } else {
                    path
                };
                format!("{path} at {reference}")
            }
        }
    }

    pub(crate) fn validate(&self) -> Result<(), WorkbenchError> {
        let missing = |field: &str, value: &str| {
            if value.trim().is_empty() {
                Err(WorkbenchError::invalid(format!("enter a {field}")))
            } else {
                Ok(())
            }
        };
        match self {
            Self::CommitRange { base, head } => {
                missing("base", base)?;
                missing("head", head)
            }
            Self::WorkingTree { base } => missing("base", base),
            Self::PullRequest { number, remote } => {
                if *number == 0 {
                    return Err(WorkbenchError::invalid("enter a pull request number"));
                }
                missing("remote", remote)
            }
            Self::Directory { reference, .. } => missing("ref", reference),
        }
    }

    /// Whether the review runs in the repository itself rather than a
    /// checkout of its own.
    pub(crate) fn in_place(&self) -> bool {
        matches!(self, Self::WorkingTree { .. })
    }

    /// Resolve the source, check it out at `checkout` unless it reviews the
    /// working tree, and freeze its manifest.
    pub(crate) fn prepare(
        &self,
        repository: &RepositoryRecord,
        checkout: &Path,
    ) -> Result<Prepared, WorkbenchError> {
        let git = repository.git();
        let lines = DEFAULT_MAX_CHUNK_LINES;
        // `Err(path)` marks a directory: everything under it is the change.
        let (head, base, title): (String, Result<String, &str>, Option<String>) = match self {
            Self::WorkingTree { base } => {
                let input = git.worktree_input(base, lines)?;
                git.ensure_changes(&input, true)?;
                return Ok(Prepared {
                    root: repository.path.clone(),
                    isolated: false,
                    input,
                    origin: ReviewOrigin {
                        base: base.clone(),
                        head: None,
                        working_tree: true,
                    },
                    title: None,
                });
            }
            Self::CommitRange { base, head } => (git.commit(head)?, Ok(git.treeish(base)?), None),
            Self::PullRequest { number, remote } => {
                let request = git.pull_request(remote, *number)?;
                let title = request.title.map(|title| format!("#{number} {title}"));
                (request.head, Ok(request.base), title)
            }
            Self::Directory { path, reference } => (git.commit(reference)?, Err(path), None),
        };
        if !checkout.exists() {
            if let Some(parent) = checkout.parent() {
                std::fs::create_dir_all(parent)?;
            }
            git.add_worktree(checkout, &head)?;
        }
        let checked_out = gewu_git::Repository::new(checkout);
        let input = match base {
            Ok(base) => checked_out.review_input(&base, &head, lines)?,
            Err(path) => checked_out.directory_input(&head, path, lines)?,
        };
        checked_out.ensure_changes(&input, false)?;
        Ok(Prepared {
            root: checkout.to_path_buf(),
            isolated: true,
            origin: ReviewOrigin {
                base: input.source.base.clone(),
                head: Some(head),
                working_tree: false,
            },
            input,
            title,
        })
    }
}
