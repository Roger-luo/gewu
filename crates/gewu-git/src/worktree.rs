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
        self.resolve(revision, true)
    }

    /// The first line of a commit's message.
    pub fn subject(&self, revision: &str) -> Result<String, GitError> {
        self.git_text(["log", "-1", "--format=%s", revision])
    }

    /// A commit or tree a revision names; Base of a range may be Git's empty tree.
    pub fn treeish(&self, revision: &str) -> Result<String, GitError> {
        self.resolve(revision, false)
    }

    fn resolve(&self, revision: &str, commit_only: bool) -> Result<String, GitError> {
        let spec = if commit_only {
            format!("{revision}^{{commit}}")
        } else {
            revision.to_owned()
        };
        self.git_text(["rev-parse", "--verify", &spec])
            .map_err(|error| {
                let message = error.to_string();
                if message.contains("Needed a single revision")
                    || message.contains("unknown revision")
                {
                    GitError::message(format!(
                        "`{revision}` is not a {} in this repository. \
                     Check Base and Head, or pick a recent commit from the list.",
                        if commit_only { "commit" } else { "revision" }
                    ))
                } else {
                    error
                }
            })
    }
}
