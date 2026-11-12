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

    /// The full messages of the commits in `base..head`, newest first, at
    /// most `limit` of them, merges and trailers left out; only `head`'s
    /// when `base` is not a commit.
    pub fn messages(&self, base: &str, head: &str, limit: usize) -> Result<Vec<String>, GitError> {
        let count = format!("--max-count={limit}");
        let range = format!("{base}..{head}");
        let log = self
            .git_text(["log", &count, "--no-merges", "--format=%B%x00", &range])
            .or_else(|_| self.git_text(["log", "-1", "--format=%B%x00", head]))?;
        Ok(log
            .split('\0')
            .map(without_trailers)
            .filter(|message| !message.is_empty())
            .collect())
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

/// A commit message without its closing trailers (`Co-Authored-By: …`,
/// `Signed-off-by: …`), which say who, not why.
fn without_trailers(message: &str) -> String {
    let message = message.trim();
    let Some((body, last)) = message.rsplit_once("\n\n") else {
        return message.to_owned();
    };
    let trailer = |line: &str| {
        line.split_once(": ").is_some_and(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '-')
        })
    };
    if last.lines().all(trailer) {
        body.trim().to_owned()
    } else {
        message.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn trailers_are_dropped_and_the_body_kept() {
        let message = "feat: pin blocks\n\nPinning fixes the layout.\n\nCo-Authored-By: A <a@b>\nClaude-Session: https://x";
        assert_eq!(
            super::without_trailers(message),
            "feat: pin blocks\n\nPinning fixes the layout."
        );
        assert_eq!(super::without_trailers("fix: typo"), "fix: typo");
        let prose = "feat: x\n\nWhy: it was slow, and this is not a trailer block.\nSecond line";
        assert_eq!(super::without_trailers(prose), prose);
    }
}
