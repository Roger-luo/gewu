use std::path::Path;

use super::{GitError, Repository};

impl Repository<'_> {
    /// A detached checkout of `commit` at `path`, sharing this repository's
    /// objects, so a review and its agent never touch the reviewer's tree.
    pub fn add_worktree(&self, path: &Path, commit: &str) -> Result<(), GitError> {
        let path = path.to_string_lossy();
        self.git_text(["worktree", "add", "--detach", "--force", &path, commit])
            .map(drop)
    }

    pub fn remove_worktree(&self, path: &Path) -> Result<(), GitError> {
        let path = path.to_string_lossy();
        self.git_text(["worktree", "remove", "--force", &path])?;
        self.git_text(["worktree", "prune"]).map(drop)
    }

    /// The commit a revision names.
    pub fn commit(&self, revision: &str) -> Result<String, GitError> {
        self.git_text(["rev-parse", "--verify", &format!("{revision}^{{commit}}")])
    }
}
