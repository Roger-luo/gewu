//! Git diff, tree, and pull request access for Gewu.

mod diff;
mod directory;
mod pull_request;
mod refs;
mod tree;
mod worktree;

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use gewu_review::{ReviewInput, SourceIdentity};
use thiserror::Error;

use diff::DiffParser;

pub use pull_request::PullRequest;
pub use refs::{CommitSummary, PullRequestSummary, RepositoryRefs};

/// Failures reading a Git repository for review.
#[derive(Debug, Error)]
pub enum GitError {
    #[error("{0}")]
    Message(String),
    /// The selected changes are empty; `dirty` when uncommitted edits exist
    /// that a working-tree review would see.
    #[error(
        "nothing to review from {base}: {}",
        if *.working_tree {
            "the working tree matches the base; there are no uncommitted changes"
        } else if *.dirty {
            "the range has no changes; your edits are uncommitted, so review them with \
             `--base HEAD --working-tree`"
        } else {
            "the range has no changes"
        }
    )]
    Empty {
        base: String,
        working_tree: bool,
        dirty: bool,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),
}

impl GitError {
    fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

/// Gewu's own bookkeeping is never part of the change under review, and a
/// harness write must not read as the reviewer editing their code.
const EXCLUDE_STATE: &str = ":(exclude,top).gewu";

#[derive(Clone, Debug)]
pub struct Repository<'a> {
    root: &'a Path,
}

impl<'a> Repository<'a> {
    #[must_use]
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    pub fn review_input(
        &self,
        base: &str,
        head: &str,
        max_chunk_lines: usize,
    ) -> Result<ReviewInput, GitError> {
        let base = self.git_text(["rev-parse", base])?;
        let head = self.git_text(["rev-parse", head])?;
        self.diff_input(&base, &head, ".", max_chunk_lines)
    }

    /// The diff from `base` to `head`, limited to `pathspec`, as a manifest.
    fn diff_input(
        &self,
        base: &str,
        head: &str,
        pathspec: &str,
        max_chunk_lines: usize,
    ) -> Result<ReviewInput, GitError> {
        let output = self
            .command()
            .args([
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "--src-prefix=a/",
                "--dst-prefix=b/",
            ])
            .arg(base)
            .arg(head)
            .args(["--", pathspec, EXCLUDE_STATE])
            .output()
            .map_err(|error| GitError::message(format!("could not run git diff: {error}")))?;
        if !output.status.success() {
            return Err(GitError::message(format!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let source = SourceIdentity::new(base, head, &output.stdout);
        let diff = String::from_utf8(output.stdout)
            .map_err(|_| GitError::message("git diff was not UTF-8"))?;
        Ok(DiffParser::new(source, &diff, max_chunk_lines).parse())
    }

    pub fn worktree_input(
        &self,
        base: &str,
        max_chunk_lines: usize,
    ) -> Result<ReviewInput, GitError> {
        let base = self.git_text(["rev-parse", base])?;
        let git_dir = PathBuf::from(self.git_text(["rev-parse", "--git-dir"])?);
        let git_dir = if git_dir.is_absolute() {
            git_dir
        } else {
            self.root.join(git_dir)
        };
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let index =
            TemporaryIndex::new(git_dir.join(format!("gewu-index-{}-{nonce}", std::process::id())));
        self.git_with_index(&index.path, ["read-tree", &base])?;
        // Naming `.gewu` in an `add` pathspec is an error when it is ignored,
        // even to exclude it, so the state directory is filtered at the diff
        // instead. Staging it here is harmless: nothing reads this tree except
        // that diff.
        self.git_with_index(&index.path, ["add", "-A", "--", "."])?;
        let tree = self.git_text_with_index(&index.path, ["write-tree"])?;
        self.review_input(&base, &tree, max_chunk_lines)
    }

    /// Refuses a review with nothing in it. A range whose ends are the same
    /// commit opens as an empty walkthrough that looks broken; this happened
    /// with `--base <HEAD's commit> --head HEAD` while all work was uncommitted.
    pub fn ensure_changes(&self, input: &ReviewInput, working_tree: bool) -> Result<(), GitError> {
        if !input.changes.is_empty() {
            return Ok(());
        }
        let base = &input.source.base[..input.source.base.len().min(12)];
        let dirty = !working_tree && !self.git_text(["status", "--porcelain"])?.is_empty();
        Err(GitError::Empty {
            base: base.to_owned(),
            working_tree,
            dirty,
        })
    }

    /// A `git` invocation in this repository. Inherited `GIT_DIR` and
    /// `GIT_INDEX_FILE` are dropped: a commit hook sets a relative index, and
    /// `worktree add` then tries to open `.git/index` where `.git` is a file.
    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command
            .current_dir(self.root)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES");
        command
    }

    fn git_text<const N: usize>(&self, args: [&str; N]) -> Result<String, GitError> {
        let output = self
            .command()
            .args(args)
            .output()
            .map_err(|error| GitError::message(format!("could not run git: {error}")))?;
        if !output.status.success() {
            return Err(GitError::message(format!(
                "git failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    }

    fn git_with_index<const N: usize>(
        &self,
        index: &Path,
        args: [&str; N],
    ) -> Result<(), GitError> {
        self.git_text_with_index(index, args).map(|_| ())
    }

    fn git_text_with_index<const N: usize>(
        &self,
        index: &Path,
        args: [&str; N],
    ) -> Result<String, GitError> {
        let output = self
            .command()
            .args(args)
            .env("GIT_INDEX_FILE", index)
            .output()
            .map_err(|error| {
                GitError::message(format!("could not run git with isolated index: {error}"))
            })?;
        if !output.status.success() {
            return Err(GitError::message(format!(
                "git failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    }
}

struct TemporaryIndex {
    path: PathBuf,
}

impl TemporaryIndex {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TemporaryIndex {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(self.path.with_extension("lock"));
    }
}

#[cfg(test)]
mod tests;
